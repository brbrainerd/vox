#[cfg(test)]
mod decorator_tests {

    use vox_ast::decl::{Decl, FnDecl, Module};
    use vox_ast::span::Span;
    use vox_typeck::diagnostics::Severity;
    use vox_typeck::{typecheck_module, Diagnostic};

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[allow(dead_code)]
    fn has_error(diags: &[Diagnostic]) -> bool {
        diags.iter().any(|d| d.severity == Severity::Error)
    }

    fn error_messages(diags: &[Diagnostic]) -> Vec<String> {
        diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone())
            .collect()
    }
    #[test]
    fn a099_deprecated_fn_usage_produces_warning() {
        // Calling a @deprecated function should produce a Warning diagnostic.
        // We set up: deprecated_fn() and a caller that references it.
        use vox_ast::decl::{Decl, FnDecl};
        use vox_ast::expr::Expr;
        use vox_ast::stmt::Stmt;

        let deprecated_fn = Decl::Function(FnDecl {
            name: "old_fn".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_deprecated: true,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: false,
            is_pub: true,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let caller_fn = Decl::Function(FnDecl {
            name: "caller".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Expr {
                expr: Expr::Call {
                    callee: Box::new(Expr::Ident {
                        name: "old_fn".to_string(),
                        span: dummy_span(),
                    }),
                    args: vec![],
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            is_async: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: false,
            is_pub: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![deprecated_fn, caller_fn],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.message.contains("deprecated"))
            .collect();
        assert!(
            !warnings.is_empty(),
            "Expected a deprecation warning, got: {:?}",
            diags
        );
    }

    #[test]
    fn a100_pure_fn_registered_correctly() {
        // @pure function should be registered without errors; no warnings
        use vox_ast::decl::{Decl, FnDecl};

        let pure_fn = Decl::Function(FnDecl {
            name: "clean".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: true,
            is_pub: true,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![pure_fn],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        assert!(
            errors.is_empty(),
            "Pure function should have no errors: {:?}",
            errors
        );
    }

    #[test]
    fn a101_deprecated_and_pure_fn_produces_warning_on_call() {
        // A fn with both @deprecated and @pure should still emit deprecation warning
        use vox_ast::decl::{Decl, FnDecl};
        use vox_ast::expr::Expr;
        use vox_ast::stmt::Stmt;

        let combo_fn = Decl::Function(FnDecl {
            name: "combo".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_deprecated: true,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: true,
            is_pub: true,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let usage_fn = Decl::Function(FnDecl {
            name: "usage".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Expr {
                expr: Expr::Call {
                    callee: Box::new(Expr::Ident {
                        name: "combo".to_string(),
                        span: dummy_span(),
                    }),
                    args: vec![],
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            is_async: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: false,
            is_pub: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![combo_fn, usage_fn],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.message.contains("deprecated"))
            .collect();
        assert!(
            !warnings.is_empty(),
            "Expected deprecation warning for combo fn, got: {:?}",
            diags
        );
    }

    #[test]
    fn a102_deprecated_table_usage_produces_warning() {
        use vox_ast::decl::{Decl, TableDecl};
        use vox_ast::expr::Expr;
        use vox_ast::stmt::Stmt;

        let deprecated_table = Decl::Table(TableDecl {
            name: "OldTable".to_string(),
            fields: vec![],
            description: None,
            is_pub: true,
            is_deprecated: true,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            span: dummy_span(),
        });

        let usage_fn = Decl::Function(FnDecl {
            name: "usage".to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Expr {
                expr: Expr::Ident {
                    name: "OldTable".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            is_async: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: false,
            is_pub: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![deprecated_table, usage_fn],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.message.contains("deprecated"))
            .collect();
        assert!(
            !warnings.is_empty(),
            "Expected deprecation warning for OldTable, got: {:?}",
            diags
        );
    }
    #[test]
    fn search_index_unknown_table_is_error() {
        use vox_ast::decl::SearchIndexDecl;

        let module = Module {
            declarations: vec![Decl::SearchIndex(SearchIndexDecl {
                table_name: "NonExistent".into(),
                index_name: "idx".into(),
                search_field: "title".into(),
                filter_fields: vec![],

                span: dummy_span(),
            })],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        assert!(
            errors.iter().any(|e| e.contains("unknown table")),
            "Expected unknown table error, got: {:?}",
            errors
        );
    }

    #[test]
    fn search_index_non_str_field_is_error() {
        use vox_ast::decl::{SearchIndexDecl, TableDecl, TableField};
        use vox_ast::types::TypeExpr;

        let table = Decl::Table(TableDecl {
            name: "Post".into(),
            fields: vec![TableField {
                name: "score".into(),
                type_ann: TypeExpr::Named {
                    name: "int".into(),
                    span: dummy_span(),
                },
                description: None,
                span: dummy_span(),
            }],
            description: None,
            is_pub: true,
            is_deprecated: false,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            span: dummy_span(),
        });

        let search_idx = Decl::SearchIndex(SearchIndexDecl {
            table_name: "Post".into(),
            index_name: "search_score".into(),
            search_field: "score".into(), // int, not str!
            filter_fields: vec![],

            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![table, search_idx],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        assert!(
            errors.iter().any(|e| e.contains("must be type 'str'")),
            "Expected str-type error, got: {:?}",
            errors
        );
    }

    #[test]
    fn search_index_str_field_is_ok() {
        use vox_ast::decl::{SearchIndexDecl, TableDecl, TableField};
        use vox_ast::types::TypeExpr;

        let table = Decl::Table(TableDecl {
            name: "Post".into(),
            fields: vec![TableField {
                name: "title".into(),
                type_ann: TypeExpr::Named {
                    name: "str".into(),
                    span: dummy_span(),
                },
                description: None,
                span: dummy_span(),
            }],
            description: None,
            is_pub: true,
            is_deprecated: false,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            span: dummy_span(),
        });

        let search_idx = Decl::SearchIndex(SearchIndexDecl {
            table_name: "Post".into(),
            index_name: "search_title".into(),
            search_field: "title".into(),
            filter_fields: vec![],

            span: dummy_span(),
        });

        let module = Module {
            declarations: vec![table, search_idx],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        assert!(
            errors.is_empty(),
            "Expected no errors for valid search index, got: {:?}",
            errors
        );
    }
}
