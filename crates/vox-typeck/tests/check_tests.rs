#[cfg(test)]
mod check_tests {

    use vox_ast::decl::{Decl, FnDecl, Module};
    use vox_ast::expr::Expr;
    use vox_ast::pattern::Pattern;
    use vox_ast::span::Span;
    use vox_ast::stmt::Stmt;
    use vox_typeck::diagnostics::Severity;
    use vox_typeck::{typecheck_module, Diagnostic};

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn module_with_fn(name: &str, body: Vec<Stmt>) -> Module {
        Module {
            declarations: vec![Decl::Function(FnDecl {
                name: name.to_string(),
                generics: vec![],
                params: vec![],
                return_type: None,
                body,
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
            })],
            span: dummy_span(),
        }
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
    fn a090_for_loop_variable_in_scope() {

        // `for x in [1, 2]: x` — should NOT produce an error for `x`
        let body = vec![Stmt::Expr {
            expr: Expr::For {
                binding: Pattern::Ident {
                    name: "x".into(),
                    span: dummy_span(),
                },
                iterable: Box::new(Expr::ListLit {
                    elements: vec![
                        Expr::IntLit {
                            value: 1,
                            span: dummy_span(),
                        },
                        Expr::IntLit {
                            value: 2,
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                key: None,
                body: Box::new(Expr::Ident {
                    name: "x".into(),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        // x should be found — no "undefined variable x" error
        assert!(
            !errors
                .iter()
                .any(|m| m.contains("Undefined") && m.contains("\"x\"")),
            "Expected no undefined-variable error for loop var x, got: {:?}",
            errors
        );
    }

    #[test]
    fn a091_for_loop_variable_elem_type() {
        // `for x in [1]: x + 1` — should resolve x without "Undefined" error
        let body = vec![Stmt::Expr {
            expr: Expr::For {
                binding: Pattern::Ident {
                    name: "x".into(),
                    span: dummy_span(),
                },
                iterable: Box::new(Expr::ListLit {
                    elements: vec![Expr::IntLit {
                        value: 1,
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                }),
                key: None,
                body: Box::new(Expr::Binary {
                    op: vox_ast::expr::BinOp::Add,
                    left: Box::new(Expr::Ident {
                        name: "x".into(),
                        span: dummy_span(),
                    }),
                    right: Box::new(Expr::IntLit {
                        value: 1,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        let errors = error_messages(&diags);
        // x should resolve from loop binding — no undefined-variable error for it
        assert!(
            !errors
                .iter()
                .any(|m| m.contains("Undefined") && m.contains("\"x\"")),
            "Expected no undefined error for loop var x, got: {:?}",
            errors
        );
    }

    #[test]
    fn a096_calling_non_callable_produces_error() {
        // `42()` — should produce "not callable" error
        let body = vec![Stmt::Expr {
            expr: Expr::Call {
                callee: Box::new(Expr::IntLit {
                    value: 42,
                    span: dummy_span(),
                }),
                args: vec![],
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        assert!(
            error_messages(&diags)
                .iter()
                .any(|m| m.to_lowercase().contains("callable")),
            "Expected 'not callable' error, got: {:?}",
            error_messages(&diags)
        );
    }

    #[test]
    fn a097_undefined_variable_produces_error() {
        // Reference `undefined_var_xyz` — should produce "Undefined" error
        let body = vec![Stmt::Expr {
            expr: Expr::Ident {
                name: "undefined_var_xyz".into(),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        assert!(
            error_messages(&diags).iter().any(|m| {
                m.contains("Undefined")
                    || m.contains("undefined_var_xyz")
                    || m.contains("not found")
            }),
            "Expected undefined variable error, got: {:?}",
            error_messages(&diags)
        );
    }

    #[test]
    fn a092_lambda_wrong_arg_type() {
        let lambda = Expr::Lambda {
            params: vec![vox_ast::expr::Param {
                name: "x".into(),
                type_ann: Some(vox_ast::types::TypeExpr::Named {
                    name: "int".into(),
                    span: dummy_span(),
                }),
                default: None,
                span: dummy_span(),
            }],
            return_type: None,
            body: Box::new(Expr::Ident {
                name: "x".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        let body = vec![Stmt::Expr {
            expr: Expr::Call {
                callee: Box::new(lambda),
                args: vec![vox_ast::expr::Arg {
                    name: None,
                    value: Expr::StringLit {
                        value: "string".into(),
                        span: dummy_span(),
                    },
                }],
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        let errs = error_messages(&diags);
        assert!(
            errs.iter().any(|m| m.to_lowercase().contains("mismatch")
                || m.to_lowercase().contains("cannot unify")),
            "Expected type mismatch for int arg given string, got: {:?}",
            errs
        );
    }

    #[test]
    fn a093_lambda_captures_outer_scope() {
        use vox_ast::pattern::Pattern;
        let stmt1 = Stmt::Let {
            pattern: Pattern::Ident {
                name: "outer".into(),
                span: dummy_span(),
            },
            type_ann: None,
            value: Expr::IntLit {
                value: 42,
                span: dummy_span(),
            },
            mutable: false,
            span: dummy_span(),
        };
        let stmt2 = Stmt::Expr {
            expr: Expr::Lambda {
                params: vec![vox_ast::expr::Param {
                    name: "x".into(),
                    type_ann: Some(vox_ast::types::TypeExpr::Named {
                        name: "int".into(),
                        span: dummy_span(),
                    }),
                    default: None,
                    span: dummy_span(),
                }],
                return_type: None,
                body: Box::new(Expr::Binary {
                    op: vox_ast::expr::BinOp::Add,
                    left: Box::new(Expr::Ident {
                        name: "outer".into(),
                        span: dummy_span(),
                    }),
                    right: Box::new(Expr::Ident {
                        name: "x".into(),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            },
            span: dummy_span(),
        };
        let body = vec![stmt1, stmt2];
        let module = module_with_fn("test", body);
        let diags = typecheck_module(&module, "");
        let errs = error_messages(&diags);
        assert!(
            !errs
                .iter()
                .any(|m| m.contains("Undefined") && m.contains("outer")),
            "Outer variable should be captured, got: {:?}",
            errs
        );
    }

    #[test]
    fn a094_match_arm_variable_bound() {
        use vox_ast::pattern::Pattern;
        let type_decl = Decl::TypeDef(vox_ast::decl::TypeDefDecl {
            name: "Color".into(),
            generics: vec![],
            variants: vec![
                vox_ast::decl::Variant {
                    name: "Red".into(),
                    fields: vec![],
                    literal_value: None,
                    span: dummy_span(),
                },
                vox_ast::decl::Variant {
                    name: "Blue".into(),
                    fields: vec![],
                    literal_value: None,
                    span: dummy_span(),
                },
            ],
            fields: vec![],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            span: dummy_span(),
        });

        let stmt = Stmt::Let {
            pattern: Pattern::Ident {
                name: "val".into(),
                span: dummy_span(),
            },
            type_ann: None,
            value: Expr::Match {
                subject: Box::new(Expr::Ident {
                    name: "Red".into(),
                    span: dummy_span(),
                }),
                arms: vec![
                    vox_ast::expr::MatchArm {
                        pattern: Pattern::Ident {
                            name: "Red".into(),
                            span: dummy_span(),
                        },
                        guard: None,
                        body: Box::new(Expr::Ident {
                            name: "Red".into(),
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    },
                    vox_ast::expr::MatchArm {
                        pattern: Pattern::Ident {
                            name: "Blue".into(),
                            span: dummy_span(),
                        },
                        guard: None,
                        body: Box::new(Expr::Ident {
                            name: "Blue".into(),
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    },
                ],
                span: dummy_span(),
            },
            mutable: false,
            span: dummy_span(),
        };
        let fn_decl = Decl::Function(FnDecl {
            name: "test".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![stmt],
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
            declarations: vec![type_decl, fn_decl],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        assert!(
            !has_error(&diags),
            "Expected no error, got: {:?}",
            error_messages(&diags)
        );
    }

    #[test]
    fn a095_match_exhaustiveness_error() {
        use vox_ast::pattern::Pattern;
        let type_decl = Decl::TypeDef(vox_ast::decl::TypeDefDecl {
            name: "Color".into(),
            generics: vec![],
            variants: vec![
                vox_ast::decl::Variant {
                    name: "Red".into(),
                    fields: vec![],
                    literal_value: None,
                    span: dummy_span(),
                },
                vox_ast::decl::Variant {
                    name: "Blue".into(),
                    fields: vec![],
                    literal_value: None,
                    span: dummy_span(),
                },
            ],
            fields: vec![],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            span: dummy_span(),
        });

        let stmt = Stmt::Let {
            pattern: Pattern::Ident {
                name: "val".into(),
                span: dummy_span(),
            },
            type_ann: None,
            value: Expr::Match {
                subject: Box::new(Expr::Ident {
                    name: "Red".into(),
                    span: dummy_span(),
                }),
                arms: vec![vox_ast::expr::MatchArm {
                    pattern: Pattern::Ident {
                        name: "Red".into(),
                        span: dummy_span(),
                    },
                    guard: None,
                    body: Box::new(Expr::Ident {
                        name: "Red".into(),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }],
                span: dummy_span(),
            },
            mutable: false,
            span: dummy_span(),
        };
        let fn_decl = Decl::Function(FnDecl {
            name: "test".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![stmt],
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
            declarations: vec![type_decl, fn_decl],
            span: dummy_span(),
        };
        let diags = typecheck_module(&module, "");
        let errs = error_messages(&diags);
        assert!(
            errs.iter()
                .any(|m| m.contains("Non-exhaustive match") && m.contains("Blue")),
            "Expected exhaustiveness error for 'Blue', got: {:?}",
            errs
        );
    }

}
