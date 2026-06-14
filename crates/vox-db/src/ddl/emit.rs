//! DDL Compiler — convert `@table` AST declarations into SQLite DDL.
//!
//! This is the bridge between the Vox type system and SQLite's physical schema.
//! It generates `CREATE TABLE`, `CREATE INDEX`, and type-safe DDL from the AST.

use crate::schema_digest::{CollectionInfo, IndexInfo, TableInfo};
use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl, VectorIndexDecl};
use vox_ast::scalar_mapping::VoxScalar;
use vox_ast::types::TypeExpr;

/// SQLite affinity for a Vox **named** scalar or common alias (`String`, `i64`, …).
///
/// Aligns with [`VoxScalar`](vox_ast::scalar_mapping::VoxScalar) and Rust table emit in `vox-codegen-rust`.
#[must_use]
pub fn sqlite_affinity_for_named_vox_type(name: &str) -> Option<&'static str> {
    if let Some(s) = VoxScalar::parse(name) {
        return Some(s.as_sqlite_affinity());
    }
    match name {
        "String" => Some(VoxScalar::Str.as_sqlite_affinity()),
        "i32" | "i64" | "u32" | "u64" => Some(VoxScalar::Int.as_sqlite_affinity()),
        "f32" | "f64" | "float64" => Some(VoxScalar::Float.as_sqlite_affinity()),
        "bytes" | "Bytes" => Some("BLOB"),
        _ => None,
    }
}

/// Generate `CREATE TABLE` SQL statements from table declarations.
pub fn tables_to_ddl(tables: &[&TableDecl]) -> Vec<String> {
    tables.iter().map(|t| table_to_ddl(t)).collect()
}

/// Generate a single `CREATE TABLE` statement.
pub fn table_to_ddl(table: &TableDecl) -> String {
    let mut cols = Vec::new();

    // Keep parity with vox-codegen's runtime DDL for app `@table` surfaces.
    cols.push("    _id INTEGER PRIMARY KEY AUTOINCREMENT".to_string());

    for field in &table.fields {
        let sqlite_type = type_to_sqlite_type(&field.type_ann);
        let nullable = if is_optional_type(&field.type_ann) {
            ""
        } else {
            " NOT NULL"
        };
        cols.push(format!("    {} {}{}", field.name, sqlite_type, nullable));
    }
    if let Some(pk) = &table.primary_key {
        cols.push(format!("    UNIQUE ({pk})"));
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

/// Map a Vox type string (e.g. `"str"`, or nested optional like `Option` of `int`) to a SQLite type.
pub fn vox_type_to_sqlite_type(ty_str: &str) -> &'static str {
    if ty_str.contains("Option[") {
        // Recursively strip Option
        let inner = ty_str.trim_start_matches("Option[").trim_end_matches(']');
        return vox_type_to_sqlite_type(inner);
    }
    if let Some(sql) = sqlite_affinity_for_named_vox_type(ty_str) {
        return sql;
    }
    match ty_str {
        _ if ty_str.starts_with("Id[") => "TEXT",
        _ if ty_str.starts_with("List[") || ty_str.starts_with("list[") => "TEXT",
        _ if ty_str.starts_with("Map[") || ty_str.starts_with("map[") => "TEXT",
        _ => "TEXT",
    }
}

/// Map a Vox `TypeExpr` to a SQLite column type.
pub fn type_to_sqlite_type(ty: &TypeExpr) -> &'static str {
    match ty {
        TypeExpr::Named { name, .. } => sqlite_affinity_for_named_vox_type(name).unwrap_or("TEXT"), // JSON TEXT for unknown ADTs
        TypeExpr::Generic { name, args, .. } => match name.as_str() {
            "Option" => args.first().map(type_to_sqlite_type).unwrap_or("TEXT"),
            "Id" => "TEXT",            // Foreign key references stored as UUID TEXT
            "List" | "list" => "TEXT", // JSON array serialized as TEXT
            "Map" | "map" => "TEXT",   // JSON object serialized as TEXT
            "Set" | "set" => "TEXT",   // JSON array serialized as TEXT
            _ => "TEXT",
        },
        TypeExpr::Tuple { .. } => "TEXT",    // Tuple as JSON array
        TypeExpr::Function { .. } => "TEXT", // Should not appear in tables
        TypeExpr::Decimal { .. } => "TEXT",
        TypeExpr::Unit { .. } => "TEXT",
        TypeExpr::Infer { .. } => "TEXT", // Unknown/inferred — default to TEXT
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

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn vector_index_to_ddl_emits_comment_and_snake_cased_create_index() {
        let vidx = VectorIndexDecl {
            table_name: "UserProfile".to_string(),
            index_name: "by_embedding".to_string(),
            column: "embedding".to_string(),
            dimensions: 1536,
            filter_fields: Vec::new(),
            span: vox_ast::span::Span::new(0, 0),
        };

        let stmts = vector_index_to_ddl(&vidx);

        assert_eq!(stmts.len(), 2);
        // Comment preserves the RAW (non-snake-cased) table/index/column names and dims.
        assert_eq!(
            stmts[0],
            "-- Vector index: UserProfile.by_embedding on column 'embedding' with 1536 dimensions"
        );
        // CREATE INDEX snake_cases the table name (UserProfile -> user_profile) but
        // leaves index_name and column verbatim.
        assert_eq!(
            stmts[1],
            "CREATE INDEX IF NOT EXISTS idx_user_profile_by_embedding_vec ON user_profile (embedding);"
        );
    }
}
