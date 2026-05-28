// OP-S049–OP-S220 supplemental gates (included from `tests/pipeline.rs`).
// Doc guards read the repo root (`crates/vox-integration-tests/../../`).

fn op_s_doc_repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn op_s_read_doc(rel: &str) -> String {
    let p = op_s_doc_repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

// --- Doc cross-link gates (OP-S052, S068, S104, S132, S152, S184, S212) ---

#[test]
fn op_s052_adr_readme_links_internal_web_ir_blueprint_and_012() {
    let s = op_s_read_doc("docs/src/adr/README.md");
    assert!(s.contains("012-internal-web-ir-strategy.md"));
    assert!(s.contains("internal-web-ir-implementation-blueprint.md"));
    assert!(s.contains("internal-web-ir-side-by-side-schema.md"));
}

#[test]
fn op_s068_vox_web_stack_links_operations_catalog() {
    let s = op_s_read_doc("docs/src/reference/vox-web-stack.md");
    assert!(s.contains("internal-web-ir-implementation-blueprint.md"));
    assert!(s.contains("web_ir") || s.contains("Web IR") || s.contains("WebIR"));
}

#[test]
fn op_s104_adr_012_contains_interop_policy_anchor() {
    let s = op_s_read_doc("docs/src/adr/012-internal-web-ir-strategy.md");
    assert!(
        s.contains("Interop policy") || s.contains("interop policy"),
        "expected interop policy section"
    );
}

#[test]
fn op_s132_vox_codegen_ts_links_blueprint_roadmap() {
    let s = op_s_read_doc("docs/src/api/vox-codegen-ts.md");
    assert!(s.contains("internal-web-ir-implementation-blueprint.md")
        || s.contains("internal-web-ir-side-by-side-schema.md"));
}

#[test]
fn op_s152_vox_web_stack_gate_matrix_anchor() {
    let s = op_s_read_doc("docs/src/reference/vox-web-stack.md");
    assert!(s.contains("WebIR") || s.contains("Web IR") || s.contains("web_ir"));
}

#[test]
fn op_s184_adr_readme_appendix_web_ir_cross_links() {
    let s = op_s_read_doc("docs/src/adr/README.md");
    assert!(s.contains("K-metric appendix") || s.contains("k-metric-appendix"));
}

#[test]
fn op_s212_vox_codegen_ts_final_gate_cross_links() {
    let s = op_s_read_doc("docs/src/api/vox-codegen-ts.md");
    assert!(s.contains("ADR") || s.contains("adr/012"));
}

// --- Pipeline codegen / Web IR gates ---

fn op_s_read_chatbot_fixture() -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chatbot.vox");
    read_utf8_path_capped(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// OP-S060: style block lowers to Web IR `style_nodes` and emits `.css` from codegen.
#[test]
fn op_s060_style_bridge_gate_chatbot_fixture_css_and_webir_styles() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;
    let src = op_s_read_chatbot_fixture();
    let module = parse(lex(&src)).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let (_web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(
        summary.style_rules_lowered >= 1,
        "expected classic style rules in Web IR summary: {summary:?}"
    );
    let out = generate_without_express!(&module);
    assert!(out.files.iter().any(|(n, _)| n.ends_with(".css")));
}

/// OP-S062: server fn contract present on multi-route HIR for Express mapping.
#[test]
fn op_s062_server_contract_fixture_multi_route_has_get_stats() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    assert!(
        hir.endpoint_fns.iter().any(|s| s.name == "get_stats"),
        "expected @server fn get_stats in fixture"
    );
    // Express route generation is decommissioned; test only endpoint presence.
}

/// OP-S076: mixed surface lowers behaviors + reactive view roots (Path C); classic views optional.
#[test]
fn op_s076_behavior_view_map_gate_mixed_surface_summary() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let (_web, s) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(s.components >= 2, "{s:?}");
    assert_eq!(s.classic_component_views_lowered, 0, "{s:?}");
}

/// OP-S080: mixed surface codegen emits route manifest (adapter-owned router).
#[test]
fn op_s080_wrapper_inventory_gate_mixed_surface_has_app_router() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let out = generate_without_express!(&module);
    let m = out
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .expect("routes.manifest.ts");
    assert!(
        m.1.contains("voxRoutes") || m.1.contains("export const voxRoutes"),
        "expected voxRoutes in manifest:\n{}",
        &m.1[..m.1.len().min(400)]
    );
}

/// OP-S081–S084: dual-run — legacy generate vs Web IR validate both succeed on mixed surface.
#[test]
fn op_s081_084_dual_run_diff_extension_gate_mixed_surface() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
    let out = generate_without_express!(&module);
    assert!(out.files.iter().any(|(n, _)| n == "Dash.tsx"));
}

/// OP-S090 / S092: route printer emits stable App route entries.
#[test]
fn op_s090_s092_route_printer_integration_gate_multi_route_paths() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let out = generate_without_express!(&module);
    let m = out.files.iter().find(|(n, _)| n == "routes.manifest.ts").unwrap();
    assert!(
        m.1.contains("/todos")
            || m.1.contains("'/todos'")
            || m.1.contains("\"/todos\"")
            || m.1.contains("TodoList"),
        "expected /todos or TodoList in manifest, got:\n{}",
        &m.1[..m.1.len().min(1200)]
    );
}

/// OP-S112: style + route pipeline — chatbot produces CSS import in TSX (ties emitter bridge).
#[test]
fn op_s112_style_node_bridge_gate_chatbot_imports_css() {
    let path = Path::new("fixtures/chatbot.vox");
    let src = read_utf8_path_capped(path)
        .or_else(|_| read_utf8_path_capped(Path::new("tests/fixtures/chatbot.vox")))
        .expect("chatbot");
    let module = parse(lex(&src)).expect("parse");
    let out = generate_without_express!(&module);
    let chat = out.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    assert!(chat.1.contains(".css"));
}

/// OP-S116: behavior + classic component in one module typecheck clean.
#[test]
fn op_s116_behavior_component_gate_mixed_typechecks() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let diags = typecheck_module(&module, "");
    assert!(
        !diags
            .iter()
            .any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error),
        "{diags:?}"
    );
}

/// OP-S118: route contracts lowered for client trees.
#[test]
fn op_s118_route_contract_fixture_client_trees() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let (_w, s) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(s.client_route_trees >= 1, "{s:?}");
}

fn op_s_pack_gate_inner(src: &'static str, label: &str) {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;
    let module = parse(lex(src)).expect(label);
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    assert!(
        validate_web_ir(&web).is_empty(),
        "{label} web_ir {:?}",
        validate_web_ir(&web)
    );
    let _ = generate_without_express!(&module);
}

/// OP-S128 fixture pack D gate.
#[test]
fn op_s128_fixture_pack_d_gate() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "pack D");
}

/// OP-S138 dual-run contract fixture A.
#[test]
fn op_s138_dual_run_contract_fixture_a() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "dual-run fixture A");
}

/// OP-S140 dual-run contract gate A.
#[test]
fn op_s140_dual_run_contract_gate() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "dual-run A");
}

/// OP-S148 fixture pack E gate.
#[test]
fn op_s148_fixture_pack_e_gate() {
    op_s_pack_gate_inner(CHATBOT_SRC, "pack E");
}

/// OP-S160 route/data bridge gate B — HTTP loaders in multi-route Web IR.
#[test]
fn op_s160_route_data_bridge_gate_b() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let (_w, s) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(s.http_loader_contracts >= 1 || s.query_fn_contracts >= 1, "{s:?}");
}

/// OP-S164 component/reactive gate B.
#[test]
fn op_s164_component_reactive_gate_b() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "B");
}

/// OP-S172 emitter bridge gate B.
#[test]
fn op_s172_emitter_bridge_gate_b() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let out = generate_without_express!(&module);
    assert!(!out.files.is_empty());
}

/// OP-S180 fixture pack F gate.
#[test]
fn op_s180_fixture_pack_f_gate() {
    op_s_pack_gate_inner(CHATBOT_SRC, "pack F");
}

/// OP-S196 component/reactive gate C.
#[test]
fn op_s196_component_reactive_gate_c() {
    op_s_pack_gate_inner(MULTI_ROUTE_SRC, "C");
}

/// OP-S200 emitter gate C.
#[test]
fn op_s200_emitter_gate_c() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let out = generate_without_express!(&module);
    assert!(out.files.iter().any(|(n, _)| n == "routes.manifest.ts"));
}

/// OP-S208 fixture pack G gate.
#[test]
fn op_s208_fixture_pack_g_gate() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "pack G");
}

/// OP-S215 / S216: final gate matrix — validate-on path clean for parity fixture.
#[test]
fn op_s215_s216_final_gate_matrix_web_ir_clean_for_parity_fixture() {
    op_s_pack_gate_inner(MIXED_SURFACE_SRC, "final matrix");
}

/// OP-S220: supplemental closure — docs blueprint lists acceptance gates table.
#[test]
fn op_s220_supplemental_closure_blueprint_has_acceptance_gates() {
    let s = op_s_read_doc("docs/src/architecture/internal-web-ir-implementation-blueprint.md");
    assert!(s.contains("Acceptance gates"));
    assert!(s.contains("G4 Parity Gate") || s.contains("Parity Gate"));
}
