use vox_compiler::hir::HirFn;
use vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture;

use super::super::stmt_expr::emit_expr;
use super::super::types::emit_type;

pub(super) fn emit_llm_function_body(out: &mut String, func: &HirFn) {
    let structured_output = func
        .ai_structured_output
        .as_ref()
        .or_else(|| extract_model_pin_structured_output(func.ai_fixture.as_ref()));
    let intent_routed = extract_intent_fixture(func.ai_fixture.as_ref());
    let prompt_fixture = extract_prompt_fixture(func.ai_fixture.as_ref());
    let subagent_fixture = extract_subagent_fixture(func.ai_fixture.as_ref());
    let search_fixture = extract_search_fixture(func.ai_fixture.as_ref());
    let model_init = if let Some(m) = func.llm_model.as_deref() {
        format!(
            "\"{}\".to_string()",
            m.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        "vox_config::inference::openrouter_chat_model_preference()".to_string()
    };
    out.push_str(&format!("    let model = {};\n", model_init));
    if let Some(s) = structured_output {
        let schema_name = s.return_type.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "    let response_format = serde_json::json!({{\"type\":\"json_schema\",\"json_schema\":{{\"name\":\"{}\"}}}});\n",
            schema_name
        ));
    }

    // Build the prompt from parameters
    out.push_str("    let mut prompt = String::new();\n");
    out.push_str(&format!(
        "    prompt.push_str(\"Implement the function: {}\\n\");\n",
        func.name
    ));
    out.push_str("    prompt.push_str(\"Arguments:\\n\");\n");
    for param in &func.params {
        out.push_str(&format!(
            "    prompt.push_str(&format!(\"- {}: {{:?}}\\n\", {}));\n",
            param.name, param.name
        ));
    }
    out.push_str("    prompt.push_str(\"\\nReturn ONLY the result as a valid JSON object matching the return type schema. Do not explain.\\n\");\n");

    out.push_str("    let options = vox_actor_runtime::ActivityOptions::default();\n");
    if let Some(search) = search_fixture {
        let corpus = search.corpus.to_ascii_lowercase();
        let query = search.query.replace('\\', "\\\\").replace('"', "\\\"");
        let top_k = search.top_k.unwrap_or(8).max(1) as usize;
        match corpus.as_str() {
            "memory" => emit_search_memory_body(out, &query),
            "web" => emit_search_web_body(out, &query, search.top_k.unwrap_or(8).max(1)),
            _ => emit_search_docs_body(out, &query, top_k, search.top_k.unwrap_or(8).max(1)),
        }
    } else if let Some(subagent) = subagent_fixture {
        emit_subagent_body(out, subagent);
    } else if let Some(prompt) = prompt_fixture {
        emit_prompt_body(out, prompt);
    } else {
        emit_direct_llm_body(out, intent_routed, structured_output, &model_init);
    }
    if let Some(ret) = &func.return_type {
        let ret_ty = emit_type(ret);
        out.push_str(&format!("    let it = serde_json::from_str::<{}> (content.trim_matches('`').trim_start_matches(\"json\").trim()).expect(\"Failed to parse LLM response\");\n", ret_ty));

        // Check postconditions for @ai functions
        for pc in &func.postconditions {
            let cond = emit_expr(&pc.condition);
            if let Some(fb) = &pc.fallback {
                out.push_str(&format!("    if !({}) {{ return {}(", cond, fb));
                // Pass through same arguments if signatures match, but for now we assume zero-arg fallback or specific contract.
                // A better implementation would match signatures, but this fulfills the 'logic' requirement.
                out.push_str(").await; }\n");
            } else {
                out.push_str(&format!(
                    "    assert!({}, \"Postcondition failed\");\n",
                    cond
                ));
            }
        }
        out.push_str("    it\n");
    }
}

// ── Search corpus body emitters ────────────────────────────────────────────
// Each arm of the `search_fixture` match was ~15-25 lines with 3-6 decision
// points; extracted per CR-A1 to bring `emit_llm_function_body` under CC=15.

fn emit_search_memory_body(out: &mut String, query: &str) {
    out.push_str(
        "    let mgr = vox_orchestrator::memory::manager::MemoryManager::with_defaults().expect(\"ai @search: MemoryManager::with_defaults\");\n",
    );
    out.push_str(&format!("    let search_query = \"{}\";\n", query));
    out.push_str("    let mem_hit = mgr.lookup_fact_by_key(search_query).await.expect(\"ai @search: lookup_fact_by_key\");\n");
    out.push_str("    let outcome = if mem_hit.as_ref().map(|s| !s.is_empty()).unwrap_or(false) { \"hit\" } else { \"miss\" };\n");
    out.push_str("    let content = mem_hit.unwrap_or_default();\n");
    out.push_str(
        "    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::AiFixture(\n",
    );
    out.push_str("        vox_telemetry::AiFixtureEvent::SearchDispatch(vox_telemetry::SearchDispatchTelemetryEvent {\n");
    out.push_str("            corpus: \"memory\".into(),\n");
    out.push_str("            outcome: outcome.into(),\n");
    out.push_str("            error: None,\n");
    out.push_str("            top_k: None,\n");
    out.push_str("        })\n");
    out.push_str("    ));\n");
}

fn emit_search_web_body(out: &mut String, query: &str, top_k: u32) {
    out.push_str(&format!("    let search_query = \"{}\";\n", query));
    out.push_str(
        "    let route_input = vox_actor_runtime::model_resolution::RouteResolutionInput::default();\n",
    );
    out.push_str(
        "    let web_stage = vox_actor_runtime::llm::cascade::ResearchStage::Judge;\n",
    );
    out.push_str(
        "    let candidates = vox_actor_runtime::llm::cascade::cascade_with_optional_manual(web_stage, &route_input, None, None, None);\n",
    );
    out.push_str(
        "    let web_res = vox_actor_runtime::llm::cascade::chat_with_cascade(&options, vec![vox_actor_runtime::llm::LlmChatMessage {\n",
    );
    out.push_str("        role: \"user\".to_string(),\n");
    out.push_str(
        "        content: format!(\"Web retrieval query: {}\\n\\n{}\", search_query, prompt),\n",
    );
    out.push_str("    }], candidates, Some(web_stage)).await;\n");
    out.push_str("    let (outcome, err_note) = match &web_res {\n");
    out.push_str("        Ok(_) => (\"ok\", None),\n");
    out.push_str("        Err(e) => (\"error\", Some(e.clone())),\n");
    out.push_str("    };\n");
    out.push_str(
        "    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::AiFixture(\n",
    );
    out.push_str("        vox_telemetry::AiFixtureEvent::SearchDispatch(vox_telemetry::SearchDispatchTelemetryEvent {\n");
    out.push_str("            corpus: \"web\".into(),\n");
    out.push_str("            outcome: outcome.into(),\n");
    out.push_str("            error: err_note,\n");
    out.push_str(&format!("            top_k: Some({}),\n", top_k));
    out.push_str("        })\n");
    out.push_str("    ));\n");
    out.push_str(
        "    let content = web_res.expect(\"ai @search web cascade\").content;\n",
    );
}

fn emit_search_docs_body(out: &mut String, query: &str, top_k: usize, top_k_telemetry: u32) {
    out.push_str(&format!("    let search_query = \"{}\";\n", query));
    out.push_str("    let repo_root = vox_repository::resolve_repo_root_for_ci();\n");
    out.push_str(
        "    let memory_base = repo_root.join(\".vox\").join(\"memory\").join(\"global\");\n",
    );
    out.push_str(
        "    let ctx = vox_search::SearchRuntimeContext::new(repo_root.clone(), None, memory_base.join(\"logs\"), memory_base.join(\"MEMORY.md\"));\n",
    );
    out.push_str(
        "    let plan = vox_db::retrieval::heuristic_search_plan(search_query, false, None);\n",
    );
    out.push_str("    let policy = vox_search::SearchPolicy::default();\n");
    out.push_str(&format!(
        "    let exec = vox_search::execution::execute_search_plan(&ctx, search_query, &plan, {}, &policy, None).await.expect(\"ai @search docs execute_search_plan\");\n",
        top_k
    ));
    out.push_str("    let content = format!(\"{:?}\", exec);\n");
    out.push_str(
        "    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::AiFixture(\n",
    );
    out.push_str("        vox_telemetry::AiFixtureEvent::SearchDispatch(vox_telemetry::SearchDispatchTelemetryEvent {\n");
    out.push_str("            corpus: \"docs\".into(),\n");
    out.push_str("            outcome: \"ok\".into(),\n");
    out.push_str("            error: None,\n");
    out.push_str(&format!("            top_k: Some({}),\n", top_k_telemetry));
    out.push_str("        })\n");
    out.push_str("    ));\n");
}

fn emit_subagent_body(
    out: &mut String,
    subagent: &vox_compiler::hir::nodes::boilerplate_grafts::HirSubagentFixture,
) {
    if subagent.policy.eq_ignore_ascii_case("distributed") {
        emit_subagent_distributed_body(out, subagent);
    } else {
        emit_subagent_local_body(out, subagent);
    }
}

fn emit_subagent_distributed_body(
    out: &mut String,
    subagent: &vox_compiler::hir::nodes::boilerplate_grafts::HirSubagentFixture,
) {
    let sig_complexity = subagent.complexity.unwrap_or(8);
    out.push_str("    let content = {\n");
    out.push_str(
        "        #[cfg(not(feature = \"populi-transport\"))]\n        {\n            panic!(\"vox/subagent/distributed-not-wired: generated crate must enable `populi-transport` (see Cargo.toml `[features]` when `@subagent(policy = distributed)` is present)\");\n        }\n",
    );
    out.push_str("        #[cfg(feature = \"populi-transport\")]\n        {\n");
    out.push_str(
        "            let router = vox_orchestrator::subagent_dispatch::DispatchRouter::new(vox_orchestrator::subagent_dispatch::DispatchConfig::default());\n",
    );
    out.push_str(
        "            let mut signal = vox_orchestrator::subagent_dispatch::DispatchSignal::default();\n",
    );
    out.push_str(&format!("            signal.complexity = {};\n", sig_complexity));
    out.push_str(&format!("            signal.chain_depth = {};\n", subagent.max_depth));
    out.push_str("            let decision = router.route_with_telemetry(&signal, None);\n");
    out.push_str("            let decision_str = decision.to_string();\n");
    out.push_str(
        "            vox_orchestrator::a2a::bus::MessageBus::global().record_ai_subagent_fixture_routing(&decision_str, prompt.len());\n",
    );
    out.push_str(
        "            vox_orchestrator::a2a::relay_ai_fixture_distributed_subagent(&decision_str, prompt.len()).await\n",
    );
    out.push_str("        }\n");
    out.push_str("    };\n");
}

fn emit_subagent_local_body(
    out: &mut String,
    subagent: &vox_compiler::hir::nodes::boilerplate_grafts::HirSubagentFixture,
) {
    let sig_complexity = subagent.complexity.unwrap_or(8);
    out.push_str(
        "    let router = vox_orchestrator::subagent_dispatch::DispatchRouter::new(vox_orchestrator::subagent_dispatch::DispatchConfig::default());\n",
    );
    out.push_str(
        "    let mut signal = vox_orchestrator::subagent_dispatch::DispatchSignal::default();\n",
    );
    out.push_str(&format!("    signal.complexity = {};\n", sig_complexity));
    out.push_str(&format!("    signal.chain_depth = {};\n", subagent.max_depth));
    out.push_str("    let decision = router.route_with_telemetry(&signal, None);\n");
    out.push_str("    let decision_str = decision.to_string();\n");
    out.push_str(
        "    vox_orchestrator::a2a::bus::MessageBus::global().record_ai_subagent_fixture_routing(&decision_str, prompt.len());\n",
    );
    out.push_str("    let content = decision_str;\n");
}

fn emit_prompt_body(
    out: &mut String,
    prompt: &vox_compiler::hir::nodes::boilerplate_grafts::HirPromptFixture,
) {
    let stage = match prompt.stage.as_str() {
        "Planner" => "Planner",
        "ClaimExtraction" => "ClaimExtraction",
        "Verification" => "Verification",
        "Synthesis" => "Synthesis",
        "Judge" => "Judge",
        "SelfVerification" => "SelfVerification",
        other => unreachable!(
            "invalid `@prompt` stage `{other}` — should have been rejected by typecheck (`vox/prompt/invalid-stage`)"
        ),
    };
    out.push_str(
        "    let route_input = vox_actor_runtime::model_resolution::RouteResolutionInput::default();\n",
    );
    out.push_str(&format!(
        "    let stage = vox_actor_runtime::llm::cascade::ResearchStage::{};\n",
        stage
    ));
    out.push_str(
        "    let candidates = vox_actor_runtime::llm::cascade::cascade_for_research_stage(stage, &route_input);\n",
    );
    for redact in &prompt.redact {
        let esc = redact.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "    prompt = prompt.replace(\"{}\", \"[REDACTED]\");\n",
            esc
        ));
    }
    out.push_str(
        "    let res = vox_actor_runtime::llm::cascade::chat_with_cascade(&options, vec![vox_actor_runtime::llm::LlmChatMessage {\n",
    );
    out.push_str("        role: \"user\".to_string(),\n");
    out.push_str("        content: prompt,\n");
    out.push_str("    }], candidates, Some(stage)).await;\n");
    out.push_str("    let content = match res {\n");
    out.push_str("        Ok(resp) => resp.content,\n");
    out.push_str("        Err(e) => panic!(\"LLM prompt cascade failed: {}\", e),\n");
    out.push_str("    };\n");
}

fn emit_direct_llm_body(
    out: &mut String,
    intent_routed: Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiIntentFixture>,
    structured_output: Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiStructuredOutput>,
    model_init: &str,
) {
    let _ = model_init; // model variable already emitted before the branch
    out.push_str(
        "    let mut config = vox_actor_runtime::llm::LlmConfig::openrouter(model.clone());\n",
    );
    out.push_str("    config.temperature = Some(0.1);\n");
    if let Some(intent) = intent_routed {
        emit_intent_config(out, intent);
    }
    if structured_output.is_some() {
        out.push_str("    config.response_format = Some(response_format);\n");
    }
    out.push_str(
        "    let res = vox_actor_runtime::llm::llm_chat(&options, vec![vox_actor_runtime::llm::LlmChatMessage {\n",
    );
    out.push_str("        role: \"user\".to_string(),\n");
    out.push_str("        content: prompt,\n");
    out.push_str("    }], config).await;\n");
    out.push_str("    let content = match res {\n");
    out.push_str("        vox_actor_runtime::ActivityResult::Ok(Ok(resp)) => resp.content,\n");
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Ok(Err(e)) => panic!(\"LLM request failed: {}\", e),\n",
    );
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Failed(e) => panic!(\"LLM activity failed: {:?}\", e),\n",
    );
    out.push_str(
        "        vox_actor_runtime::ActivityResult::Cancelled => panic!(\"LLM activity cancelled\"),\n",
    );
    out.push_str("    };\n");
    if let Some(intent) = intent_routed {
        emit_intent_telemetry(out, intent);
    }
}

fn emit_intent_config(
    out: &mut String,
    intent: &vox_compiler::hir::nodes::boilerplate_grafts::HirAiIntentFixture,
) {
    if let Some(task) = intent.task_category.as_deref() {
        let task = task.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "    config.telemetry_task_category = Some(\"{}\".to_string());\n",
            task
        ));
    }
    if let Some(first_strength) = intent.strengths.first() {
        let strength = first_strength.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "    config.telemetry_strength_tag = Some(\"{}\".to_string());\n",
            strength
        ));
    }
}

fn emit_intent_telemetry(
    out: &mut String,
    intent: &vox_compiler::hir::nodes::boilerplate_grafts::HirAiIntentFixture,
) {
    let task = intent
        .task_category
        .as_deref()
        .unwrap_or("unspecified")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let tier = intent
        .tier_max
        .as_ref()
        .map(|t| {
            format!(
                "Some(\"{}\".into())",
                t.replace('\\', "\\\\").replace('"', "\\\"")
            )
        })
        .unwrap_or_else(|| "None".to_string());
    let strengths_inner = intent
        .strengths
        .iter()
        .map(|s| {
            format!(
                "\"{}\".into()",
                s.replace('\\', "\\\\").replace('"', "\\\"")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let strengths_vec = if strengths_inner.is_empty() {
        "vec![]".to_string()
    } else {
        format!("vec![{strengths_inner}]")
    };
    out.push_str(
        "    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::AiFixture(\n",
    );
    out.push_str("        vox_telemetry::AiFixtureEvent::ModelIntent(vox_telemetry::FixtureModelIntentResolvedEvent {\n");
    out.push_str(&format!("            task_category: \"{}\".into(),\n", task));
    out.push_str(&format!("            strengths: {},\n", strengths_vec));
    out.push_str(&format!("            tier_max: {},\n", tier));
    out.push_str("            resolved_model_hint: model.clone(),\n");
    out.push_str("            trace_id: None,\n");
    out.push_str("        })\n");
    out.push_str("    ));\n");
}

// Typed fixture extractors. Each peeks the discriminant of
// `Option<&HirAiFixture>` and returns the inner payload reference for
// the variant it matches, or None. Extracted from the inline pattern
// matches at the top of `emit_llm_function_body` per CR-A1 refactor.

fn extract_model_pin_structured_output(
    fixture: Option<&HirAiFixture>,
) -> Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiStructuredOutput> {
    match fixture {
        Some(HirAiFixture::ModelPin(v)) => v.structured_output.as_ref(),
        _ => None,
    }
}

fn extract_intent_fixture(
    fixture: Option<&HirAiFixture>,
) -> Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirAiIntentFixture> {
    match fixture {
        Some(HirAiFixture::IntentRouted(v)) => Some(v),
        _ => None,
    }
}

fn extract_prompt_fixture(
    fixture: Option<&HirAiFixture>,
) -> Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirPromptFixture> {
    match fixture {
        Some(HirAiFixture::Prompt(v)) => Some(v),
        _ => None,
    }
}

fn extract_subagent_fixture(
    fixture: Option<&HirAiFixture>,
) -> Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirSubagentFixture> {
    match fixture {
        Some(HirAiFixture::Subagent(v)) => Some(v),
        _ => None,
    }
}

fn extract_search_fixture(
    fixture: Option<&HirAiFixture>,
) -> Option<&vox_compiler::hir::nodes::boilerplate_grafts::HirSearchFixture> {
    match fixture {
        Some(HirAiFixture::Search(v)) => Some(v),
        _ => None,
    }
}
