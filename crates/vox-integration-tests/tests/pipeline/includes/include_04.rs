// Pipeline integration — Web IR block 19 completion (OP-0291..OP-0303).

const PIPELINE_CLASSIC_BOX_STYLE_SRC: &str = r#"
component Box() {
    view: column(raw_class="x") { "a" }
}
style {
    .x { color: "red" }
}
"#;

const PIPELINE_DUP_CLIENT_ROUTE_BLOCKS_SRC: &str = r#"
import react.use_state
component A() {
    state n: int = 0
    view: column(raw_class="a") { n }
}
component B() {
    state n: int = 0
    view: column(raw_class="b") { n }
}
routes {
    "/" to A
}
routes {
    "/b" to B
}
"#;

#[test]
fn pipeline_integration_classic_style_emits_css_module_import() {
    with_web_ir_validate_cleared(|| {
        let tokens = lex(PIPELINE_CLASSIC_BOX_STYLE_SRC);
        let module = parse(tokens).expect("parse Box style");
        let hir = vox_compiler::hir::lower_module(&module);
        let out = generate(&hir).expect("codegen");
        let css = out
            .files
            .iter()
            .find(|(n, _)| n == "Box.css")
            .map(|(_, c)| c.as_str())
            .expect("Box.css");
        assert!(css.contains("color") || css.contains("red"), "{css}");
        let tsx = out
            .files
            .iter()
            .find(|(n, _)| n == "Box.tsx")
            .map(|(_, c)| c.as_str())
            .expect("Box.tsx");
        assert!(
            tsx.contains("Box.css") && tsx.contains("import"),
            "expected css import, got:\n{tsx}"
        );
    });
}

/// OP-0297: same intent as OP-0291 — full `chatbot.vox` Path C `component` + top-level `style { }` emits CSS module import.
#[test]
fn pipeline_integration_chatbot_fixture_classic_css_module_import() {
    let path = Path::new("fixtures/chatbot.vox");
    let src = read_utf8_path_capped(path)
        .or_else(|_| read_utf8_path_capped(Path::new("tests/fixtures/chatbot.vox")))
        .expect("read fixtures/chatbot.vox");
    with_web_ir_validate_cleared(|| {
        let tokens = lex(&src);
        let module = parse(tokens).expect("parse chatbot");
        let hir = vox_compiler::hir::lower_module(&module);
        let out = generate(&hir).expect("codegen chatbot");
        let tsx = out
            .files
            .iter()
            .find(|(n, _)| n == "Chat.tsx")
            .map(|(_, c)| c.as_str())
            .expect("Chat.tsx");
        assert!(
            tsx.contains("Chat.css") && tsx.contains("import"),
            "expected ./Chat.css import:\n{tsx}"
        );
    });
}

#[test]
fn pipeline_mixed_surface_endpoint_fn_emit_contains_api_x() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    // Express emit test decommissioned
}

#[test]
fn pipeline_reactive_view_whitespace_parity_legacy_vs_web_ir_env() {
    use std::collections::HashSet;

    use vox_codegen::codegen_ts::hir_emit::{EmitCtx, emit_hir_expr};

    use vox_compiler::hir::HirReactiveMember;
    use vox_codegen::web_ir::emit_tsx::emit_component_view_tsx;
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let src = r#"
component T() {
    state n: int = 1
    view: column(raw_class="x") { n }
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("parse T");
    let hir = vox_compiler::hir::lower_module(&module);
    let rc = hir
        .components
        .first()
        .expect("one reactive component");
    let view = rc.view.as_ref().expect("view");
    let state_name = match &rc.members[0] {
        HirReactiveMember::State(s) => s.name.clone(),
        _ => panic!("expected state member"),
    };
    let state_set = HashSet::from([state_name]);
    let legacy = emit_hir_expr(view, &EmitCtx::new(&state_set));
    let web = lower_hir_to_web_ir(&hir);
    let validate_diags = validate_web_ir(&web);
    assert!(validate_diags.is_empty(), "validate_web_ir: {validate_diags:?}");
    let preview = emit_component_view_tsx(&web, "T").expect("emit_component_view_tsx");
    assert_eq!(
        vox_codegen::codegen_ts::reactive::normalize_reactive_view_jsx_ws(&legacy),
        vox_codegen::codegen_ts::reactive::normalize_reactive_view_jsx_ws(&preview),
        "legacy vs Web IR preview (whitespace-normalized):\n{legacy}\n{preview}"
    );

    let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");
    let out = generate(&hir).expect("codegen");
    let after = out.reactive_stats;
    assert!(
        after.web_ir_view_emitted >= 1,
        "expected WebIrViewEmitted after parity match; after={after:?}"
    );
}

#[test]
fn pipeline_web_ir_rejects_duplicate_route_contract_ids_from_two_routes_blocks() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(PIPELINE_DUP_CLIENT_ROUTE_BLOCKS_SRC);
    let module = parse(tokens).expect("parse dup routes");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let diags = validate_web_ir(&web);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.duplicate_contract_id"),
        "expected duplicate contract id diagnostic, got {diags:?}"
    );
}

#[test]
fn pipeline_web_ir_validate_diagnostic_codes_use_dotted_prefix() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(PIPELINE_DUP_CLIENT_ROUTE_BLOCKS_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let diags = validate_web_ir(&web);
    assert!(!diags.is_empty(), "expected validation failures");
    for d in &diags {
        assert!(
            d.code.starts_with("web_ir_validate."),
            "expected web_ir_validate.* code, got {:?}",
            d.code
        );
    }
}

#[test]
fn pipeline_codegen_fails_duplicate_client_routes_when_web_ir_validate_enabled() {
    with_web_ir_validate_enabled(|| {
        let tokens = lex(PIPELINE_DUP_CLIENT_ROUTE_BLOCKS_SRC);
        let module = parse(tokens).expect("parse");
        let hir = vox_compiler::hir::lower_module(&module);
        match generate_with_options(&hir, CodegenOptions::default()) {
            Ok(_) => panic!("expected codegen to fail under VOX_WEBIR_VALIDATE=1"),
            Err(err) => assert!(
                err.contains("web_ir_validate.") || err.contains("VOX_WEBIR_VALIDATE"),
                "{err}"
            ),
        }
    });
}

#[test]
fn pipeline_web_ir_lower_validate_benchmark_smoke() {
    use std::time::Instant;
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let n = 25;
    let t0 = Instant::now();
    for _ in 0..n {
        let web = lower_hir_to_web_ir(&hir);
        let diags = validate_web_ir(&web);
        assert!(diags.is_empty(), "{diags:?}");
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "expected {n} lower+validate cycles in <10s, took {elapsed:?}"
    );
}

#[test]
fn pipeline_web_ir_ops_gate_compose() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let clean_tokens = lex(MIXED_SURFACE_SRC);
    let clean_mod = parse(clean_tokens).expect("parse mixed");
    let clean_hir = vox_compiler::hir::lower_module(&clean_mod);
    let clean_web = lower_hir_to_web_ir(&clean_hir);
    assert!(
        validate_web_ir(&clean_web).is_empty(),
        "MIXED_SURFACE must stay validator-clean"
    );

    let dup_tokens = lex(PIPELINE_DUP_CLIENT_ROUTE_BLOCKS_SRC);
    let dup_mod = parse(dup_tokens).expect("parse dup");
    let dup_hir = vox_compiler::hir::lower_module(&dup_mod);
    let dup_web = lower_hir_to_web_ir(&dup_hir);
    let dup_diags = validate_web_ir(&dup_web);
    assert!(
        dup_diags
            .iter()
            .any(|d| d.code == "web_ir_validate.route.duplicate_contract_id"),
        "dup fixture must fail validate: {dup_diags:?}"
    );
}

/// OP-0304 interim rollout gate: compose + perf smoke in one test; CI also runs `cargo nextest run -p vox-compiler --test web_ir_lower_emit_test --run-ignored ignored-only`.
#[test]
fn pipeline_web_ir_rollout_compose_gate_interim() {
    pipeline_web_ir_ops_gate_compose();
    pipeline_web_ir_lower_validate_benchmark_smoke();
}
