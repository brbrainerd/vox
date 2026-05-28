//! Mn-T4 / Mn-T5 acceptance: MENS decorators parse and participate in capability lowering.

// Rust 2024: env mutation primitives are `unsafe`. The CUDA-tier gate is
// toggled here via `VOX_CUDA_TIER`; SAFETY rationale lives at each block.
#![allow(unsafe_code)]

use vox_compiler::ast::decl::Decl;
use vox_compiler::hir::HirCapability;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_hir_module;

#[test]
fn inference_decorator_parses_model() {
    let src = r#"
        @inference(model = "llama")
        fn predict(x: str) to str {
            return x
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.inference_model.as_deref(), Some("llama"));
}

#[test]
fn extended_ai_payload_parses_intent_keys() {
    let src = r#"
        @ai(task_category = CodeGen, strengths = [codegen, reasoning], tier_max = Pro, cost_ceiling_usd_per_call = 0.5)
        fn route_me(spec: str) to str {
            return spec
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.ai_task_category.as_deref(), Some("CodeGen"));
    assert_eq!(
        f.ai_strengths,
        vec!["codegen".to_string(), "reasoning".to_string()]
    );
    assert_eq!(f.ai_tier_max.as_deref(), Some("Pro"));
    assert_eq!(f.ai_cost_ceiling_usd_per_call, Some(0.5));

    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "route_me")
        .expect("route_me");
    match &f.ai_fixture {
        Some(vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture::IntentRouted(v)) => {
            assert_eq!(v.task_category.as_deref(), Some("CodeGen"));
            assert_eq!(v.strengths, vec!["codegen", "reasoning"]);
            assert_eq!(v.tier_max.as_deref(), Some("Pro"));
            assert_eq!(v.cost_ceiling_usd_per_call, Some(0.5));
        }
        other => panic!("expected IntentRouted fixture, got {other:?}"),
    }
}

#[test]
fn extended_ai_payload_rejects_unknown_tier() {
    let src = r#"
        @ai(task_category = CodeGen, strengths = [codegen], tier_max = Ultra)
        fn route_me(spec: str) to str {
            return spec
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.ai_tier_max, None);
}

#[test]
fn prompt_decorator_parses_and_lowers_prompt_fixture() {
    let src = r#"
        @prompt(stage = Planner, schema = PlanBlob, redact = [api_key, "token"])
        @uses(net)
        fn plan_next(spec: str) to str {
            return spec
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.prompt_stage.as_deref(), Some("Planner"));
    assert_eq!(f.prompt_schema.as_deref(), Some("PlanBlob"));
    assert_eq!(
        f.prompt_redact,
        vec!["api_key".to_string(), "token".to_string()]
    );
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "plan_next")
        .expect("plan_next");
    match &f.ai_fixture {
        Some(vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture::Prompt(v)) => {
            assert_eq!(v.stage, "Planner");
            assert_eq!(v.schema, "PlanBlob");
            assert_eq!(v.redact, vec!["api_key", "token"]);
        }
        other => panic!("expected Prompt fixture, got {other:?}"),
    }
}

#[test]
fn subagent_decorator_parses_and_lowers_subagent_fixture() {
    let src = r#"
        @subagent(policy = parallel, max_depth = 2, budget_usd = 0.5, description = "fanout")
        @uses(net, spawn)
        fn fanout_summaries(spec: str) to str {
            return spec
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.subagent_policy.as_deref(), Some("parallel"));
    assert_eq!(f.subagent_max_depth, Some(2));
    assert_eq!(f.subagent_budget_usd, Some(0.5));
    assert_eq!(f.subagent_description.as_deref(), Some("fanout"));

    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "fanout_summaries")
        .expect("fanout_summaries");
    match &f.ai_fixture {
        Some(vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture::Subagent(v)) => {
            assert_eq!(v.policy, "parallel");
            assert_eq!(v.max_depth, 2);
            assert_eq!(v.budget_usd, Some(0.5));
            assert_eq!(v.description.as_deref(), Some("fanout"));
        }
        other => panic!("expected Subagent fixture, got {other:?}"),
    }
}

#[test]
fn search_decorator_parses_and_lowers_search_fixture() {
    let src = r#"
        @search(corpus = docs, query = "agentos fixtures", into = SearchHit, top_k = 5, policy = strict)
        @uses(net)
        fn find_docs() to str {
            return "ok"
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(f.search_corpus.as_deref(), Some("docs"));
    assert_eq!(f.search_query.as_deref(), Some("agentos fixtures"));
    assert_eq!(f.search_into.as_deref(), Some("SearchHit"));
    assert_eq!(f.search_top_k, Some(5));
    assert_eq!(f.search_policy.as_deref(), Some("strict"));

    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "find_docs")
        .expect("find_docs");
    match &f.ai_fixture {
        Some(vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture::Search(v)) => {
            assert_eq!(v.corpus, "docs");
            assert_eq!(v.query, "agentos fixtures");
            assert_eq!(v.into_type, "SearchHit");
            assert_eq!(v.top_k, Some(5));
            assert_eq!(v.policy.as_deref(), Some("strict"));
        }
        other => panic!("expected Search fixture, got {other:?}"),
    }
}

#[test]
fn hole_decorator_emits_unfilled_fixture_diagnostic() {
    let src = r#"
        @hole(spec = "fill me", reviewer = human, cache_key = "hole:demo", constraints = [strict])
        fn deferred() to str {
            return "todo"
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "deferred")
        .expect("deferred");
    match &f.ai_fixture {
        Some(vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture::Hole(v)) => {
            assert_eq!(v.spec, "fill me");
            assert_eq!(v.reviewer, "human");
            assert_eq!(v.cache_key, "hole:demo");
            assert_eq!(v.constraints, vec!["strict"]);
        }
        other => panic!("expected Hole fixture, got {other:?}"),
    }

    let mut hir = hir;
    let diags = typecheck_hir_module(src, &mut hir);
    assert!(
        diags
            .iter()
            .any(|d| d.code.as_deref() == Some("vox/fixture/unfilled-hole")),
        "expected unfilled hole diagnostic, got {diags:?}"
    );
}

#[test]
fn inference_lowers_gpu_random_net_caps() {
    let src = r#"
        @inference(model = "m")
        fn predict(x: str) to str {
            return x
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "predict")
        .expect("predict fn");
    assert!(f.capabilities.contains(&HirCapability::GpuCompute));
    assert!(f.capabilities.contains(&HirCapability::Random));
    assert!(f.capabilities.contains(&HirCapability::Net));
}

#[test]
fn training_step_lowers_caps() {
    let src = r#"
        @training_step
        fn step() to Unit {
            return Unit
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "step")
        .expect("step");
    assert!(f.training_step);
    assert!(f.capabilities.contains(&HirCapability::GpuCompute));
    assert!(f.capabilities.contains(&HirCapability::Mutate));
}

#[test]
#[serial_test::serial]
fn training_step_cuda_gate_emits_when_tier_low() {
    // SAFETY: serial_test ensures no concurrent env mutation in other tests.
    unsafe {
        std::env::set_var("VOX_CUDA_TIER", "0");
    }
    let src = r#"
        @training_step
        fn step() to Unit {
            return Unit
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let mut hir = lower_module(&m);
    let diags = typecheck_hir_module(src, &mut hir);
    unsafe {
        std::env::remove_var("VOX_CUDA_TIER");
    }
    assert!(
        diags
            .iter()
            .any(|d| d.code.as_deref() == Some("vox/train/cuda-required")),
        "expected cuda-required diagnostic, got {diags:?}"
    );
}

#[test]
fn distributed_train_workflow_parses_metadata() {
    let src = r#"
        @distributed_train(strategy = "data_parallel", peers = 4)
        workflow Train() to Unit {
            return Unit
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let Decl::Workflow(w) = &m.declarations[0] else {
        panic!("expected workflow");
    };
    assert_eq!(
        w.distributed_train_strategy.as_deref(),
        Some("data_parallel")
    );
    assert_eq!(w.distributed_train_peers, Some(4));
}

#[test]
fn distributed_train_workflow_lowers_caps_and_meta() {
    let src = r#"
        @distributed_train(strategy = "data_parallel", peers = 4)
        workflow Train() to Unit {
            return Unit
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "Train")
        .expect("Train");
    assert_eq!(
        f.distributed_train.as_ref().map(|(s, p)| (s.as_str(), *p)),
        Some(("data_parallel", 4))
    );
    assert!(f.capabilities.contains(&HirCapability::Spawn));
    assert!(f.capabilities.contains(&HirCapability::Net));
}
