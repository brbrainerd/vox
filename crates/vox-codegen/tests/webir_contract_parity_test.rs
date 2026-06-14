/// Task 5B: WebIR↔AppContract parity CI gate.
///
/// Both `web_ir::lower_hir_to_web_ir` and `app_contract::project_app_contract`
/// derive endpoint contracts from `HirModule.endpoint_fns`. This test asserts
/// they produce the same endpoint name set so the two derivations cannot diverge
/// (e.g., adding a new HirEndpointKind that only one side handles).
use std::collections::BTreeSet;
use vox_codegen::web_ir::{RouteNode, lower::lower_hir_to_web_ir};
use vox_compiler::app_contract::project_app_contract;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::descent::parse;

fn endpoint_names_from_web_ir(src: &str) -> BTreeSet<String> {
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let web_ir = lower_hir_to_web_ir(&hir);
    web_ir
        .route_nodes
        .iter()
        .filter_map(|n| match n {
            RouteNode::ServerFnContract(c) => Some(c.name.clone()),
            RouteNode::MutationContract(c) => Some(c.name.clone()),
            _ => None,
        })
        .collect()
}

fn endpoint_names_from_contract(src: &str) -> BTreeSet<String> {
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let contract = project_app_contract(&hir);
    let mut names = BTreeSet::new();
    for sf in &contract.server_fns {
        names.insert(sf.name.clone());
    }
    for qf in &contract.query_fns {
        names.insert(qf.name.clone());
    }
    for mf in &contract.mutation_fns {
        names.insert(mf.name.clone());
    }
    names
}

const MIXED_ENDPOINT_SRC: &str = r#"
@server
fn get_user(id: Int) -> Str { "user" }

@query
fn search_users(q: Str) -> Str { "results" }

@mutation
fn create_user(name: Str) -> Str { "created" }
"#;

#[test]
fn web_ir_lowers_endpoint_only_module() {
    // Existing test (carried forward — verifies the harness runs end-to-end).
    let module = parse(lex(MIXED_ENDPOINT_SRC)).expect("parse");
    let hir = lower_module(&module);
    let web_ir = lower_hir_to_web_ir(&hir);
    let count = web_ir
        .route_nodes
        .iter()
        .filter(|n| {
            matches!(
                n,
                RouteNode::ServerFnContract(_) | RouteNode::MutationContract(_)
            )
        })
        .count();
    assert_eq!(count, 3, "all 3 endpoints lowered to WebIR contract nodes");
}

#[test]
fn webir_and_contract_endpoint_names_match_for_server_fn() {
    let src = "@server\nfn ping(x: Int) -> Int { x }";
    assert_eq!(
        endpoint_names_from_web_ir(src),
        endpoint_names_from_contract(src),
        "WebIR and AppContract must agree on endpoint names for @server"
    );
}

#[test]
fn webir_and_contract_endpoint_names_match_for_query() {
    let src = "@query\nfn list_items(limit: Int) -> Str { \"ok\" }";
    assert_eq!(
        endpoint_names_from_web_ir(src),
        endpoint_names_from_contract(src),
        "WebIR and AppContract must agree on endpoint names for @query"
    );
}

#[test]
fn webir_and_contract_endpoint_names_match_for_mutation() {
    let src = "@mutation\nfn delete_item(id: Int) -> Str { \"ok\" }";
    assert_eq!(
        endpoint_names_from_web_ir(src),
        endpoint_names_from_contract(src),
        "WebIR and AppContract must agree on endpoint names for @mutation"
    );
}

#[test]
fn webir_and_contract_endpoint_names_match_for_mixed_module() {
    assert_eq!(
        endpoint_names_from_web_ir(MIXED_ENDPOINT_SRC),
        endpoint_names_from_contract(MIXED_ENDPOINT_SRC),
        "WebIR and AppContract must agree on ALL endpoint names in a mixed module"
    );
}

#[test]
fn tauri_rust_commands_match_vox_client_invoke() {
    // Existing test (carried forward).
    let module = parse(lex(MIXED_ENDPOINT_SRC)).expect("parse");
    let hir = lower_module(&module);
    let contract = project_app_contract(&hir);
    // Server fns go in server_fns, query fns in query_fns, mutations in mutation_fns.
    assert_eq!(contract.server_fns.len(), 1);
    assert_eq!(contract.query_fns.len(), 1);
    assert_eq!(contract.mutation_fns.len(), 1);
}
