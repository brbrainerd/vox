//! DDL Compiler — convert `@table` AST declarations into SQLite DDL.
//!
//! This is the bridge between the Vox type system and SQLite's physical schema.
//! It generates `CREATE TABLE`, `CREATE INDEX`, and type-safe DDL from the AST.

use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl, VectorIndexDecl};
use vox_ast::types::TypeExpr;
use crate::schema_digest::{TableInfo, CollectionInfo, IndexInfo};

/// Generate `CREATE TABLE` SQL statements from table declarations.
pub fn tables_to_ddl(tables: &[&TableDecl]) -> Vec<String> {
    tables.iter().map(|t| table_to_ddl(t)).collect()
}

/// Generate a single `CREATE TABLE` statement.
pub fn table_to_ddl(table: &TableDecl) -> String {
    let mut cols = Vec::new();

    // Every table gets an auto-generated _id and _creationTime
    cols.push("    _id TEXT PRIMARY KEY NOT NULL".to_string());
    cols.push(
        "    _creationTime TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"
            .to_string(),
    );

    for field in &table.fields {
        let sqlite_type = type_to_sqlite_type(&field.type_ann);
        let nullable = if is_optional_type(&field.type_ann) {
            ""
        } else {
            " NOT NULL"
        };
        cols.push(format!("    {} {}{}", field.name, sqlite_type, nullable));
    }

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        to_snake_case(&table.name),
        cols.join(",\n")
    )
}

/// Generate `CREATE TABLE` SQL statements from collection declarations.
pub fn collections_to_ddl(collections: &[&CollectionDecl]) -> Vec<String> {
    collections.iter().map(|c| collection_to_ddl(c)).collect()
}

/// Generate a single `CREATE TABLE` and schema storage statement for a collection.
pub fn collection_to_ddl(collection: &CollectionDecl) -> String {
    let name = to_snake_case(&collection.name);
    // document store structure: _id INTEGER, _data TEXT (JSON), timestamps
    let cols = vec![
        "    _id INTEGER PRIMARY KEY AUTOINCREMENT".to_string(),
        "    _data TEXT NOT NULL".to_string(), // JSON doc
        "    _created_at TEXT DEFAULT (datetime('now'))".to_string(),
        "    _updated_at TEXT DEFAULT (datetime('now'))".to_string(),
    ];

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        name,
        cols.join(",\n")
    )
}

/// Generate `CREATE INDEX` SQL statements from index declarations.
pub fn indexes_to_ddl(indexes: &[&IndexDecl]) -> Vec<String> {
    indexes.iter().map(|idx| index_to_ddl(idx)).collect()
}

/// Generate a single `CREATE INDEX` statement.
pub fn index_to_ddl(idx: &IndexDecl) -> String {
    let table = to_snake_case(&idx.table_name);
    let cols = idx.columns.join(", ");
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{}_{} ON {} ({});",
        table, idx.index_name, table, cols
    )
}

/// Generate a single `CREATE INDEX` statement for a collection document field.
pub fn collection_index_to_ddl(idx: &IndexDecl) -> String {
    let table = to_snake_case(&idx.table_name);
    // Extract JSON field for index: json_extract(_data, '$.field')
    let cols_str: Vec<String> = idx
        .columns
        .iter()
        .map(|c| format!("json_extract(_data, '$.{}')", c))
        .collect();
    let cols = cols_str.join(", ");
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{}_{} ON {} ({});",
        table, idx.index_name, table, cols
    )
}

/// Generate DDL for a vector index (stored as metadata table + index).
pub fn vector_index_to_ddl(vidx: &VectorIndexDecl) -> Vec<String> {
    let table = to_snake_case(&vidx.table_name);
    let mut stmts = Vec::new();

    // Create a metadata entry for the vector index
    // (actual vector indexing would require an extension like sqlite-vss)
    stmts.push(format!(
        "-- Vector index: {}.{} on column '{}' with {} dimensions",
        vidx.table_name, vidx.index_name, vidx.column, vidx.dimensions
    ));

    // Ensure the column exists as a BLOB for binary embedding storage
    stmts.push(format!(
        "CREATE INDEX IF NOT EXISTS idx_{}_{}_vec ON {} ({});",
        table, vidx.index_name, table, vidx.column
    ));

    stmts
}


/// Generate `CREATE TABLE` from `TableInfo`.
pub fn table_info_to_ddl(info: &TableInfo) -> String {
    let mut cols = Vec::new();
    cols.push("    _id TEXT PRIMARY KEY NOT NULL".to_string());
    cols.push(
        "    _creationTime TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"
            .to_string(),
    );

    for field in &info.fields {
        let sqlite_type = if field.type_str.starts_with("FLOAT32(") {
            &field.type_str
        } else {
            vox_type_to_sqlite_type(&field.type_str)
        };
        let nullable = if field.is_optional { "" } else { " NOT NULL" };
        cols.push(format!("    {} {}{}", field.name, sqlite_type, nullable));
    }

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        to_snake_case(&info.name),
        cols.join(",\n")
    )
}

/// Generate `CREATE TABLE` from `CollectionInfo`.
pub fn collection_info_to_ddl(info: &CollectionInfo) -> String {
    let name = to_snake_case(&info.name);
    let cols = vec![
        "    _id INTEGER PRIMARY KEY AUTOINCREMENT".to_string(),
        "    _data TEXT NOT NULL".to_string(),
        "    _created_at TEXT DEFAULT (datetime('now'))".to_string(),
        "    _updated_at TEXT DEFAULT (datetime('now'))".to_string(),
    ];

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        name,
        cols.join(",\n")
    )
}

/// Generate `CREATE INDEX` from `IndexInfo`.
pub fn index_info_to_ddl(info: &IndexInfo) -> String {
    let table = to_snake_case(&info.table_name);
    let cols = info.columns.join(", ");
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{}_{} ON {} ({});",
        table, info.index_name, table, cols
    )
}

/// Map a Vox type string (e.g. "str", "Option[int]") to a SQLite type.
pub fn vox_type_to_sqlite_type(ty_str: &str) -> &'static str {
    if ty_str.contains("Option[") {
        // Recursively strip Option
        let inner = ty_str.trim_start_matches("Option[").trim_end_matches(']');
        return vox_type_to_sqlite_type(inner);
    }
    match ty_str {
        "str" | "String" => "TEXT",
        "int" | "i32" | "i64" | "u32" | "u64" => "INTEGER",
        "float" | "f32" | "f64" | "float64" => "REAL",
        "bool" => "INTEGER",
        "bytes" | "Bytes" => "BLOB",
        _ if ty_str.starts_with("Id[") => "TEXT",
        _ if ty_str.starts_with("List[") || ty_str.starts_with("list[") => "TEXT",
        _ if ty_str.starts_with("Map[") || ty_str.starts_with("map[") => "TEXT",
        _ => "TEXT",
    }
}

/// Map a Vox `TypeExpr` to a SQLite column type.
pub fn type_to_sqlite_type(ty: &TypeExpr) -> &'static str {
    match ty {
        TypeExpr::Named { name, .. } => match name.as_str() {
            "str" | "String" => "TEXT",
            "int" | "i32" | "i64" | "u32" | "u64" => "INTEGER",
            "float" | "f32" | "f64" | "float64" => "REAL",
            "bool" => "INTEGER", // SQLite uses 0/1 for booleans
            "bytes" | "Bytes" => "BLOB",
            _ => "TEXT", // Custom types serialize as JSON TEXT
        },
        TypeExpr::Generic { name, args, .. } => match name.as_str() {
            "Option" => args.first().map(type_to_sqlite_type).unwrap_or("TEXT"),
            "Id" => "TEXT",            // Foreign key references stored as UUID TEXT
            "List" | "list" => "TEXT", // JSON array serialized as TEXT
            "Map" | "map" => "TEXT",   // JSON object serialized as TEXT
            "Set" | "set" => "TEXT",   // JSON array serialized as TEXT
            _ => "TEXT",
        },
        TypeExpr::Union { .. } => "TEXT", // Tagged union as JSON
        TypeExpr::Tuple { .. } => "TEXT", // Tuple as JSON array
        TypeExpr::Map { .. } => "TEXT",
        TypeExpr::Set { .. } => "TEXT",
        TypeExpr::Intersection { .. } => "TEXT",
        TypeExpr::Function { .. } => "TEXT", // Should not appear in tables
        TypeExpr::Unit { .. } => "TEXT",
    }
}

/// Check if a TypeExpr is optional (wrapped in Option).
fn is_optional_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Generic { name, .. } if name == "Option")
}

/// Convert PascalCase to snake_case for SQL table names.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

// ── Schema Diffing ──────────────────────────────────────

/// Represents differences between two schema versions.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    pub added_collections: Vec<String>,
    pub removed_collections: Vec<String>,
    pub added_tables: Vec<String>,
    pub removed_tables: Vec<String>,
    pub added_columns: Vec<(String, String, String)>, // (table, column_name, type)
    pub removed_columns: Vec<(String, String)>,       // (table, column_name)
    pub added_indexes: Vec<(String, String, Vec<String>)>, // (table, index_name, columns)
    pub removed_indexes: Vec<(String, String)>,       // (table, index_name)
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

        assert!(sql
            .iter()
            .any(|s| s.contains("ALTER TABLE task ADD COLUMN done INTEGER")));
    }
}
