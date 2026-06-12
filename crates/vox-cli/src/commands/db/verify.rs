//! Live-schema verification against declared `@table` surfaces (`vox db verify`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use vox_compiler::ast::decl::{Decl, TableDecl};
use vox_compiler::ast::types::TypeExpr;
use vox_sql::schema_model::{IntrospectedSchema, IntrospectedTable};
use vox_sql::type_map::{UnsupportedTypePolicy, vox_type_to_sql};
use vox_sql::{AnySqlBackend, BackendKind};

#[derive(Debug, serde::Serialize)]
struct VerifyFinding {
    severity: &'static str,
    code: &'static str,
    table: String,
    field: Option<String>,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct VerifyReport {
    backend: String,
    declared_table_count: usize,
    live_table_count: usize,
    ok: bool,
    strict: bool,
    findings: Vec<VerifyFinding>,
}

pub async fn verify(
    file: Option<&PathBuf>,
    url: Option<&str>,
    strict: bool,
    compact: bool,
) -> Result<()> {
    let path = file
        .cloned()
        .unwrap_or_else(|| PathBuf::from("src/main.vox"));
    if !path.exists() {
        return Err(anyhow!(
            "No source file found at {}. Run `vox db verify --file <path>` to specify one.",
            path.display()
        ));
    }
    let frontend = crate::pipeline::run_frontend(&path, false).await?;
    let declared_tables = frontend
        .module
        .declarations
        .iter()
        .filter_map(|d| match d {
            Decl::Table(t) => Some(t.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let backend = if let Some(url) = url {
        AnySqlBackend::connect_from_url(url).await?
    } else {
        AnySqlBackend::connect_from_app_env().await?
    };
    let live = backend.introspect_schema().await?;
    let kind = match live.backend.as_str() {
        "postgres" => BackendKind::Postgres,
        "mysql" => BackendKind::MySql,
        _ => BackendKind::Libsql,
    };
    let policy = if strict {
        UnsupportedTypePolicy::Reject
    } else {
        UnsupportedTypePolicy::JsonText
    };

    let findings = compare_tables(&declared_tables, &live, kind, policy, strict);
    let report = VerifyReport {
        backend: live.backend,
        declared_table_count: declared_tables.len(),
        live_table_count: live.tables.len(),
        ok: findings.is_empty(),
        strict,
        findings,
    };
    if compact {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    if report.ok {
        return Ok(());
    }
    Err(anyhow!("schema drift detected (see report above)"))
}

fn compare_tables(
    declared_tables: &[TableDecl],
    live: &IntrospectedSchema,
    backend: BackendKind,
    policy: UnsupportedTypePolicy,
    strict: bool,
) -> Vec<VerifyFinding> {
    let mut findings = Vec::new();
    let mut live_by_name: BTreeMap<String, &IntrospectedTable> = BTreeMap::new();
    for t in &live.tables {
        live_by_name.insert(t.name.clone(), t);
    }

    for decl in declared_tables {
        let live_name = live_table_name(decl);
        let Some(live_table) = live_by_name.get(&live_name) else {
            findings.push(VerifyFinding {
                severity: "error",
                code: "verify.table_missing",
                table: decl.name.clone(),
                field: None,
                message: format!(
                    "Declared table '{}' maps to live table '{}' which was not found",
                    decl.name, live_name
                ),
            });
            continue;
        };
        let mut live_cols = BTreeMap::new();
        for c in &live_table.columns {
            live_cols.insert(c.name.to_ascii_lowercase(), c);
        }
        for f in &decl.fields {
            let key = f.name.to_ascii_lowercase();
            let Some(live_col) = live_cols.get(&key) else {
                findings.push(VerifyFinding {
                    severity: "error",
                    code: "verify.column_missing",
                    table: decl.name.clone(),
                    field: Some(f.name.clone()),
                    message: format!(
                        "Declared field '{}' missing from live table '{}'",
                        f.name, live_name
                    ),
                });
                continue;
            };
            let vox_type = type_expr_to_string(&f.type_ann);
            match vox_type_to_sql(backend, &vox_type, policy) {
                Ok(expected_sql) => {
                    if !type_matches(&expected_sql, &live_col.data_type) {
                        findings.push(VerifyFinding {
                            severity: "error",
                            code: "verify.type_mismatch",
                            table: decl.name.clone(),
                            field: Some(f.name.clone()),
                            message: format!(
                                "Field '{}' type mismatch: expected '{}' but live type is '{}'",
                                f.name, expected_sql, live_col.data_type
                            ),
                        });
                    }
                }
                Err(e) => {
                    if strict {
                        findings.push(VerifyFinding {
                            severity: "error",
                            code: "verify.unsupported_type",
                            table: decl.name.clone(),
                            field: Some(f.name.clone()),
                            message: format!(
                                "Unsupported type mapping for field '{}': {}",
                                f.name, e
                            ),
                        });
                    } else {
                        findings.push(VerifyFinding {
                            severity: "warning",
                            code: "verify.unsupported_type",
                            table: decl.name.clone(),
                            field: Some(f.name.clone()),
                            message: format!(
                                "Unsupported type mapping for field '{}' (non-strict mode): {}",
                                f.name, e
                            ),
                        });
                    }
                }
            }
        }
    }
    findings
}

fn live_table_name(t: &TableDecl) -> String {
    if t.is_extern {
        return t.source.clone().unwrap_or_else(|| to_snake_case(&t.name));
    }
    to_snake_case(&t.name)
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn type_expr_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => name.clone(),
        TypeExpr::Generic { name, args, .. } => {
            let inner = args
                .iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}[{inner}]")
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let p = params
                .iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({p}) -> {}", type_expr_to_string(return_type))
        }
        TypeExpr::Tuple { elements, .. } => {
            let e = elements
                .iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({e})")
        }
        TypeExpr::Unit { .. } => "Unit".to_string(),
        TypeExpr::Infer { .. } => "_".to_string(),
        TypeExpr::Decimal { .. } => "dec".to_string(),
    }
}

fn type_matches(expected: &str, live: &str) -> bool {
    normalize_type(expected) == normalize_type(live)
}

fn normalize_type(ty: &str) -> String {
    ty.to_ascii_lowercase()
        .replace(' ', "")
        .replace("(1)", "")
        .replace("(65,30)", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::decl::TableField;
    use vox_compiler::ast::span::Span;

    fn s() -> Span {
        Span::new(0, 0)
    }

    fn field(name: &str, ty: TypeExpr) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: ty,
            description: None,
            span: s(),
        }
    }

    #[test]
    fn live_table_name_prefers_source_for_extern() {
        let t = TableDecl {
            name: "User".to_string(),
            fields: vec![],
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub: false,
            is_deprecated: false,
            primary_key: Some("id".to_string()),
            is_extern: true,
            source: Some("legacy_users".to_string()),
            span: s(),
        };
        assert_eq!(live_table_name(&t), "legacy_users");
    }

    #[test]
    fn compare_tables_detects_missing_column() {
        let decl = TableDecl {
            name: "Task".to_string(),
            fields: vec![field(
                "title",
                TypeExpr::Named {
                    name: "str".to_string(),
                    span: s(),
                },
            )],
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub: false,
            is_deprecated: false,
            primary_key: None,
            is_extern: false,
            source: None,
            span: s(),
        };
        let live = IntrospectedSchema {
            backend: "sqlite".to_string(),
            tables: vec![IntrospectedTable {
                name: "task".to_string(),
                columns: vec![],
            }],
        };
        let findings = compare_tables(
            &[decl],
            &live,
            BackendKind::Libsql,
            UnsupportedTypePolicy::Reject,
            true,
        );
        assert!(!findings.is_empty());
        assert_eq!(findings[0].code, "verify.column_missing");
    }
}
