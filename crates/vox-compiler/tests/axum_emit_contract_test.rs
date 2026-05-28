//! WS15 — generated Axum `serve_dispatch` keeps `/api` off the SSR dev proxy path.
//!
//! **Error JSON:** transactional `@mutation` when `@table` exists uses `db.transaction` and maps
//! failures to `Json(serde_json::json!({"error": e.to_string()}))`. `@query` and `@server` handlers
//! are emitted as straight-line bodies without that wrapper — see `vox-fullstack-artifacts.md`.

use vox_codegen::codegen_rust::emit::emit_main;
use vox_codegen::projection_bundle::project_bundle_from_hir;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

fn emit_main_hir(hir: &vox_compiler::hir::HirModule, pkg: &str) -> String {
    let bundle = project_bundle_from_hir(hir);
    emit_main(hir, pkg, &bundle.app)
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn emit_main_serve_dispatch_reserves_api_prefix_for_local_handlers() {
    let src = r#"
http get "/api/ping" to int {
    return 1
}
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let main_rs = emit_main_hir(&hir, "demo");
    assert!(
        main_rs.contains("starts_with(\"/api\")"),
        "fallback proxy must not steal /api GETs when VOX_SSR_DEV_URL is set"
    );
    assert!(
        main_rs.contains(".route(\"/api/ping\"") || main_rs.contains(".route(\"/api/ping\","),
        "expected explicit api route in router: {}",
        main_rs
    );
}

#[test]
fn emit_main_registers_query_get_before_fallback() {
    let src = r#"
@query fn q_ping() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    assert!(
        hir.endpoint_fns.len() == 1,
        "fixture should contain one @query: {:?}",
        hir.endpoint_fns
    );
    let main_rs = emit_main_hir(&hir, "demo");
    let fallback = main_rs.find(".fallback(serve_dispatch)").expect("fallback");
    let get_route = main_rs
        .find(".route(\"/api/query/q_ping\", get(handle_q_q_ping))")
        .or_else(|| main_rs.find(", get(handle_q_q_ping))"))
        .expect("GET query route");
    assert!(
        get_route < fallback,
        "registered /api routes must come before SPA/static fallback"
    );
    assert!(
        main_rs.contains("Query(q): Query<std::collections::BTreeMap<String, String>>"),
        "query handler should decode JSON query map"
    );
}

#[test]
fn emit_main_mutation_with_schema_wraps_transaction_and_emits_json_error_envelope() {
    let src = r#"
@table type T { a: str }

@mutation fn m_save() to int {
    return 1
}
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let main_rs = emit_main_hir(&hir, "demo");
    insta::assert_snapshot!("mutation_with_schema_main_rs_emit", main_rs);
}

#[test]
fn emit_main_query_handler_does_not_emit_transaction_error_envelope() {
    let src = r#"
@query fn q_only() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let main_rs = emit_main_hir(&hir, "demo");
    assert!(
        !main_rs.contains(r#"Json(serde_json::json!({"error": e.to_string()}))"#),
        "@query-only module should not inject mutation transaction error mapping; got:\n{main_rs}"
    );
    assert!(
        !main_rs.contains("db.transaction(async move {"),
        "query handler should not use db.transaction wrapper;\n{main_rs}"
    );
}

#[test]
fn emit_main_server_fn_without_schema_has_no_transaction_error_envelope() {
    let src = r#"
@server fn sf_ping() to int { return 1 }
"#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let main_rs = emit_main_hir(&hir, "demo");
    assert!(
        !main_rs.contains(r#"Json(serde_json::json!({"error": e.to_string()}))"#),
        "@server without schema should not use transactional error envelope;\n{main_rs}"
    );
}
