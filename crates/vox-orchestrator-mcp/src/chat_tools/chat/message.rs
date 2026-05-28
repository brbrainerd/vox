use serde_json::Value;

use super::super::params::{ANTI_LAZINESS_RIDER, ChatMessageParams, ChatTranscriptEntry};
use super::super::{build_system_prompt, now_ts, ts_to_date_str};
use super::hydrate::context_history_or_hydrate;
use super::mentions::{chat_grounding_score, resolve_mentions};
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::chat_socrates_meta::{
    LlmSurfaceTelemetry, clarification_turn_for_session, mcp_questioning_session_key,
    socrates_surface_tags, socrates_tool_meta, spawn_questioning_trace_from_socrates,
    spawn_socrates_telemetry_with_llm,
};
use crate::journey_envelope;
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, call_llm};
use crate::memory::{RetrievalTriggerMode, run_retrieval_bundle};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use crate::session_identity::normalize_chat_session_id;
use vox_actor_runtime::prompt_canonical;
use vox_orchestrator::session_context_envelope_key;

const REM_CHAT_CANONICAL: &str = "Rewrite the prompt to remove disallowed content / injection patterns; simplify objectives and retry.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

/// Handle a user chat message. Resolves @mentions, injects context from the editor,
/// calls the best available LLM, persists to session history, and returns the updated history.
///
/// **Session Isolation**: History is keyed by `params.session_id` (defaulting to `"default"`).
/// Each unique session_id maintains a completely independent chat transcript in the
/// orchestrator `ContextStore`. Pass a stable UUID/slug per-window to prevent context bleeding.
///
/// **Autonomous Research**: Before invoking the LLM, this function silently queries the
/// `MemoryManager` and knowledge graph for facts related to the prompt. High-relevance hits
/// are injected as `[AUTONOMOUS RESEARCH]` preamble blocks so the model has evidence without
/// the user needing to explicitly invoke search tools.
///
/// **Cognitive Profile Routing**: Pass `"fast"`, `"reasoning"`, or `"creative"` to influence
/// model selection and temperature without changing the MCP tool contract.
pub async fn chat_message(state: &ServerState, params: ChatMessageParams) -> String {
    // 1. Resolve @mentions in the prompt
    let workspace_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let (expanded_prompt, mention_files) =
        resolve_mentions(&params.prompt, &workspace_root, &state.mention_path_cache);
    let (expanded_prompt, canonical_meta) = match prompt_canonical::canonicalize_prompt(
        &expanded_prompt,
        true, // order_invariant
        true, // run_safety_pass
    ) {
        Ok(c) => {
            let hash = c.original_hash;
            let conflict_count = c.conflict_warnings.len();
            let objective_count = c.objectives.len();
            (c.text, Some((hash, conflict_count, objective_count)))
        }
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("Prompt rejected by safety canonicalizer: {e}"),
                REM_CHAT_CANONICAL,
            )
            .to_json();
        }
    };
    let mention_count = mention_files.len();
    if let Some((original_hash, conflict_count, objective_count)) = canonical_meta {
        tracing::debug!(
            target: "vox_mcp::prompt_canonical",
            original_hash = %original_hash,
            conflict_warning_count = conflict_count,
            objective_count = objective_count,
            "chat prompt canonicalized"
        );
    }

    // 2a. Build context preamble from editor state
    let mut context_parts = Vec::new();

    if let Some(active_file) = &params.active_file {
        let line_info = params
            .active_line
            .map(|l| format!(" (line {l})"))
            .unwrap_or_default();
        context_parts.push(format!("[ACTIVE FILE]: {active_file}{line_info}"));
    }

    if let Some(selected) = &params.selected_text
        && !selected.is_empty()
    {
        context_parts.push(format!("[SELECTED TEXT]:\n{selected}"));
    }

    if !params.diagnostics.is_empty() {
        let diag_str: Vec<String> = params
            .diagnostics
            .iter()
            .filter_map(|d| {
                let msg = d["message"].as_str()?;
                let line = d["line"].as_u64().unwrap_or(0);
                let sev = d["severity"].as_str().unwrap_or("error");
                Some(format!("  Line {line} [{sev}]: {msg}"))
            })
            .collect();
        if !diag_str.is_empty() {
            context_parts.push(format!(
                "[ACTIVE ERRORS/WARNINGS]:\n{}",
                diag_str.join("\n")
            ));
        }
    }

    if !params.open_files.is_empty() {
        context_parts.push(format!("[OPEN FILES]: {}", params.open_files.join(", ")));
    }

    // 2b/2c. Unified autonomous retrieval injection:
    // Use the same retrieval pipeline as `vox_memory_search` with deterministic fallback
    // (hybrid -> BM25 -> lexical fallback), then append memory + knowledge snippets.
    let mut retrieval_evidence = None;
    let retrieval_trace = params
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            params
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        });
    match run_retrieval_bundle(
        state,
        &expanded_prompt,
        RetrievalTriggerMode::AutoChatPreamble,
        3,
        retrieval_trace,
    )
    .await
    {
        Ok(bundle) => {
            if !bundle.rrf_fused_lines.is_empty() {
                let snippets = bundle
                    .rrf_fused_lines
                    .iter()
                    .map(|h| format!("- {h}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — RRF FUSION (tier: {})]:\n{snippets}",
                    bundle.evidence.retrieval_tier
                ));
            }
            if !bundle.memory_lines.is_empty() {
                let snippets = bundle
                    .memory_lines
                    .iter()
                    .map(|h| format!("- {h}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — MEMORY (tier: {})]:\n{snippets}",
                    bundle.evidence.retrieval_tier
                ));
            }
            if !bundle.knowledge_lines.is_empty() {
                let formatted = bundle
                    .knowledge_lines
                    .iter()
                    .map(|n| format!("- {n}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — KNOWLEDGE GRAPH]:\n{formatted}"
                ));
            }
            if !bundle.chunk_lines.is_empty() {
                let formatted = bundle
                    .chunk_lines
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!(
                    "[AUTONOMOUS RESEARCH — DOCUMENT CHUNKS]:\n{formatted}"
                ));
            }
            if !bundle.repo_lines.is_empty() {
                let formatted = bundle
                    .repo_lines
                    .iter()
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                context_parts.push(format!("[AUTONOMOUS RESEARCH — REPOSITORY]:\n{formatted}"));
            }
            retrieval_evidence = Some(bundle.evidence);
        }
        Err(e) => {
            tracing::debug!(
                target: "vox_mcp::autonomous_research",
                error = %e,
                "autonomous retrieval injection failed — continuing without injected context"
            );
        }
    }

    let all_context_files: Vec<String> = {
        let mut v = params.context_files.clone();
        v.extend(mention_files);
        v.dedup();
        v
    };

    let user_prompt = if context_parts.is_empty() {
        expanded_prompt.clone()
    } else {
        format!("{}\n\n{}", context_parts.join("\n"), expanded_prompt)
    };

    // 3. Call LLM with cognitive-profile aware routing.
    // When cognitive_profile is set we use mcp_infer_completion() with an explicit
    // resolution template — the same pattern already used by inline_edit() and ghost_text().
    let (session_id, implicit_session_default) =
        normalize_chat_session_id(params.session_id.as_deref());
    let thread_id_for_envelope = params.thread_id.clone();
    let journey_id = params
        .journey_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "journey_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            )
        });
    if implicit_session_default {
        tracing::debug!(
            target: "vox_mcp::session",
            tool = "vox_chat_message",
            "session_id omitted; using default chat session bucket"
        );
    }
    let ctx_handle = state.orchestrator.context_handle();
    let session_ts =
        match crate::sync_poison::poison_rw_read(ctx_handle.read(), "orchestrator context") {
            Ok(guard) => guard
                .age_secs(&format!("chat_history:{session_id}"))
                .map(|a: u64| format!(" Session last active: {a}s ago."))
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    tool = "vox_chat_message",
                    "context lock poisoned; skipping session age hint"
                );
                String::new()
            }
        };
    let system_prompt = format!(
        "{}{}\n\n{}",
        build_system_prompt(state, None).await,
        session_ts,
        ANTI_LAZINESS_RIDER
    );
    let llm_started = std::time::Instant::now();

    let (response_text, model_used, tokens) = match params.cognitive_profile.as_deref() {
        Some(profile) => {
            let resolution_template = McpChatModelResolution {
                allow_cheapest_fallback: profile == "fast",
                complexity: match profile {
                    "reasoning" => 9,
                    "creative" => 7,
                    _ => 5,
                },
                ..Default::default()
            };
            let base_temperature = if profile == "creative" {
                0.8_f32
            } else {
                0.3_f32
            };
            match resolve_chat_llm_model(
                state,
                &user_prompt,
                resolution_template.clone(),
                Some(session_id.as_str()),
            )
            .await
            {
                Ok((model, free_only)) => {
                    let pref = match crate::sync_poison::poison_rw_read(
                        state.mcp_chat_model_override.read(),
                        "mcp_chat_model_override",
                    ) {
                        Ok(g) => g.clone(),
                        Err(e) => {
                            tracing::warn!(error = %e, "mcp_chat_model_override poisoned");
                            None
                        }
                    };
                    let max_tokens =
                        crate::llm_bridge::clamp_http_max_output_tokens(model.max_tokens);
                    let routing = McpInferRouting {
                        user_prompt: &user_prompt,
                        sticky_model_pref: pref.as_deref(),
                        resolution_template,
                        free_only,
                        allow_cloud_ollama_fallback: true,
                        user_id: Some(session_id.as_str()),
                    };
                    match crate::llm_bridge::mcp_infer_completion(
                        state,
                        model,
                        "vox_chat_message",
                        &system_prompt,
                        &routing,
                        max_tokens,
                        base_temperature,
                        params.temperature,
                        params.top_p,
                        params.json_mode,
                        params.attachment_manifest.clone(),
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            return ToolResult::<String>::err_with_remediation(
                                format!("LLM error: {e}"),
                                REM_LLM_COMPLETION,
                            )
                            .to_json();
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: "vox_mcp::cognitive_routing",
                        profile,
                        error = %e,
                        "cognitive profile model resolution failed — using standard routing"
                    );
                    match call_llm(
                        state,
                        &system_prompt,
                        &user_prompt,
                        Some(session_id.as_str()),
                        params.temperature,
                        params.top_p,
                        params.attachment_manifest.clone(),
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e2) => {
                            return ToolResult::<String>::err_with_remediation(
                                format!("LLM error: {e2}"),
                                REM_LLM_COMPLETION,
                            )
                            .to_json();
                        }
                    }
                }
            }
        }
        None => match call_llm(
            state,
            &system_prompt,
            &user_prompt,
            Some(session_id.as_str()),
            params.temperature,
            params.top_p,
            params.attachment_manifest.clone(),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("LLM error: {e}"),
                    REM_LLM_COMPLETION,
                )
                .to_json();
            }
        },
    };

    let chat_q_key =
        mcp_questioning_session_key(state, "vox_chat_message", Some(session_id.as_str()));
    state.record_questioning_attention_spend(&chat_q_key, llm_started.elapsed().as_millis() as u64);

    tracing::info!(
        target: "vox_mcp::populi_kpi",
        tool = "vox_chat_message",
        model_id = %model_used,
        tokens,
        elapsed_ms = llm_started.elapsed().as_millis() as u64,
        cognitive_profile = params.cognitive_profile.as_deref().unwrap_or("standard"),
        "mcp chat LLM round-trip"
    );

    // 4. Persist to session-scoped history.
    //
    // The history key is derived from `params.session_id` (defaulting to `"default"`).
    // Each distinct value yields an independent key, preventing context bleeding
    // across concurrent VS Code windows, agent threads, or other logical sessions.
    let history_key = format!("chat_history:{session_id}");
    let context_key = session_context_envelope_key(session_id.as_str());

    let user_msg = ChatTranscriptEntry {
        id: format!("usr-{}", now_ts()),
        role: "user".to_string(),
        content: params.prompt.clone(),
        timestamp: now_ts(),
        context_files: all_context_files,
        model_used: None,
        tokens: None,
    };
    let asst_msg = ChatTranscriptEntry {
        id: format!("asst-{}", now_ts() + 1),
        role: "assistant".to_string(),
        content: response_text.clone(),
        timestamp: now_ts() + 1,
        context_files: vec![],
        model_used: Some(model_used.clone()),
        tokens: Some(tokens),
    };

    let mut history =
        context_history_or_hydrate(state, history_key.as_str(), session_id.as_str()).await;
    history.push(user_msg.clone());
    history.push(asst_msg.clone());
    // Keep last 100 messages per session to bound memory usage.
    if history.len() > 100 {
        let trim_to = history.len() - 100;
        history.drain(0..trim_to);
    }

    match serde_json::to_string(&history) {
        Ok(history_json) => {
            let ctx_handle = state.orchestrator.context_handle();
            match crate::sync_poison::poison_rw_write(ctx_handle.write(), "orchestrator context") {
                Ok(ctx) => {
                    ctx.set(vox_orchestrator::AgentId(0), &history_key, &history_json, 0);
                    if let Some(ev) = &retrieval_evidence {
                        let context_envelope = ev.to_context_envelope(
                            state.repository.repository_id.as_str(),
                            Some(session_id.as_str()),
                        );
                        if let Ok(context_json) = serde_json::to_string(&context_envelope) {
                            ctx.set(
                                vox_orchestrator::AgentId(0),
                                &context_key,
                                &context_json,
                                3600,
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "chat_message: context poisoned persisting history");
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                session_id,
                "chat_message: failed to serialize chat history — \
                 history will not persist for this turn"
            );
        }
    }

    if let Some(db) = &state.db {
        let repo_id = &state.repository.repository_id;
        let q_session = session_id.clone();
        let q_repo = repo_id.to_string();

        let route_reason = serde_json::json!({
            "cognitive_profile": params.cognitive_profile,
        });
        let route_reason_s = route_reason.to_string();
        let _ = db
            .record_routing_decision(
                Some(journey_id.as_str()),
                q_repo.as_str(),
                Some(q_session.as_str()),
                "vox_chat_message",
                Some(model_used.as_str()),
                Some(route_reason_s.as_str()),
            )
            .await;

        // Insert user turn
        let user_ctx_files = serde_json::to_string(&user_msg.context_files).unwrap_or_default();
        let _ = db
            .insert_chat_transcript_turn(
                user_msg.id.as_str(),
                q_session.as_str(),
                user_msg.role.as_str(),
                user_msg.content.as_str(),
                user_msg.model_used.as_deref(),
                user_msg.tokens.map(|t| t as i64),
                user_ctx_files.as_str(),
                q_repo.as_str(),
            )
            .await;

        // Insert assistant turn into chat_transcripts (V17 legacy / VS Code history API)
        let asst_ctx_files = serde_json::to_string(&asst_msg.context_files).unwrap_or_default();
        let _ = db
            .insert_chat_transcript_turn(
                asst_msg.id.as_str(),
                q_session.as_str(),
                asst_msg.role.as_str(),
                asst_msg.content.as_str(),
                asst_msg.model_used.as_deref(),
                asst_msg.tokens.map(|t| t as i64),
                asst_ctx_files.as_str(),
                q_repo.as_str(),
            )
            .await;

        let journey_payload = journey_envelope::build_journey_envelope_v1(
            journey_id.as_str(),
            q_session.as_str(),
            thread_id_for_envelope.as_deref(),
            params.trace_id.as_deref(),
            params.correlation_id.as_deref(),
            q_repo.as_str(),
            "mcp",
            params.cognitive_profile.as_deref(),
        );
        let journey_json = journey_payload.to_string();
        if let Ok(conv_id) = db
            .chat_ensure_workspace_conversation(
                q_repo.as_str(),
                q_session.as_str(),
                thread_id_for_envelope.as_deref(),
                "mcp",
            )
            .await
        {
            let _ = db
                .chat_append_workspace_message(
                    conv_id,
                    user_msg.id.as_str(),
                    user_msg.role.as_str(),
                    user_msg.content.as_str(),
                    user_msg.model_used.as_deref(),
                    user_msg.tokens.map(|t| t as i64),
                    Some(user_ctx_files.as_str()),
                    Some(journey_json.as_str()),
                )
                .await;
            let _ = db
                .chat_append_workspace_message(
                    conv_id,
                    asst_msg.id.as_str(),
                    asst_msg.role.as_str(),
                    asst_msg.content.as_str(),
                    asst_msg.model_used.as_deref(),
                    asst_msg.tokens.map(|t| t as i64),
                    Some(asst_ctx_files.as_str()),
                    Some(journey_json.as_str()),
                )
                .await;
        }

        let now_s = now_ts();
        let date_str = ts_to_date_str(now_s);
        let server_idle_secs = now_s.saturating_sub(state.orchestrator.last_activity_ms() / 1000);
        let ctx_handle = state.orchestrator.context_handle();
        let session_age_secs = match crate::sync_poison::poison_rw_read(
            ctx_handle.read(),
            "orchestrator context",
        ) {
            Ok(g) => g
                .age_secs(&format!("chat_history:{session_id}"))
                .unwrap_or(0),
            Err(e) => {
                tracing::warn!(error = %e, "chat_message: context poisoned for session_age_secs");
                0
            }
        };

        // Record high-quality LLM turn in agent_events for Mens replay/SFT
        let mut payload = serde_json::json!({
            "type": "llm_turn",
            "agent_id": 0u64,
            "prompt": user_prompt,
            "response": response_text,
            "model": model_used,
            "tokens": tokens,
            "session_id": q_session,
            "repository_id": state.repository.repository_id,
            "temporal_context": {
                "date": date_str,
                "server_idle_secs": server_idle_secs,
                "session_age_secs": session_age_secs,
            }
        });
        if let Some(ev) = &retrieval_evidence {
            payload["retrieval"] = serde_json::to_value(ev).unwrap_or(Value::Null);
        }
        if vox_gamify::config_gate::is_enabled() {
            let _ = vox_gamify::event_router::route_event_auto_user(db, &payload).await;
        } else {
            let _ =
                vox_gamify::db::insert_event(db, "0", "llm_turn", Some(&payload.to_string())).await;
        }
    }

    // 5. Return updated history + the new assistant message

    let retrieval_contradiction = retrieval_evidence
        .as_ref()
        .map(|e| e.contradiction_count > 0)
        .unwrap_or(false);
    let retrieval_boost = retrieval_evidence
        .as_ref()
        .map(|e| match e.retrieval_tier.as_str() {
            "hybrid" => 0.08_f64,
            "bm25" => 0.04_f64,
            _ => 0.0_f64,
        })
        .unwrap_or(0.0_f64);
    let grounding =
        (chat_grounding_score(&params, mention_count) + retrieval_boost).clamp(0.0, 1.0);
    let pol = state.orchestrator_config.effective_socrates_policy();
    let session_key =
        mcp_questioning_session_key(state, "vox_chat_message", Some(session_id.as_str()));
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(
        &pol,
        grounding,
        retrieval_contradiction,
        turn,
        spent_att,
        max_att,
        retrieval_evidence.as_ref(),
    );
    let mut retrieval_meta = retrieval_evidence
        .as_ref()
        .and_then(|ev| serde_json::to_value(ev).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(meta_obj) = retrieval_meta.as_object_mut() {
        meta_obj.insert(
            "task_class".to_string(),
            serde_json::Value::String("chat_turn".to_string()),
        );
        meta_obj.insert(
            "domain_tags".to_string(),
            serde_json::json!(["interactive", "general_coding"]),
        );
    } else {
        retrieval_meta = socrates_surface_tags("chat_turn", &["interactive", "general_coding"]);
    }
    let llm_turn = state.db.as_ref().map(|_| {
        let provider_slug = model_used
            .split_once('/')
            .map(|(p, _)| p)
            .unwrap_or("openrouter")
            .to_string();
        let strength_tag = params
            .cognitive_profile
            .clone()
            .unwrap_or_else(|| "generalist".to_string());
        LlmSurfaceTelemetry {
            session_id: session_id.clone(),
            user_id: None,
            tenant_id: None,
            prompt: user_prompt.clone(),
            response: response_text.clone(),
            model_id: model_used.clone(),
            provider: provider_slug,
            task_category: "General".to_string(),
            strength_tag,
            latency_ms: llm_started.elapsed().as_millis() as i64,
            input_tokens: None,
            output_tokens: Some(tokens as i64),
            cache_read_tokens: None,
            trace_id: Some(journey_id.clone()),
            success: true,
            cost_usd: None,
            quality_score: Some(1.0),
        }
    });
    spawn_socrates_telemetry_with_llm(
        state,
        "vox_chat_message",
        soc.clone(),
        Some(model_used.clone()),
        Some(retrieval_meta),
        llm_turn,
    );
    spawn_questioning_trace_from_socrates(
        state,
        "vox_chat_message",
        soc.clone(),
        Some(session_key.clone()),
        Some(user_prompt.clone()),
    );
    let result = serde_json::json!({
        "message": asst_msg,
        "history": history,
        "model_used": model_used,
        "tokens": tokens,
        "session_id": session_id,
        "socrates": soc,
        "retrieval": retrieval_evidence,
    });

    ToolResult::ok(result).to_json()
}
