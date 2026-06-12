use crate::type_map::{UnsupportedTypePolicy, vox_type_to_sql};
use crate::{BackendKind, SqlBackendError};
use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl};
use vox_ast::types::TypeExpr;

#[must_use]
pub fn to_snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    let mut prev_is_lower_or_digit = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if prev_is_lower_or_digit && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
                prev_is_lower_or_digit = false;
            } else {
                out.push(ch);
                prev_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
            }
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
            prev_is_lower_or_digit = false;
        }
    }
    out.trim_matches('_').to_string()
}

#[must_use]
pub fn live_table_name(table: &TableDecl) -> String {
    if table.is_extern
        && let Some(src) = table.source.as_ref().map(|s| s.trim())
        && !src.is_empty()
    {
        return src.to_string();
    }
    to_snake_case(&table.name)
}

#[must_use]
pub fn type_expr_to_vox_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => name.clone(),
        TypeExpr::Generic { name, args, .. } => {
            let args = args
                .iter()
                .map(type_expr_to_vox_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}[{args}]")
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let params = params
                .iter()
                .map(type_expr_to_vox_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({params}) -> {}", type_expr_to_vox_type(return_type))
        }
        TypeExpr::Tuple { elements, .. } => {
            let elems = elements
                .iter()
                .map(type_expr_to_vox_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({elems})")
        }
        TypeExpr::Unit { .. } => "Unit".to_string(),
        TypeExpr::Infer { .. } => "_".to_string(),
        TypeExpr::Decimal { .. } => "dec".to_string(),
    }
}

#[must_use]
pub fn index_to_ddl(_backend: BackendKind, idx: &IndexDecl) -> String {
    let table = to_snake_case(&idx.table_name);
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{table}_{name} ON {table} ({cols});",
        name = idx.index_name,
        cols = idx.columns.join(", ")
    )
}

pub fn table_to_ddl(
    backend: BackendKind,
    table: &TableDecl,
    policy: UnsupportedTypePolicy,
) -> Result<String, SqlBackendError> {
    let table_name = live_table_name(table);
    let mut cols = Vec::with_capacity(table.fields.len() + 2);
    cols.push(id_column_sql(backend).to_string());
    for field in &table.fields {
        let vox_type = type_expr_to_vox_type(&field.type_ann);
        let sql_type = vox_type_to_sql(backend, &vox_type, policy)
            .map_err(|e| SqlBackendError::new(e.to_string()))?;
        let not_null = if is_optional_type(&field.type_ann) {
            ""
        } else {
            " NOT NULL"
        };
        cols.push(format!("{} {}{}", field.name, sql_type, not_null));
    }
    if let Some(pk) = &table.primary_key {
        cols.push(format!("UNIQUE ({pk})"));
    }
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n);",
        table_name,
        cols.join(",\n    ")
    ))
}

pub fn add_column_ddl(
    backend: BackendKind,
    table: &str,
    column: &str,
    type_ann: &TypeExpr,
    policy: UnsupportedTypePolicy,
) -> Result<String, SqlBackendError> {
    let vox_type = type_expr_to_vox_type(type_ann);
    let sql_type = vox_type_to_sql(backend, &vox_type, policy)
        .map_err(|e| SqlBackendError::new(e.to_string()))?;
    let not_null = if is_optional_type(type_ann) {
        ""
    } else {
        " NOT NULL"
    };
    Ok(format!(
        "ALTER TABLE {table} ADD COLUMN {column} {sql_type}{not_null};"
    ))
}

pub fn collection_to_ddl(
    backend: BackendKind,
    collection: &CollectionDecl,
) -> Result<String, SqlBackendError> {
    if backend != BackendKind::Libsql {
        return Err(SqlBackendError::new(
            "collection auto-migrations currently supported only on libsql/sqlite",
        ));
    }
    let table = to_snake_case(&collection.name);
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {table} (\n    _id TEXT PRIMARY KEY,\n    doc TEXT NOT NULL,\n    _creationTime INTEGER NOT NULL\n);"
    ))
}

fn is_optional_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Generic { name, .. } if name.eq_ignore_ascii_case("option"))
}

fn id_column_sql(backend: BackendKind) -> &'static str {
    match backend {
        BackendKind::Libsql => "_id INTEGER PRIMARY KEY AUTOINCREMENT",
        BackendKind::Postgres => "_id BIGSERIAL PRIMARY KEY",
        BackendKind::MySql => "_id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_ast::span::Span;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn table_name_respects_extern_source() {
        let t = TableDecl {
            name: "Task".to_string(),
            fields: vec![],
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub: false,
            is_deprecated: false,
            primary_key: None,
            is_extern: true,
            source: Some("legacy_tasks".to_string()),
            span: span(),
        };
        assert_eq!(live_table_name(&t), "legacy_tasks");
    }
}
