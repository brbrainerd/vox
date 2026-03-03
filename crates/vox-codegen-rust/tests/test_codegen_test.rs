use vox_hir::*;
use vox_ast::span::Span;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

#[test]
fn test_codegen_generates_rust_tests() {
    let mut module = HirModule::default();

    // Add a fixture
    module.fixtures.push(HirFn {
        id: DefId(1),
        name: "setup_db".to_string(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
        is_component: false,
        is_async: false,
        is_deprecated: false,
        is_pure: false,
        is_traced: false,
        is_llm: false,
        llm_model: None,
        is_pub: false,
        is_metric: false,
        metric_name: None,
        is_health: false,
        is_layout: false,
        preconditions: vec![],
        span: dummy_span(),
    });

    // Add a test
    module.tests.push(HirFn {
        id: DefId(2),
        name: "test_basic_addition".to_string(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![
            HirStmt::Expr {
                expr: HirExpr::Call(
                    Box::new(HirExpr::Ident("assert_eq".to_string(), dummy_span())),
                    vec![
                        HirArg { name: None, value: HirExpr::IntLit(2, dummy_span()) },
                        HirArg { name: None, value: HirExpr::IntLit(2, dummy_span()) },
                    ],
                    false,
                    dummy_span()
                ),
                span: dummy_span(),
            }
        ],
        is_component: false,
        is_async: false,
        is_deprecated: false,
        is_pure: false,
        is_traced: false,
        is_llm: false,
        llm_model: None,
        is_pub: false,
        is_metric: false,
        metric_name: None,
        is_health: false,
        is_layout: false,
        preconditions: vec![],
        span: dummy_span(),
    });

    // Add a mock
    module.mocks.push(HirMock {
        target: "api_call".to_string(),
        func: HirFn {
            id: DefId(3),
            name: "mock_api_call".to_string(),
            generics: vec![],
            params: vec![],
            return_type: Some(HirType::Named("String".to_string())),
            body: vec![
                HirStmt::Return {
                    value: Some(HirExpr::StringLit("mocked".to_string(), dummy_span())),
                    span: dummy_span(),
                }
            ],
            is_component: false,
            is_async: false,
            is_deprecated: false,
            is_pure: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_pub: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            is_layout: false,
            preconditions: vec![],
            span: dummy_span(),
        },
    });

    let output = vox_codegen_rust::generate(&module, "test_app").unwrap();
    let lib_rs = output.files.get("src/lib.rs").unwrap();

    assert!(lib_rs.contains("#[cfg(test)]"), "Should contain test cfg");
    assert!(lib_rs.contains("pub mod fixtures"), "Should contain fixtures module");
    assert!(lib_rs.contains("pub fn setup_db"), "Should contain setup_db fixture");
    assert!(lib_rs.contains("#[test]"), "Should contain #[test] attribute");
    assert!(lib_rs.contains("pub fn test_basic_addition"), "Should contain test function");
    assert!(lib_rs.contains("fixtures::setup_db()"), "Should call fixture in test");
    assert!(lib_rs.contains("assert_eq!(2, 2)"), "Should emit assert_eq! macro");

    assert!(lib_rs.contains("pub mod mocks"), "Should contain mocks module");
    assert!(lib_rs.contains("pub fn mock_api_call() -> String"), "Should contain mock function");
    assert!(lib_rs.contains("return \"mocked\".to_string();"), "Should contain mock body");
}
