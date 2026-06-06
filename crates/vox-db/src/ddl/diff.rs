use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl};

use super::emit::{
    collection_to_ddl, index_to_ddl, table_to_ddl, to_snake_case, type_to_sqlite_type,
};

// ── Schema Diffing ──────────────────────────────────────

/// Represents differences between two schema versions.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    /// Collection names present only in the new schema.
    pub added_collections: Vec<String>,
    /// Collection names dropped from the new schema.
    pub removed_collections: Vec<String>,
    /// Table names present only in the new schema.
    pub added_tables: Vec<String>,
    /// Table names dropped from the new schema.
    pub removed_tables: Vec<String>,
    /// `(table, column_name, sql_type)` tuples added.
    pub added_columns: Vec<(String, String, String)>,
    /// `(table, column_name)` tuples removed.
    pub removed_columns: Vec<(String, String)>,
    /// `(table, index_name, columns)` tuples added.
    pub added_indexes: Vec<(String, String, Vec<String>)>,
    /// `(table, index_name)` tuples removed.
    pub removed_indexes: Vec<(String, String)>,
}

/// Diff two schema versions to produce migration SQL.
pub fn diff_schemas(
    old_tables: &[&TableDecl],
    new_tables: &[&TableDecl],
    old_collections: &[&CollectionDecl],
    new_collections: &[&CollectionDecl],
    old_indexes: &[&IndexDecl],
    new_indexes: &[&IndexDecl],
) -> SchemaDiff {
    let old_names: std::collections::HashSet<&str> =
        old_tables.iter().map(|t| t.name.as_str()).collect();
    let new_names: std::collections::HashSet<&str> =
        new_tables.iter().map(|t| t.name.as_str()).collect();

    let added_tables: Vec<String> = new_names
        .difference(&old_names)
        .map(|s| s.to_string())
        .collect();
    let removed_tables: Vec<String> = old_names
        .difference(&new_names)
        .map(|s| s.to_string())
        .collect();

    // Find added/removed columns in tables that exist in both versions
    let mut added_columns = Vec::new();
    let mut removed_columns = Vec::new();

    for new_t in new_tables {
        if let Some(old_t) = old_tables.iter().find(|t| t.name == new_t.name) {
            let old_fields: std::collections::HashSet<&str> =
                old_t.fields.iter().map(|f| f.name.as_str()).collect();
            let new_fields: std::collections::HashSet<&str> =
                new_t.fields.iter().map(|f| f.name.as_str()).collect();

            for f in new_t.fields.iter() {
                if !old_fields.contains(f.name.as_str()) {
                    added_columns.push((
                        new_t.name.clone(),
                        f.name.clone(),
                        type_to_sqlite_type(&f.type_ann).to_string(),
                    ));
                }
            }

            for name in old_fields.difference(&new_fields) {
                removed_columns.push((new_t.name.clone(), name.to_string()));
            }
        }
    }

    // Find added/removed indexes
    let old_idx_names: std::collections::HashSet<(&str, &str)> = old_indexes
        .iter()
        .map(|i| (i.table_name.as_str(), i.index_name.as_str()))
        .collect();
    let new_idx_names: std::collections::HashSet<(&str, &str)> = new_indexes
        .iter()
        .map(|i| (i.table_name.as_str(), i.index_name.as_str()))
        .collect();

    let added_indexes: Vec<(String, String, Vec<String>)> = new_indexes
        .iter()
        .filter(|i| !old_idx_names.contains(&(i.table_name.as_str(), i.index_name.as_str())))
        .map(|i| {
            (
                i.table_name.clone(),
                i.index_name.clone(),
                i.columns.clone(),
            )
        })
        .collect();

    let removed_indexes: Vec<(String, String)> = old_indexes
        .iter()
        .filter(|i| !new_idx_names.contains(&(i.table_name.as_str(), i.index_name.as_str())))
        .map(|i| (i.table_name.clone(), i.index_name.clone()))
        .collect();

    // Collection diffs
    let old_coll_names: std::collections::HashSet<&str> =
        old_collections.iter().map(|c| c.name.as_str()).collect();
    let new_coll_names: std::collections::HashSet<&str> =
        new_collections.iter().map(|c| c.name.as_str()).collect();

    let added_collections: Vec<String> = new_coll_names
        .difference(&old_coll_names)
        .map(|s| s.to_string())
        .collect();
    let removed_collections: Vec<String> = old_coll_names
        .difference(&new_coll_names)
        .map(|s| s.to_string())
        .collect();

    SchemaDiff {
        added_collections,
        removed_collections,
        added_tables,
        removed_tables,
        added_columns,
        removed_columns,
        added_indexes,
        removed_indexes,
    }
}

/// Generate SQL migration statements from a schema diff.
pub fn diff_to_sql(
    diff: &SchemaDiff,
    new_tables: &[&TableDecl],
    new_indexes: &[&IndexDecl],
) -> Vec<String> {
    let mut stmts = Vec::new();

    // Create new tables
    for table_name in &diff.added_tables {
        if let Some(t) = new_tables.iter().find(|t| t.name == *table_name) {
            stmts.push(table_to_ddl(t));
        }
    }

    // Add new columns (SQLite supports ADD COLUMN but not DROP COLUMN easily)
    for (table, col, ty) in &diff.added_columns {
        stmts.push(format!(
            "ALTER TABLE {} ADD COLUMN {} {};",
            to_snake_case(table),
            col,
            ty
        ));
    }

    // Create new indexes
    for (table, idx_name, _cols) in &diff.added_indexes {
        if let Some(i) = new_indexes
            .iter()
            .find(|i| i.table_name == *table && i.index_name == *idx_name)
        {
            stmts.push(index_to_ddl(i));
        }
    }

    // Drop removed indexes
    for (table, idx_name) in &diff.removed_indexes {
        stmts.push(format!(
            "DROP INDEX IF EXISTS idx_{}_{};",
            to_snake_case(table),
            idx_name
        ));
    }

    // Create new collections
    for coll_name in &diff.added_collections {
        // Ideally we'd look up the exact CollectionDecl, but we only have its name.
        // And collection schemas are always the same fields (_id, _data, _created_at, _updated_at).
        // Let's generate a synthetic one to use collection_to_ddl.
        let synth = CollectionDecl {
            name: coll_name.clone(),
            fields: vec![],
            description: None,
            is_pub: true,
            has_spread: false,
            span: vox_ast::span::Span { start: 0, end: 0 },
        };
        stmts.push(collection_to_ddl(&synth));
    }

    // Note: DROP TABLE and DROP COLUMN are not generated by default for safety.
    // Removed tables/columns are logged as comments.
    for table in &diff.removed_tables {
        stmts.push(format!(
            "-- WARNING: Table '{}' was removed from schema (not dropped for safety)",
            table
        ));
    }
    for (table, col) in &diff.removed_columns {
        stmts.push(format!(
            "-- WARNING: Column '{}' was removed from table '{}' (not dropped for safety)",
            col, table
        ));
    }

    stmts
}

/// Generate a human-readable description of schema changes.
pub fn describe_diff(diff: &SchemaDiff) -> String {
    let mut parts = Vec::new();

    if !diff.added_tables.is_empty() {
        parts.push(format!("Added table(s): {}", diff.added_tables.join(", ")));
    }
    if !diff.removed_tables.is_empty() {
        parts.push(format!(
            "Removed table(s): {}",
            diff.removed_tables.join(", ")
        ));
    }
    if !diff.added_collections.is_empty() {
        parts.push(format!(
            "Added collection(s): {}",
            diff.added_collections.join(", ")
        ));
    }
    if !diff.removed_collections.is_empty() {
        parts.push(format!(
            "Removed collection(s): {}",
            diff.removed_collections.join(", ")
        ));
    }
    if !diff.added_columns.is_empty() {
        let cols: Vec<String> = diff
            .added_columns
            .iter()
            .map(|(t, c, ty)| format!("{}.{}({})", t, c, ty))
            .collect();
        parts.push(format!("Added column(s): {}", cols.join(", ")));
    }
    if !diff.removed_columns.is_empty() {
        let cols: Vec<String> = diff
            .removed_columns
            .iter()
            .map(|(t, c)| format!("{}.{}", t, c))
            .collect();
        parts.push(format!("Removed column(s): {}", cols.join(", ")));
    }
    if !diff.added_indexes.is_empty() {
        parts.push(format!("Added {} index(es)", diff.added_indexes.len()));
    }
    if !diff.removed_indexes.is_empty() {
        parts.push(format!("Removed {} index(es)", diff.removed_indexes.len()));
    }

    if parts.is_empty() {
        "No schema changes detected.".to_string()
    } else {
        parts.join("\n")
    }
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ddl::emit::{sqlite_affinity_for_named_vox_type, vox_type_to_sqlite_type};
    use vox_ast::decl::*;
    use vox_ast::span::Span;
    use vox_ast::types::TypeExpr;

    fn s() -> Span {
        Span { start: 0, end: 0 }
    }

    fn field(name: &str, ty: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Named {
                name: ty.to_string(),
                span: s(),
            },
            description: None,
            span: s(),
        }
    }

    fn opt_field(name: &str, inner_ty: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Generic {
                name: "Option".to_string(),
                args: vec![TypeExpr::Named {
                    name: inner_ty.to_string(),
                    span: s(),
                }],
                span: s(),
            },
            description: None,
            span: s(),
        }
    }

    fn table(name: &str, fields: Vec<TableField>) -> TableDecl {
        TableDecl {
            name: name.to_string(),
            fields,
            primary_key: None,
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: Vec::new(),
            cors: None,
            is_pub: false,
            is_deprecated: false,
            span: s(),
        }
    }

    #[test]
    fn test_basic_ddl() {
        let t = table(
            "Task",
            vec![
                field("title", "str"),
                field("done", "bool"),
                field("priority", "int"),
            ],
        );

        let ddl = table_to_ddl(&t);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS task"));
        assert!(ddl.contains("_id TEXT PRIMARY KEY NOT NULL"));
        assert!(ddl.contains("_creationTime TEXT NOT NULL"));
        assert!(ddl.contains("title TEXT NOT NULL"));
        assert!(ddl.contains("done INTEGER NOT NULL"));
        assert!(ddl.contains("priority INTEGER NOT NULL"));
    }

    #[test]
    fn test_optional_field() {
        let t = table("Note", vec![field("body", "str"), opt_field("tag", "str")]);

        let ddl = table_to_ddl(&t);
        assert!(ddl.contains("body TEXT NOT NULL"));
        // Optional field should NOT have NOT NULL
        assert!(ddl.contains("tag TEXT,") || ddl.contains("tag TEXT\n"));
    }

    #[test]
    fn test_index_ddl() {
        let idx = IndexDecl {
            table_name: "Task".to_string(),
            index_name: "by_done".to_string(),
            columns: vec!["done".to_string(), "priority".to_string()],
            span: s(),
        };

        let ddl = index_to_ddl(&idx);
        assert!(ddl.contains("CREATE INDEX IF NOT EXISTS idx_task_by_done"));
        assert!(ddl.contains("ON task (done, priority)"));
    }

    #[test]
    fn test_type_mapping() {
        assert_eq!(
            type_to_sqlite_type(&TypeExpr::Named {
                name: "str".to_string(),
                span: s()
            }),
            "TEXT"
        );
        assert_eq!(
            type_to_sqlite_type(&TypeExpr::Named {
                name: "int".to_string(),
                span: s()
            }),
            "INTEGER"
        );
        assert_eq!(
            type_to_sqlite_type(&TypeExpr::Named {
                name: "float".to_string(),
                span: s()
            }),
            "REAL"
        );
        assert_eq!(
            type_to_sqlite_type(&TypeExpr::Named {
                name: "bool".to_string(),
                span: s()
            }),
            "INTEGER"
        );
        assert_eq!(
            type_to_sqlite_type(&TypeExpr::Named {
                name: "bytes".to_string(),
                span: s()
            }),
            "BLOB"
        );
        assert_eq!(vox_type_to_sqlite_type("i64"), "INTEGER");
        assert_eq!(vox_type_to_sqlite_type("String"), "TEXT");
        assert_eq!(sqlite_affinity_for_named_vox_type("int"), Some("INTEGER"));
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(to_snake_case("Task"), "task");
        assert_eq!(to_snake_case("UserProfile"), "user_profile");
        assert_eq!(to_snake_case("HTTPRoute"), "h_t_t_p_route");
    }

    #[test]
    fn test_schema_diff() {
        let old_t = table("Task", vec![field("title", "str")]);
        let new_t = table("Task", vec![field("title", "str"), field("done", "bool")]);
        let new_t2 = table("User", vec![field("name", "str")]);

        let diff = diff_schemas(&[&old_t], &[&new_t, &new_t2], &[], &[], &[], &[]);

        assert!(diff.added_tables.contains(&"User".to_string()));
        assert_eq!(diff.added_columns.len(), 1);
        assert_eq!(diff.added_columns[0].1, "done");
    }

    #[test]
    fn test_diff_to_sql() {
        let old_t = table("Task", vec![field("title", "str")]);
        let new_t = table("Task", vec![field("title", "str"), field("done", "bool")]);

        let diff = diff_schemas(&[&old_t], &[&new_t], &[], &[], &[], &[]);
        let sql = diff_to_sql(&diff, &[&new_t], &[]);

        assert!(
            sql.iter()
                .any(|s| s.contains("ALTER TABLE task ADD COLUMN done INTEGER"))
        );
    }
}
