use vox_ast::span::Span;
use vox_codegen_rust::emit::emit_fn;
use vox_hir::hir::{DefId, HirFn, HirType};

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

#[test]
fn test_llm_tracking_with_table() {
    let func = HirFn {
        id: DefId(1),
        name: "test_ai".to_string(),
        generics: vec![],
        params: vec![],
        return_type: Some(HirType::Named("str".to_string())),
        body: vec![],
        is_component: false,
        is_traced: false,
        is_async: true,
        is_deprecated: false,
        is_pure: false,
        is_llm: true,
        llm_model: Some("gpt-4".to_string()),
        is_layout: false,
        is_pub: true,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    };

    // Case 1: No ModelMetric table
    let rust_code_no_table = emit_fn(&func, false);
    assert!(rust_code_no_table.contains("vox_runtime::llm::llm_chat"));
    assert!(!rust_code_no_table.contains("ModelMetric::insert"));

    // Case 2: ModelMetric table exists
    let rust_code_with_table = emit_fn(&func, true);
    assert!(rust_code_with_table.contains("vox_runtime::llm::llm_chat"));
    assert!(rust_code_with_table.contains("ModelMetric::insert("));
    assert!(rust_code_with_table.contains("_metric.ts"));
    assert!(rust_code_with_table.contains("_metric.model.clone()"));
}
