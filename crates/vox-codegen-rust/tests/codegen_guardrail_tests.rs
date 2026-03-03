/// Codegen tests A-102 through A-104: validate generated main.rs content.
///
/// These tests verify that the code generator:
/// - Uses VOX_PORT env var (not hardcoded port 3000) — A-102
/// - Uses VOX_DB_PATH env var (not hardcoded app.db) — A-103
/// - Uses .expect() not .unwrap() on listener/serve — A-104
use vox_ast::span::Span;
use vox_hir::*;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

fn minimal_module() -> HirModule {
    HirModule {
        consts: vec![],
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![],
        tables: vec![],
        indexes: vec![],
        vector_indexes: vec![],
        search_indexes: vec![],
        mcp_tools: vec![],
        traits: vec![],
        impls: vec![],
        queries: vec![],
        mutations: vec![],
        actions: vec![],
        skills: vec![],
        agents: vec![],
        native_agents: vec![],
        scheduled: vec![],
        messages: vec![],
        config_blocks: vec![],
        collections: vec![],
        contexts: vec![],
        hooks: vec![],
        providers: vec![],
        ..Default::default()
    }
}

#[test]
fn a102_generated_main_uses_vox_port_env_var() {
    // Need a route so the listener/port code is emitted
    let mut module = minimal_module();
    module.routes.push(vox_hir::HirRoute {
        method: vox_hir::HirHttpMethod::Get,
        path: "/ping".to_string(),
        return_type: None,
        body: vec![],
        is_deprecated: false,
        params: vec![],
        is_traced: false,
        span: dummy_span(),
    });
    let main_rs = vox_codegen_rust::emit::emit_main(&module, "test_app");

    assert!(
        main_rs.contains("VOX_PORT"),
        "Generated main.rs should read port from VOX_PORT env var, got:\n{}",
        &main_rs[..main_rs.len().min(3000)]
    );
    assert!(
        !main_rs.contains("SocketAddr::from(([127, 0, 0, 1], 3000))"),
        "Generated main.rs should NOT hardcode port 3000"
    );
}

#[test]
fn a103_generated_main_uses_vox_db_path_env_var() {
    // Module needs at least one table AND one route to emit db setup + has_routes path
    let mut module = minimal_module();
    module.tables.push(HirTable {
        id: DefId(1),
        name: "Task".to_string(),
        fields: vec![],
        description: None,
        is_pub: false,
        is_deprecated: false,
        span: dummy_span(),
    });
    module.routes.push(vox_hir::HirRoute {
        method: vox_hir::HirHttpMethod::Get,
        path: "/ping".to_string(),
        return_type: None,
        body: vec![],
        is_deprecated: false,
        params: vec![],
        is_traced: false,
        span: dummy_span(),
    });
    let main_rs = vox_codegen_rust::emit::emit_main(&module, "test_app");

    assert!(
        main_rs.contains("VOX_DB_PATH"),
        "Generated main.rs should read db path from VOX_DB_PATH env var, got:\n{}",
        &main_rs[..main_rs.len().min(3000)]
    );
    assert!(
        main_rs.contains("DEFAULT_DB_PATH"),
        "Generated main.rs should use DEFAULT_DB_PATH constant for db fallback"
    );
    assert!(
        !main_rs.contains("unwrap_or_else(|_| \"app.db\""),
        "Generated main.rs should NOT inline 'app.db' literal in db_path assignment"
    );
}

#[test]
fn a104_generated_main_uses_expect_not_unwrap_on_listener() {
    // Need at least one route to trigger route setup and listener code
    let mut module = minimal_module();
    module.routes.push(vox_hir::HirRoute {
        method: vox_hir::HirHttpMethod::Get,
        path: "/slow".to_string(),
        params: vec![],
        return_type: None,
        body: vec![],
        is_deprecated: false,
        is_traced: false,
        span: dummy_span(),
    });
    let main_rs = vox_codegen_rust::emit::emit_main(&module, "test_app");

    // Verify no bare .unwrap() on the critical listener/serve lines
    assert!(
        !main_rs.contains("bind(addr).await.unwrap()"),
        "Generated code should not use .unwrap() on TcpListener::bind"
    );
    assert!(
        !main_rs.contains("serve(listener, app).await.unwrap()"),
        "Generated code should not use .unwrap() on axum::serve"
    );
    assert!(
        main_rs.contains(".expect("),
        "Generated code should use .expect() for error handling on listener/serve, got:\n{}",
        &main_rs[..main_rs.len().min(3000)]
    );
}

/// Codegen rejects ListComprehension and Jsx with a clear error (compile-time failure).
#[test]
fn codegen_rejects_list_comprehension() {
    use vox_hir::{HirExpr, HirPattern, HirStmt};

    let mut module = minimal_module();
    module.functions.push(HirFn {
        id: DefId(0),
        name: "foo".to_string(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![HirStmt::Expr {
            expr: HirExpr::ListComprehension {
                expr: Box::new(HirExpr::Ident("x".to_string(), dummy_span())),
                binding: HirPattern::Ident("x".to_string(), dummy_span()),
                iterable: Box::new(HirExpr::ListLit(vec![], dummy_span())),
                condition: None,
                span: dummy_span(),
            },
            span: dummy_span(),
        }],
        is_component: false,
        is_traced: false,
        is_llm: false,
        llm_model: None,
        is_async: false,
        is_deprecated: false,
        is_pure: false,
        is_layout: false,
        is_pub: false,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    });

    let result = vox_codegen_rust::emit::generate(&module, "test");
    assert!(
        result.is_err(),
        "generate should fail for ListComprehension"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("List comprehension") || err_msg.contains("not implemented"),
        "error message should mention List comprehension, got: {}",
        err_msg
    );
}

/// Generated table insert uses map_err for JSON serialization, not unwrap.
#[test]
fn table_insert_uses_map_err_not_unwrap() {
    let mut module = minimal_module();
    module.tables.push(HirTable {
        id: DefId(1),
        name: "Item".to_string(),
        fields: vec![HirTableField {
            name: "item".to_string(),
            type_ann: HirType::Named("serde_json::Value".to_string()),
            description: None,
            span: dummy_span(),
        }],
        is_pub: false,
        description: None,
        is_deprecated: false,
        span: dummy_span(),
    });

    let output = vox_codegen_rust::emit::generate(&module, "test").unwrap();
    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs");
    assert!(
        lib_rs.contains("map_err")
            && !lib_rs.contains("serde_json::to_string(&item.data).unwrap()"),
        "table insert should use map_err for JSON serialization, not unwrap"
    );
}

#[test]
fn codegen_maps_assert_to_assert_eq() {
    let mut module = minimal_module();

    use vox_hir::{HirBinOp, HirExpr, HirStmt};

    // @test fn check_math() { assert(1 + 1 == 2) }
    module.tests.push(HirFn {
        id: DefId(100),
        name: "check_math".to_string(),
        generics: vec![],
        params: vec![],
        return_type: None,
        body: vec![HirStmt::Expr {
            expr: HirExpr::Call(
                Box::new(HirExpr::Ident("assert".to_string(), dummy_span())),
                vec![vox_hir::HirArg {
                    name: None,
                    value: HirExpr::Binary(
                        HirBinOp::Is,
                        Box::new(HirExpr::Binary(
                            HirBinOp::Add,
                            Box::new(HirExpr::IntLit(1, dummy_span())),
                            Box::new(HirExpr::IntLit(1, dummy_span())),
                            dummy_span(),
                        )),
                        Box::new(HirExpr::IntLit(2, dummy_span())),
                        dummy_span(),
                    ),
                }],
                false,
                dummy_span(),
            ),
            span: dummy_span(),
        }],
        is_component: false,
        is_traced: false,
        is_llm: false,
        llm_model: None,
        is_pure: false,
        is_async: false,
        is_deprecated: false,
        is_layout: false,
        is_pub: false,
        is_metric: false,
        metric_name: None,
        is_health: false,
        preconditions: vec![],
        span: dummy_span(),
    });

    let output = vox_codegen_rust::emit::generate(&module, "test").unwrap();
    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs");

    assert!(
        lib_rs.contains("#[test]\npub fn check_math()"),
        "Should generate Rust test function, got: {}",
        lib_rs
    );
    assert!(
        lib_rs.contains("assert_eq!((1 + 1), 2)"),
        "Should map assert(a == b) to assert_eq!(a, b), got: {}",
        lib_rs
    );
}
