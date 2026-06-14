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
                    "route_path is synthesized during lowering; this indicates an internal lowering bug",
                    "@server fn",
                ),
                crate::hir::HirEndpointKind::Query => (
                    "route_path is synthesized during lowering; this indicates an internal lowering bug",
                    "@query fn",
                ),
                crate::hir::HirEndpointKind::Mutation => (
                    "route_path is synthesized during lowering; this indicates an internal lowering bug",
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

// ---------------------------------------------------------------------------
// Adversarial tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod semcov_wave12_tests {
    use super::*;
    use crate::hir::{
        DefId, HirMcpResource, HirParam, HirRustImport, HirTable, HirTableField, HirType,
        HirVectorIndex,
    };
    use crate::hir::HirModule;

    // -----------------------------------------------------------------------
    // Helper: zero span
    // -----------------------------------------------------------------------

    fn zspan() -> Span {
        Span { start: 0, end: 0 }
    }

    // -----------------------------------------------------------------------
    // Helper: minimal valid HirFn (named, no params)
    // -----------------------------------------------------------------------

    fn named_fn(name: &str) -> crate::hir::HirFn {
        crate::hir::HirFn {
            id: DefId(0),
            name: name.to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            capabilities: vec![],
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: zspan(),
            inference_model: None,
            training_step: false,
            distributed_train: None,
        }
    }

    // -----------------------------------------------------------------------
    // Error-path: empty-named function triggers fn-name diagnostic
    // -----------------------------------------------------------------------

    #[test]
    fn empty_fn_name_produces_error() {
        // Catches: validate_name_and_params skipping the name check when the
        // kind string is "function", causing anonymous functions to slip through.
        let mut m = HirModule::default();
        m.functions.push(named_fn(""));
        let errs = validate_module(&m);
        assert!(
            errs.iter().any(|e| e.message.contains("function name is empty")),
            "expected 'function name is empty' diagnostic, got: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Error-path: empty param name in a named function
    // -----------------------------------------------------------------------

    #[test]
    fn empty_param_name_in_named_fn_produces_error() {
        // Catches: the param loop in validate_name_and_params only firing when
        // the fn name is also empty (incorrect && vs || condition).
        let mut f = named_fn("my_fn");
        f.params.push(HirParam {
            id: DefId(1),
            name: "".to_string(),
            type_ann: None,
            default: None,
            span: zspan(),
        });
        let mut m = HirModule::default();
        m.functions.push(f);
        let errs = validate_module(&m);
        assert!(
            errs.iter().any(|e| e.message.contains("Empty parameter name")),
            "expected 'Empty parameter name' diagnostic, got: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Error-path: mcp resource with params must be rejected
    // -----------------------------------------------------------------------

    #[test]
    fn mcp_resource_with_params_is_rejected() {
        // Catches: the !m.func.params.is_empty() guard in validate_module being
        // accidentally inverted (params forbidden but no error emitted).
        let mut f = named_fn("get_resource");
        f.params.push(HirParam {
            id: DefId(2),
            name: "extra".to_string(),
            type_ann: None,
            default: None,
            span: zspan(),
        });
        let mut m = HirModule::default();
        m.mcp_resources.push(HirMcpResource {
            uri: "mcp://my-resource".to_string(),
            description: "test".to_string(),
            func: f,
        });
        let errs = validate_module(&m);
        assert!(
            errs.iter()
                .any(|e| e.message.contains("mcp resource function must take no parameters")),
            "mcp resource with params should error, got: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Error-path: duplicate mcp resource URIs must both be reported
    // -----------------------------------------------------------------------

    #[test]
    fn duplicate_mcp_resource_uri_produces_error() {
        // Catches: seen_resource_uris dedup check only inserting without ever
        // checking the return value, so duplicates pass silently.
        let mut m = HirModule::default();
        for i in 0..2 {
            m.mcp_resources.push(HirMcpResource {
                uri: "mcp://same-uri".to_string(),
                description: format!("resource {i}"),
                func: named_fn(&format!("handler_{i}")),
            });
        }
        let errs = validate_module(&m);
        assert!(
            errs.iter()
                .any(|e| e.message.contains("duplicate @mcp.resource URI")),
            "expected duplicate URI diagnostic, got: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Invariant: validate_module is idempotent (pure, no mutation)
    // -----------------------------------------------------------------------

    #[test]
    fn validate_module_is_idempotent() {
        // Catches: validate_module accumulating state across calls (e.g. mutating
        // the module or a thread-local, so error counts differ on re-runs).
        let mut m = HirModule::default();
        m.functions.push(named_fn(""));  // triggers a function-name error
        m.tables.push(HirTable {
            id: DefId(0),
            name: "".to_string(),
            fields: vec![],
            primary_key: None,
            is_extern: false,
            source: None,
            is_pub: false,
            is_deprecated: false,
            span: zspan(),
        });
        let first = validate_module(&m).len();
        let second = validate_module(&m).len();
        assert_eq!(
            first, second,
            "validate_module must be pure: first={first}, second={second}"
        );
    }

    // -----------------------------------------------------------------------
    // State: rust import with both path and git simultaneously is rejected
    // -----------------------------------------------------------------------

    #[test]
    fn rust_import_with_path_and_git_produces_error() {
        // Catches: the path+git mutual-exclusion check being guarded by an `&&`
        // that also requires crate_name to be empty, making the check unreachable
        // for well-named imports.
        let mut m = HirModule::default();
        m.rust_imports.push(HirRustImport {
            crate_name: "tokio".to_string(),
            alias: "tokio".to_string(),
            version: None,
            path: Some("../tokio".to_string()),
            git: Some("https://github.com/tokio-rs/tokio".to_string()),
            rev: None,
            span: zspan(),
        });
        let errs = validate_module(&m);
        assert!(
            errs.iter()
                .any(|e| e.message.contains("both path and git source configured")),
            "expected path+git conflict diagnostic, got: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary: empty table field name inside a named table
    // -----------------------------------------------------------------------

    fn dummy_hir_type() -> HirType {
        HirType::Named("String".to_string())
    }

    #[test]
    fn empty_table_field_name_in_named_table() {
        // Catches: the field-name loop iterating over the wrong slice or
        // short-circuiting after the first error so subsequent empty fields
        // are silently ignored.
        let mut m = HirModule::default();
        m.tables.push(HirTable {
            id: DefId(0),
            name: "Users".to_string(),
            fields: vec![
                HirTableField { name: "id".to_string(), type_ann: dummy_hir_type(), span: zspan() },
                HirTableField { name: "".to_string(), type_ann: dummy_hir_type(), span: zspan() },
                HirTableField { name: "".to_string(), type_ann: dummy_hir_type(), span: zspan() },
            ],
            primary_key: None,
            is_extern: false,
            source: None,
            is_pub: false,
            is_deprecated: false,
            span: zspan(),
        });
        let errs = validate_module(&m);
        let field_errs = errs
            .iter()
            .filter(|e| e.message.contains("Empty field name in table"))
            .count();
        assert_eq!(
            field_errs, 2,
            "expected 2 empty-field-name diagnostics, got {field_errs}: {errs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary: vector index with all three required fields empty
    // -----------------------------------------------------------------------

    #[test]
    fn vector_index_all_fields_empty_produces_three_errors() {
        // Catches: the three-check sequence for vector indexes short-circuiting
        // after the first error so callers never see a complete error list.
        let mut m = HirModule::default();
        m.vector_indexes.push(HirVectorIndex {
            table_name: "".to_string(),
            index_name: "".to_string(),
            column: "".to_string(),
            dimensions: 0,
            filter_fields: vec![],
            span: zspan(),
        });
        let errs = validate_module(&m);
        let vi_errs = errs
            .iter()
            .filter(|e| {
                e.message.contains("vector index table_name is empty")
                    || e.message.contains("vector index name is empty")
                    || e.message.contains("vector index column is empty")
            })
            .count();
        assert_eq!(
            vi_errs, 3,
            "all three vector-index fields empty should produce 3 diagnostics, got {vi_errs}: {errs:?}"
        );
    }
}
