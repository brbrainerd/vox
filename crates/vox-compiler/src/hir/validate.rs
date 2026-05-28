//! HIR structural validation — invariants that should hold after lowering.
//!
//! Emits [`HirValidationError`] values; the CLI pipeline maps these to
//! [`crate::typeck::diagnostics::Diagnostic`] with category [`crate::typeck::diagnostics::DiagnosticCategory::HirInvariant`].

use crate::ast::span::Span;
use crate::hir::*;

/// A HIR validation diagnostic (span + message).
#[derive(Debug)]
pub struct HirValidationError {
    pub message: String,
    pub span: Span,
    pub correction_hint: Option<String>,
}

/// Validate structural invariants of a [`HirModule`].
/// Returns a list of validation errors (empty = no structural violations reported here).
#[must_use]
pub fn validate_module(module: &HirModule) -> Vec<HirValidationError> {
    let mut errors = Vec::new();

    for f in &module.functions {
        validate_fn(f, "function", &mut errors);
    }
    for f in &module.tests {
        validate_fn(f, "test", &mut errors);
    }

    for s in &module.endpoint_fns {
        let label = match s.kind {
            crate::hir::HirEndpointKind::Server => "server fn",
            crate::hir::HirEndpointKind::Query => "@query fn",
            crate::hir::HirEndpointKind::Mutation => "@mutation fn",
        };
        validate_name_and_params(&s.name, &s.params, s.span, label, &mut errors);
        if s.route_path.is_empty() {
            let (hint, kind_str) = match s.kind {
                crate::hir::HirEndpointKind::Server => (
                    "@server fn must declare a route, e.g. @server fn foo(\"/path\")",
                    "@server fn",
                ),
                crate::hir::HirEndpointKind::Query => (
                    "@query fn must declare a route, e.g. @query fn foo(\"/path\")",
                    "@query fn",
                ),
                crate::hir::HirEndpointKind::Mutation => (
                    "@mutation fn must declare a route, e.g. @mutation fn foo(\"/path\")",
                    "@mutation fn",
                ),
            };
            errors.push(HirValidationError {
                message: format!("{kind_str} route_path is empty"),
                span: s.span,
                correction_hint: Some(hint.into()),
            });
        }
    }
    for m in &module.mcp_tools {
        validate_fn(&m.func, "mcp tool", &mut errors);
    }
    let mut seen_resource_uris = std::collections::HashSet::<&str>::new();
    for m in &module.mcp_resources {
        validate_fn(&m.func, "mcp resource", &mut errors);
        if m.uri.trim().is_empty() {
            errors.push(HirValidationError {
                message: "mcp resource URI must not be empty".into(),
                span: m.func.span,
                correction_hint: Some(
                    "@mcp.resource requires a URI, e.g. @mcp.resource(\"mcp://my-resource\")"
                        .into(),
                ),
            });
        }
        if !seen_resource_uris.insert(m.uri.as_str()) {
            errors.push(HirValidationError {
                message: format!("duplicate @mcp.resource URI: {}", m.uri),
                span: m.func.span,
                correction_hint: Some(format!(
                    "Use a unique URI for each @mcp.resource; '{}' is already declared elsewhere",
                    m.uri
                )),
            });
        }
        if !m.func.params.is_empty() {
            errors.push(HirValidationError {
                message: "mcp resource function must take no parameters (MCP resources/read supplies only `uri`)".into(),
                span: m.func.span,
                correction_hint: Some("Remove parameters from the @mcp.resource function; the URI is the only identifier".into()),
            });
        }
    }

    for c in &module.components {
        validate_name_and_params(
            &c.name,
            &c.params,
            c.span,
            "reactive component",
            &mut errors,
        );
    }

    for table in &module.tables {
        if table.name.is_empty() {
            errors.push(HirValidationError {
                message: "Table name is empty".into(),
                span: table.span,
                correction_hint: Some(
                    "Define a name for the table, e.g. @table User { ... }".into(),
                ),
            });
        }
        for field in &table.fields {
            if field.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty field name in table '{}'", table.name),
                    span: field.span,
                    correction_hint: Some("All table fields must have a name".into()),
                });
            }
        }
    }

    for t in &module.types {
        if t.name.is_empty() {
            errors.push(HirValidationError {
                message: "Type name is empty".into(),
                span: t.span,
                correction_hint: Some("Define a name for the type, e.g. type MyType = ...".into()),
            });
        }
        for v in &t.variants {
            if v.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty variant name in type '{}'", t.name),
                    span: v.span,
                    correction_hint: Some("All variants in an ADT must have a name".into()),
                });
            }
            for (fname, _) in &v.fields {
                if fname.is_empty() {
                    errors.push(HirValidationError {
                        message: format!(
                            "Empty field name in variant '{}' of type '{}'",
                            v.name, t.name
                        ),
                        span: v.span,
                        correction_hint: Some("All variant fields must have a name".into()),
                    });
                }
            }
        }
    }

    for idx in &module.indexes {
        if idx.table_name.is_empty() {
            errors.push(HirValidationError {
                message: "index table_name is empty".into(),
                span: idx.span,
                correction_hint: Some(
                    "Specify the table for the index, e.g. @index MyTable.idx_name on (field)"
                        .into(),
                ),
            });
        }
        if idx.index_name.is_empty() {
            errors.push(HirValidationError {
                message: format!("index name is empty (table '{}')", idx.table_name),
                span: idx.span,
                correction_hint: Some(
                    "Provide a name for the index, e.g. MyTable.my_index_name".into(),
                ),
            });
        }
    }

    for c in &module.collections {
        if c.name.is_empty() {
            errors.push(HirValidationError {
                message: "collection name is empty".into(),
                span: c.span,
                correction_hint: Some(
                    "Define a name for the collection, e.g. collection MyCollection { ... }".into(),
                ),
            });
        }
        for field in &c.fields {
            if field.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty field name in collection '{}'", c.name),
                    span: field.span,
                    correction_hint: Some("All collection fields must have a name".into()),
                });
            }
        }
    }

    for v in &module.vector_indexes {
        if v.table_name.is_empty() {
            errors.push(HirValidationError {
                message: "vector index table_name is empty".into(),
                span: v.span,
                correction_hint: Some("Specify the table for the vector index".into()),
            });
        }
        if v.index_name.is_empty() {
            errors.push(HirValidationError {
                message: format!("vector index name is empty (table '{}')", v.table_name),
                span: v.span,
                correction_hint: Some("Provide a name for the vector index".into()),
            });
        }
        if v.column.is_empty() {
            errors.push(HirValidationError {
                message: format!("vector index column is empty ('{}')", v.index_name),
                span: v.span,
                correction_hint: Some("Specify the column to index for vector search".into()),
            });
        }
    }

    for s in &module.search_indexes {
        if s.table_name.is_empty() {
            errors.push(HirValidationError {
                message: "search index table_name is empty".into(),
                span: s.span,
                correction_hint: Some("Specify the table for the search index".into()),
            });
        }
        if s.index_name.is_empty() {
            errors.push(HirValidationError {
                message: format!("search index name is empty (table '{}')", s.table_name),
                span: s.span,
                correction_hint: Some("Provide a name for the search index".into()),
            });
        }
        if s.search_field.is_empty() {
            errors.push(HirValidationError {
                message: format!("search index field is empty ('{}')", s.index_name),
                span: s.span,
                correction_hint: Some("Specify the field to index for full-text search".into()),
            });
        }
    }

    for ri in &module.rust_imports {
        if ri.crate_name.trim().is_empty() {
            errors.push(HirValidationError {
                message: "rust import crate name is empty".into(),
                span: ri.span,
                correction_hint: Some("Specify the crate name, e.g. import rust:tokio".into()),
            });
        }
        if ri.alias.trim().is_empty() {
            errors.push(HirValidationError {
                message: format!("rust import alias is empty for crate '{}'", ri.crate_name),
                span: ri.span,
                correction_hint: Some("Provide an alias for the rust import".into()),
            });
        }
        if ri.path.is_some() && ri.git.is_some() {
            errors.push(HirValidationError {
                message: format!(
                    "rust import '{}' has both path and git source configured",
                    ri.crate_name
                ),
                span: ri.span,
                correction_hint: Some(
                    "Use either 'path' or 'git', not both for a single import".into(),
                ),
            });
        }
    }

    errors
}

fn validate_fn(f: &HirFn, kind: &str, errors: &mut Vec<HirValidationError>) {
    validate_name_and_params(&f.name, &f.params, f.span, kind, errors);
    if let Some(iv) = &f.schedule_interval
        && iv.trim().is_empty()
    {
        errors.push(HirValidationError {
            message: format!(
                "{kind} `{}`: @scheduled interval must be a non-empty string",
                f.name
            ),
            span: f.span,
            correction_hint: Some(r#"use @scheduled("1h") or a cron-like string"#.into()),
        });
    }
}

fn validate_name_and_params(
    name: &str,
    params: &[HirParam],
    span: Span,
    kind: &str,
    errors: &mut Vec<HirValidationError>,
) {
    if name.is_empty() {
        errors.push(HirValidationError {
            message: format!("{kind} name is empty"),
            span,
            correction_hint: Some(format!("Provide a name for this {kind}")),
        });
    }
    for p in params {
        if p.name.is_empty() {
            errors.push(HirValidationError {
                message: format!("Empty parameter name in {kind} '{name}'"),
                span: p.span,
                correction_hint: Some("All parameters must have a valid identifier name".into()),
            });
        }
    }
}
