use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn emit_fn_includes_response_format_for_ai_structured_output() {
    let src = r#"
        type StubDto {
            ok: bool
        }

        @ai(structured_output = StubDto)
        @uses(net)
        fn with_schema(ctx: str) to StubDto {
            return StubDto { ok: true }
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "with_schema")
        .expect("with_schema");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("config.response_format = Some(response_format);"),
        "expected response_format wiring through LlmConfig, got:\n{emitted}"
    );
    assert!(
        emitted.contains("\"json_schema\":{\"name\":\"StubDto\"}"),
        "expected schema name in response_format payload, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_omits_response_format_without_structured_output() {
    let src = r#"
        @ai(model = "openrouter/auto")
        @uses(net)
        fn without_schema(ctx: str) to str {
            return ctx
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "without_schema")
        .expect("without_schema");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        !emitted.contains("\"response_format\": response_format"),
        "did not expect response_format injection, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_maps_intent_routed_payload_to_llm_telemetry_fields() {
    let src = r#"
        @ai(task_category = CodeGen, strengths = [codegen], tier_max = Pro)
        @uses(net)
        fn routed(spec: str) to str {
            return spec
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "routed")
        .expect("routed");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("config.telemetry_task_category = Some(\"CodeGen\".to_string());"),
        "expected task_category telemetry assignment, got:\n{emitted}"
    );
    assert!(
        emitted.contains("config.telemetry_strength_tag = Some(\"codegen\".to_string());"),
        "expected strength telemetry assignment, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_uses_prompt_cascade_when_prompt_fixture_present() {
    let src = r#"
        @prompt(stage = Planner, schema = PlanBlob)
        @uses(net)
        fn plan_next(spec: str) to str {
            return spec
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "plan_next")
        .expect("plan_next");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("llm::cascade::cascade_for_research_stage"),
        "expected cascade candidate construction, got:\n{emitted}"
    );
    assert!(
        emitted.contains("ResearchStage::Planner"),
        "expected Planner stage wiring, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_distributed_subagent_invokes_mesh_relay_helper() {
    let src = r#"
        @subagent(policy = distributed, max_depth = 2)
        @uses(net, spawn)
        fn mesh_delegate(spec: str) to str {
            return spec
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "mesh_delegate")
        .expect("mesh_delegate");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("relay_ai_fixture_distributed_subagent"),
        "expected distributed fixture to call mesh relay helper, got:\n{emitted}"
    );
    assert!(
        emitted.contains("cfg(feature = \"populi-transport\")"),
        "expected populi-transport cfg gate, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_routes_subagent_fixture_via_dispatch_router() {
    let src = r#"
        @subagent(policy = parallel, max_depth = 3)
        @uses(net, spawn)
        fn delegate(spec: str) to str {
            return spec
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "delegate")
        .expect("delegate");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("DispatchRouter::new"),
        "expected dispatch router creation, got:\n{emitted}"
    );
    assert!(
        emitted.contains("route_with_telemetry(&signal"),
        "expected telemetry-aware route call, got:\n{emitted}"
    );
}

#[test]
fn emit_fn_branches_search_fixture_with_aci_envelope() {
    let src = r#"
        @search(corpus = memory, query = "tenant:customer:42", into = SearchHit, top_k = 3)
        @uses(net)
        fn lookup_memory() to str {
            return "noop"
        }
    "#;
    let ast = parse(lex(src)).expect("parse");
    let hir = lower_module(&ast);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "lookup_memory")
        .expect("lookup_memory");
    let emitted = emit_fn(f, None, &[]);
    assert!(
        emitted.contains("lookup_fact_by_key"),
        "expected memory runtime hook reference, got:\n{emitted}"
    );
    assert!(
        emitted.contains("SearchDispatch"),
        "expected search telemetry emit, got:\n{emitted}"
    );
}
