//! Integration tests for Rust code generation, focusing on server functions.

use vox_ast::span::Span;
use vox_hir::*;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

fn make_module_with_server_fns() -> HirModule {
    HirModule {
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![
            HirServerFn {
                id: DefId(0),
                name: "greet".to_string(),
                params: vec![HirParam {
                    id: DefId(1),
                    name: "name".to_string(),
                    type_ann: Some(HirType::Named("str".to_string())),
                    default: None,
                    span: dummy_span(),
                }],
                return_type: Some(HirType::Named("str".to_string())),
                body: vec![HirStmt::Return {
                    value: Some(HirExpr::StringLit("hello".to_string(), dummy_span())),
                    span: dummy_span(),
                }],
                route_path: "/api/greet".to_string(),
                span: dummy_span(),
            },
            HirServerFn {
                id: DefId(2),
                name: "add".to_string(),
                params: vec![
                    HirParam {
                        id: DefId(3),
                        name: "a".to_string(),
                        type_ann: Some(HirType::Named("int".to_string())),
                        default: None,
                        span: dummy_span(),
                    },
                    HirParam {
                        id: DefId(4),
                        name: "b".to_string(),
                        type_ann: Some(HirType::Named("int".to_string())),
                        default: None,
                        span: dummy_span(),
                    },
                ],
                return_type: Some(HirType::Named("int".to_string())),
                body: vec![],
                route_path: "/api/add".to_string(),
                span: dummy_span(),
            },
        ],
        tables: vec![],
        indexes: vec![],
        mcp_tools: vec![],
    }
}

#[test]
fn server_fn_generates_routes() {
    let module = make_module_with_server_fns();
    let output = vox_codegen_rust::generate(&module, "test_app").unwrap();

    let main_rs = output
        .files
        .get("src/main.rs")
        .expect("main.rs should exist");

    // Routes registered
    assert!(
        main_rs.contains(".route(\"/api/greet\", post(handle_sf_greet))"),
        "greet route missing"
    );
    assert!(
        main_rs.contains(".route(\"/api/add\", post(handle_sf_add))"),
        "add route missing"
    );

    // Handlers generated
    assert!(
        main_rs.contains("async fn handle_sf_greet("),
        "greet handler missing"
    );
    assert!(
        main_rs.contains("async fn handle_sf_add("),
        "add handler missing"
    );

    // Param extraction
    assert!(
        main_rs.contains("request[\"name\"]"),
        "name param extraction missing"
    );
    assert!(
        main_rs.contains("request[\"a\"]"),
        "a param extraction missing"
    );
    assert!(
        main_rs.contains("request[\"b\"]"),
        "b param extraction missing"
    );
}

#[test]
fn server_fn_generates_api_client() {
    let module = make_module_with_server_fns();
    let output = vox_codegen_rust::generate(&module, "test_app").unwrap();

    let api_ts = &output.api_client_ts;
    assert!(!api_ts.is_empty(), "api.ts should not be empty");

    // greet function
    assert!(
        api_ts.contains("export async function greet(name: string): Promise<string>"),
        "greet TS function missing"
    );
    assert!(
        api_ts.contains("fetch(`${API_BASE}/api/greet`"),
        "greet fetch URL missing"
    );
    assert!(
        api_ts.contains("JSON.stringify({ name })"),
        "greet body missing"
    );

    // add function
    assert!(
        api_ts.contains("export async function add(a: number, b: number): Promise<number>"),
        "add TS function missing"
    );
    assert!(
        api_ts.contains("fetch(`${API_BASE}/api/add`"),
        "add fetch URL missing"
    );
    assert!(
        api_ts.contains("JSON.stringify({ a, b })"),
        "add body missing"
    );
}

#[test]
fn empty_server_fns_generate_no_api_client() {
    let module = HirModule {
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
        mcp_tools: vec![],
    };
    let output = vox_codegen_rust::generate(&module, "test_app").unwrap();
    assert!(
        output.api_client_ts.is_empty(),
        "api.ts should be empty when no server fns"
    );
}

#[test]
fn server_fn_route_path_derived_from_name() {
    let module = make_module_with_server_fns();
    // Verify the route path matches /api/{name}
    assert_eq!(module.server_fns[0].route_path, "/api/greet");
    assert_eq!(module.server_fns[1].route_path, "/api/add");
}
