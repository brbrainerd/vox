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
        #[cfg(feature = "postgres")]
        BackendKind::Postgres => "_id BIGSERIAL PRIMARY KEY",
        #[cfg(feature = "mysql")]
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

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::BackendKind;
    use vox_ast::decl::CollectionDecl;
    use vox_ast::span::Span;
    use vox_ast::types::TypeExpr;

    fn sp() -> Span {
        Span { start: 0, end: 0 }
    }

    // --- collection_to_ddl ---

    #[test]
    fn collection_to_ddl_libsql_generates_correct_schema() {
        let col = CollectionDecl {
            name: "UserMessage".to_string(),
            fields: vec![],
            description: None,
            is_pub: false,
            has_spread: false,
            span: sp(),
        };
        let ddl = collection_to_ddl(BackendKind::Libsql, &col)
            .expect("Libsql collection DDL should succeed");
        assert!(
            ddl.contains("user_message"),
            "table name should be snake_case: {ddl}"
        );
        assert!(
            ddl.contains("_id TEXT PRIMARY KEY"),
            "should include _id TEXT PRIMARY KEY: {ddl}"
        );
        assert!(
            ddl.contains("doc TEXT NOT NULL"),
            "should include doc column: {ddl}"
        );
        assert!(
            ddl.contains("_creationTime INTEGER NOT NULL"),
            "should include _creationTime: {ddl}"
        );
    }

    #[test]
    #[cfg(any(feature = "postgres", feature = "mysql"))]
    fn collection_to_ddl_non_libsql_returns_error() {
        let col = CollectionDecl {
            name: "Msg".to_string(),
            fields: vec![],
            description: None,
            is_pub: false,
            has_spread: false,
            span: sp(),
        };
        #[cfg(feature = "postgres")]
        assert!(
            collection_to_ddl(BackendKind::Postgres, &col).is_err(),
            "Postgres should be rejected"
        );
        #[cfg(feature = "mysql")]
        assert!(
            collection_to_ddl(BackendKind::MySql, &col).is_err(),
            "MySQL should be rejected"
        );
    }

    // --- type_expr_to_vox_type ---

    #[test]
    fn type_expr_to_vox_type_named() {
        let ty = TypeExpr::Named {
            name: "str".to_string(),
            span: sp(),
        };
        assert_eq!(type_expr_to_vox_type(&ty), "str");
    }

    #[test]
    fn type_expr_to_vox_type_generic_single_arg() {
        let ty = TypeExpr::Generic {
            name: "List".to_string(),
            args: vec![TypeExpr::Named {
                name: "int".to_string(),
                span: sp(),
            }],
            span: sp(),
        };
        assert_eq!(type_expr_to_vox_type(&ty), "List[int]");
    }

    #[test]
    fn type_expr_to_vox_type_function_with_params() {
        let ty = TypeExpr::Function {
            params: vec![
                TypeExpr::Named {
                    name: "str".to_string(),
                    span: sp(),
                },
                TypeExpr::Named {
                    name: "int".to_string(),
                    span: sp(),
                },
            ],
            return_type: Box::new(TypeExpr::Named {
                name: "bool".to_string(),
                span: sp(),
            }),
            span: sp(),
        };
        assert_eq!(type_expr_to_vox_type(&ty), "fn(str, int) -> bool");
    }

    #[test]
    fn type_expr_to_vox_type_tuple() {
        let ty = TypeExpr::Tuple {
            elements: vec![
                TypeExpr::Named {
                    name: "int".to_string(),
                    span: sp(),
                },
                TypeExpr::Named {
                    name: "str".to_string(),
                    span: sp(),
                },
            ],
            span: sp(),
        };
        assert_eq!(type_expr_to_vox_type(&ty), "(int, str)");
    }

    #[test]
    fn type_expr_to_vox_type_unit() {
        let ty = TypeExpr::Unit { span: sp() };
        assert_eq!(type_expr_to_vox_type(&ty), "Unit");
    }

    #[test]
    fn type_expr_to_vox_type_infer() {
        let ty = TypeExpr::Infer { span: sp() };
        assert_eq!(type_expr_to_vox_type(&ty), "_");
    }

    #[test]
    fn type_expr_to_vox_type_decimal() {
        let ty = TypeExpr::Decimal { span: sp() };
        assert_eq!(type_expr_to_vox_type(&ty), "dec");
    }

    // --- to_snake_case ---

    #[test]
    fn to_snake_case_converts_camel_case() {
        assert_eq!(to_snake_case("UserProfile"), "user_profile");
        assert_eq!(to_snake_case("MyHttpRequest"), "my_http_request");
    }

    #[test]
    fn to_snake_case_already_snake_is_unchanged() {
        assert_eq!(to_snake_case("user_profile"), "user_profile");
        assert_eq!(to_snake_case("task"), "task");
    }

    #[test]
    fn to_snake_case_non_alphanumeric_becomes_single_underscore() {
        assert_eq!(to_snake_case("some-field"), "some_field");
        assert_eq!(to_snake_case("some field"), "some_field");
    }

    #[test]
    fn to_snake_case_consecutive_separators_produce_no_double_underscore() {
        assert_eq!(to_snake_case("foo--bar"), "foo_bar");
    }

    #[test]
    fn to_snake_case_trims_leading_and_trailing_underscores() {
        assert_eq!(to_snake_case("_Task_"), "task");
    }

    #[test]
    fn to_snake_case_digits_break_on_following_uppercase() {
        assert_eq!(to_snake_case("myField2Name"), "my_field2_name");
    }
}
