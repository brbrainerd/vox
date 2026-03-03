use vox_ast::span::Span;
use vox_codegen_rust::emit::emit_fn;
use vox_hir::hir::{DefId, HirFn, HirType};

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

#[test]
fn test_deprecated_decorator_codegen() {
    let func = HirFn {
        id: DefId(1),
        name: "old_stuff".to_string(),
        generics: vec![],
        params: vec![],
        return_type: Some(HirType::Unit),
        body: vec![],
        is_component: false,
        is_traced: false,
        is_async: false,
        is_deprecated: true,
        is_pure: false,
        is_llm: false,
        llm_model: None,
        is_layout: false,
        is_pub: true,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    };

    let rust_code = emit_fn(&func, false);
    assert!(rust_code.contains("#[deprecated]"));
    assert!(rust_code.contains("pub fn old_stuff()"));
}

#[test]
fn test_pure_decorator_codegen() {
    let func = HirFn {
        id: DefId(2),
        name: "clean_stuff".to_string(),
        generics: vec![],
        params: vec![],
        return_type: Some(HirType::Unit),
        body: vec![],
        is_component: false,
        is_traced: true,
        is_async: false,
        is_deprecated: false,
        is_pure: true,
        is_llm: false,
        llm_model: None,
        is_layout: false,
        is_pub: true,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    };

    let rust_code = emit_fn(&func, false);
    assert!(rust_code.contains("/* @pure */"));
    assert!(rust_code.contains("pub fn clean_stuff()"));
}

#[test]
fn test_log_mapping_to_tracing() {
    use vox_hir::hir::{HirExpr, HirStmt, HirArg};
    let func = HirFn {
        id: DefId(3),
        name: "test_log".to_string(),
        generics: vec![],
        params: vec![],
        return_type: Some(HirType::Unit),
        body: vec![HirStmt::Expr {
            expr: HirExpr::MethodCall(
                Box::new(HirExpr::Ident("log".to_string(), dummy_span())),
                "info".to_string(),
                vec![
                    HirArg { name: None, value: HirExpr::StringLit("Hello {}".into(), dummy_span()) },
                    HirArg { name: None, value: HirExpr::Ident("name".into(), dummy_span()) },
                ],
                dummy_span(),
            ),
            span: dummy_span(),
        }],
        is_component: false,
        is_traced: false,
        is_async: false,
        is_deprecated: false,
        is_pure: false,
        is_llm: false,
        llm_model: None,
        is_layout: false,
        is_pub: true,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    };

    let rust_code = emit_fn(&func, false);
    assert!(rust_code.contains("tracing::info!(\"Hello {}\", name.clone())"));
}
