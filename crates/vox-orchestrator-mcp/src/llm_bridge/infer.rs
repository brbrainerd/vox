//! HTTP inference loop: budget gate, provider dispatch, Ollama fallback, usage recording.
//!
//! ## LLM cost bus events (`VOX_MCP_LLM_COST_EVENTS`)
//! After a successful completion, [`should_emit_llm_cost_events`] gates [`vox_orchestrator::AgentEventKind::CostIncurred`] on the
//! orchestrator bus. **Unset env + Codex attached** ⇒ **no bus emit** (usage is already persisted via
//! [`vox_orchestrator::usage::UsageTracker`] / budget paths). **Unset + no DB** ⇒ **emit** so operators still see cost signals.
//! Truthy `1`/`true` forces emits even with DB; `0`/`false` disables. Full semantics: `docs/src/reference/env-vars.md`.

use vox_config::inference_profile_allows_local_ollama_http;
use vox_orchestrator::models::scoring::is_deepseek_off_peak;
use vox_orchestrator::models::{ModelSpec, ProviderType};
use vox_orchestrator::usage::UsageTracker;
use vox_orchestrator::{AgentEventKind, BudgetGate, GateResult};

use crate::server_state::ServerState;

use super::MCP_GLOBAL_LLM_AGENT;
use super::limits::HTTP_MAX_OUTPUT_TOKENS_CAP;
use super::model_route_policy::{McpChatModelResolution, resolve_mcp_chat_model};
use super::provider_adapter::{ProviderInferResult, infer_via_provider_adapter};
use base64::Engine;

/// Routing context for [`mcp_infer_completion`] (sticky override, free-tier policy, Ollama fallback).
#[derive(Clone)]
pub struct McpInferRouting<'a> {
    /// User text used when re-resolving under `enforce_free_tier_only`.
    pub user_prompt: &'a str,
    /// Sticky MCP model id override (same as registry resolve).
    pub sticky_model_pref: Option<&'a str>,
    /// Template merged with `enforce_free_tier_only` on mismatch; `context_fill_ratio` should match resolve.
    pub resolution_template: McpChatModelResolution,
    /// Resolver marked this path as free-tier (`ModelSpec.is_free` should match; enforced at infer).
    pub free_only: bool,
    /// When cloud gate denies (daily cap, in-memory budget) or HTTP fails, try local Ollama.
    /// Effective only if **`vox_populi::inference_PROFILE`** allows local Ollama HTTP (`desktop_ollama` or `lan_gateway`).
    pub allow_cloud_ollama_fallback: bool,
    /// Optional tenant/session usage partition key for centralized accounting.
    pub user_id: Option<&'a str>,
}

/// Whether to emit [`vox_orchestrator::AgentEventKind::CostIncurred`] after LLM success (see module docs for `VOX_MCP_LLM_COST_EVENTS` precedence).
fn should_emit_llm_cost_events(state: &ServerState) -> bool {
    if !vox_telemetry::is_master_enabled() {
        return false;
    }
    match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpLlmCostEvents).expose() {
        Some(v) => {
            let v = v.trim();
            if v == "0" || v.eq_ignore_ascii_case("false") {
                return false;
            }
            if v == "1" || v.eq_ignore_ascii_case("true") {
                return true;
            }
            state.db.is_none()
        }
        None => state.db.is_none(),
    }
}

/// DeepSeek off-peak discount factors applied to both input and output token pricing.
/// Window: UTC 16:30–00:30. V3 = 50% off (factor 0.5); R1 = 75% off (factor 0.25).
fn deepseek_off_peak_discount(model: &ModelSpec) -> f64 {
    if matches!(model.provider_type, ProviderType::DeepSeek) && is_deepseek_off_peak() {
        if model.id.to_ascii_lowercase().contains("r1") {
            0.25 // 75% discount
        } else {
            0.50 // 50% discount
        }
    } else {
        1.0 // no discount
    }
}

fn estimated_cost_usd(
    model: &ModelSpec,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: Option<u32>,
) -> f64 {
    let discount = deepseek_off_peak_discount(model);
    let in_cost = model.cost_per_1k_input * discount;
    let out_cost = model.cost_per_1k_output * discount;
    if in_cost > 0.0 || out_cost > 0.0 {
        let cached = cached_tokens.unwrap_or(0).min(prompt_tokens);
        let non_cached = prompt_tokens - cached;
        // When the model supports prompt caching and the provider reported hits,
        // use cache_read_cost_per_1k for those tokens (typically 10% of input price).
        // Cache-hit pricing is also discounted during off-peak.
        let cache_read_cost = model.cache_read_cost_per_1k * discount;
        let input_cost = if cached > 0 && cache_read_cost > 0.0 {
            (non_cached as f64 / 1000.0) * in_cost + (cached as f64 / 1000.0) * cache_read_cost
        } else {
            (prompt_tokens as f64 / 1000.0) * in_cost
        };
        input_cost + (completion_tokens as f64 / 1000.0) * out_cost
    } else {
        (((prompt_tokens + completion_tokens) as f64) / 1000.0) * model.cost_per_1k * discount
    }
}

/// Prefer larger context, then stable id (registry list order is arbitrary).
async fn best_ollama_model(state: &ServerState) -> Option<ModelSpec> {
    if !inference_profile_allows_local_ollama_http() {
        return None;
    }
    let orch = &state.orchestrator;
    let mut v: Vec<ModelSpec> = vox_orchestrator::sync_lock::rw_read(&*orch.models_handle())
        .list_models()
        .into_iter()
        .filter(|m| matches!(m.provider_type, ProviderType::Ollama))
        .collect();
    v.sort_by(|a, b| {
        b.max_tokens
            .cmp(&a.max_tokens)
            .then_with(|| a.id.cmp(&b.id))
    });
    v.into_iter().next()
}

async fn best_non_ollama_model_except(
    state: &ServerState,
    exclude_model_id: &str,
) -> Option<ModelSpec> {
    let orch = &state.orchestrator;
    let mut v: Vec<ModelSpec> = vox_orchestrator::sync_lock::rw_read(&*orch.models_handle())
        .list_models()
        .into_iter()
        .filter(|m| {
            !matches!(m.provider_type, ProviderType::Ollama)
                && m.id != exclude_model_id
                && !matches!(m.provider_type, ProviderType::Custom(_))
                && model_has_available_credentials(m)
        })
        .collect();
    v.sort_by(|a, b| {
        a.cost_per_1k
            .total_cmp(&b.cost_per_1k)
            .then_with(|| b.max_tokens.cmp(&a.max_tokens))
    });
    v.into_iter().next()
}

fn model_has_available_credentials(model: &ModelSpec) -> bool {
    match model.provider_type {
        ProviderType::GoogleDirect => {
            vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiApiKey)
                .expose()
                .is_some_and(|s| !s.trim().is_empty())
        }
        ProviderType::Ollama => true,
        _ => super::provider_auth::bearer_for(model).is_ok(),
    }
}

fn is_openrouter_gemini_model(model: &ModelSpec) -> bool {
    matches!(model.provider_type, ProviderType::OpenRouter)
        && model.id.to_ascii_lowercase().contains("gemini")
}

fn google_direct_fallback_for_gemini(
    state: &ServerState,
    current: &ModelSpec,
) -> Option<ModelSpec> {
    if !is_openrouter_gemini_model(current) {
        return None;
    }
    if vox_config::GeminiRoutePolicy::from_env() != vox_config::GeminiRoutePolicy::OpenRouterFirst {
        return None;
    }
    vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiApiKey)
        .expose()
        .filter(|s| !s.trim().is_empty())?;
    let targets = vox_config::gemini_route_targets_from_env();
    vox_orchestrator::sync_lock::rw_read::<vox_orchestrator::models::ModelRegistry>(
        &*state.orchestrator.models_handle(),
    )
    .get(&targets.google_direct_model)
}

/// Phase D telemetry helper: emit one `ErrorEvent` for a failed LLM call.
///
/// `retry_attempt` is the 0-based count of retries already attempted before this
/// error. `retried` is `true` when the caller is about to switch to a fallback model
/// (`continue` in the retry loop), `false` when the error is terminal.
fn emit_llm_error_event(
    error_class: &str,
    http_status: Option<u16>,
    retry_attempt: u32,
    retried: bool,
    model_id: &str,
    provider: &str,
) {
    let trace_ctx = vox_telemetry::current_trace_ctx();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
        vox_telemetry::ErrorEvent {
            subsystem: "llm.http".into(),
            error_class: error_class.into(),
            http_status,
            retry_attempt,
            retried,
            model: Some(model_id.to_owned()),
            provider: Some(provider.to_owned()),
            task_id: trace_ctx.task_id,
            trace_id: Some(trace_ctx.trace_id.to_string()),
        }
    ));
}

/// Dispatch a chat completion for MCP tools (inline edit, ghost text, etc.).
pub async fn mcp_infer_completion(
    state: &ServerState,
    model: ModelSpec,
    tool: &str,
    system_prompt: &str,
    routing: &McpInferRouting<'_>,
    max_tokens: u64,
    base_temperature: f32,
    temperature_override: Option<f32>,
    top_p_override: Option<f32>,
    json_mode: bool,
    attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
) -> Result<(String, String, u64), String> {
    mcp_infer_tool_completion(
        state,
        model,
        tool,
        system_prompt,
        routing,
        max_tokens,
        base_temperature,
        temperature_override,
        top_p_override,
        json_mode,
        None,
        None,
        attachment_manifest,
    )
    .await
}

/// Dispatch a chat completion for MCP tools (inline edit, ghost text, etc.) with explicit tools/tool_choice.
#[allow(clippy::too_many_arguments)]
pub async fn mcp_infer_tool_completion(
    state: &ServerState,
    mut model: ModelSpec,
    tool: &str,
    system_prompt: &str,
    routing: &McpInferRouting<'_>,
    max_tokens: u64,
    base_temperature: f32,
    temperature_override: Option<f32>,
    top_p_override: Option<f32>,
    json_mode: bool,
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
    attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
) -> Result<(String, String, u64), String> {
    if tool == "vox_plan" && super::infer_test_stub::infer_stub_env_active() {
        if let Some(body) = super::infer_test_stub::stub_completion_body() {
            let id = super::infer_test_stub::stub_plan_model_spec().id;
            return Ok((body, id, 0));
        }
    }

    let max_t = super::clamp_http_max_output_tokens(max_tokens);
    let client = &state.http_client;
    let allow_ollama_fallback =
        routing.allow_cloud_ollama_fallback && inference_profile_allows_local_ollama_http();
    let mut tried_local_fallback = false;
    let mut tried_google_direct_fallback = false;
    let mut tried_secondary_cloud = false;
    // Phase D: track how many fallback retries have occurred for telemetry.
    let mut retry_attempt: u32 = 0;

    let mut first_pass = true;

    loop {
        if first_pass {
            first_pass = false;
            if routing.free_only && !model.is_free {
                let mut res = routing.resolution_template.clone();
                res.enforce_free_tier_only = true;
                match resolve_mcp_chat_model(
                    state,
                    routing.user_prompt,
                    routing.sticky_model_pref,
                    res,
                    routing.user_id,
                )
                .await
                {
                    Ok((m, _)) => model = m,
                    Err(e) => return Err(e),
                }
            } else if routing.free_only != model.is_free {
                tracing::debug!(
                    target: "vox.mcp.llm",
                    free_only = routing.free_only,
                    model_is_free = model.is_free,
                    model_id = %model.id,
                    "mcp_infer_completion: free_only flag disagrees with ModelSpec.is_free"
                );
            }
        }

        let usage = model.llm_usage_key();

        let mut estimated_vision_tokens = 0;
        if let Some(ref manifest) = attachment_manifest {
            for a in &manifest.attachments {
                if a.mime_type.starts_with("image/") {
                    // Safe heuristic: 85 base + ~6 tiles (1020) ~ 1000 tokens per image.
                    estimated_vision_tokens += 1000;
                }
            }
        }

        if let Some(db) = state.db.as_ref() {
            let orch_arc = state.orchestrator.budget_manager_handle();
            let orch_attention = {
                let g = vox_orchestrator::sync_lock::rw_read(&*orch_arc);
                g.attention_snapshot()
            };
            let tracker = if let Some(user_id) = routing.user_id {
                UsageTracker::with_user(db.as_ref(), user_id)
            } else {
                UsageTracker::new_ref(db.as_ref())
            };
            let gate = BudgetGate::new(
                state.budget_manager.as_ref(),
                &tracker,
                &state.orchestrator_config,
            );
            match gate
                .allow_with_pilot_attention(
                    MCP_GLOBAL_LLM_AGENT,
                    &usage,
                    Some(orch_attention),
                    estimated_vision_tokens,
                )
                .await
            {
                GateResult::Allowed => {}
                GateResult::BudgetExceeded { message } => {
                    if allow_ollama_fallback && !matches!(model.provider_type, ProviderType::Ollama)
                    {
                        if let Some(fb) = best_ollama_model(state).await {
                            model = fb;
                            tried_local_fallback = true;
                            continue;
                        }
                    }
                    if matches!(model.provider_type, ProviderType::Ollama) && !tried_secondary_cloud
                    {
                        if let Some(fb) = best_non_ollama_model_except(state, &model.id).await {
                            model = fb;
                            tried_secondary_cloud = true;
                            continue;
                        }
                    }
                    return Err(message);
                }
                GateResult::RateLimited { .. } => {
                    if allow_ollama_fallback && !matches!(model.provider_type, ProviderType::Ollama)
                    {
                        if let Some(fb) = best_ollama_model(state).await {
                            model = fb;
                            tried_local_fallback = true;
                            continue;
                        }
                    }
                    if matches!(model.provider_type, ProviderType::Ollama) && !tried_secondary_cloud
                    {
                        if let Some(fb) = best_non_ollama_model_except(state, &model.id).await {
                            model = fb;
                            tried_secondary_cloud = true;
                            continue;
                        }
                    }
                    return Err(
                        if allow_ollama_fallback {
                            "LLM daily quota or rate limit active for this provider; try a local Ollama model or wait."
                        } else {
                            "LLM daily quota or rate limit active for this provider; configure cloud keys, set vox_populi::inference_PROFILE=desktop_ollama or lan_gateway to allow Ollama fallback, or wait."
                        }
                        .into(),
                    );
                }
                GateResult::AttentionExhausted { message, .. } => {
                    return Err(message);
                }
                GateResult::BehavioralTestFailed { message } => {
                    return Err(message);
                }
                // `DoomLoop` is currently produced only by the task-submission
                // gate (`BudgetGate::check_doom_loop` in `task_submit.rs`), not
                // by the budget/attention gates above this match. The arm is
                // present for exhaustiveness and to ensure correct behavior if
                // a future change adds doom-loop checking at LLM-call granularity.
                GateResult::DoomLoop { message } => {
                    return Err(message);
                }
            }
        }

        let chatml_collapsed: Option<String> = if state.orchestrator_config.chatml_strict {
            Some(format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                vox_config::sanitize_chatml(system_prompt),
                vox_config::sanitize_chatml(routing.user_prompt)
            ))
        } else {
            None
        };
        let (final_system, final_user): (&str, &str) = if let Some(ref collapsed) = chatml_collapsed
        {
            ("", collapsed.as_str())
        } else {
            (system_prompt, routing.user_prompt)
        };

        let mut user_parts = vec![vox_openai::ChatMessagePart::Text { text: final_user }];
        if let Some(ref manifest) = attachment_manifest {
            if let Some(db) = state.db.as_ref() {
                for attachment in &manifest.attachments {
                    if attachment.mime_type.starts_with("image/") {
                        match db.get(&attachment.sha256).await {
                            Ok(bytes) => {
                                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                let url = format!("data:{};base64,{}", attachment.mime_type, b64);
                                user_parts.push(vox_openai::ChatMessagePart::ImageUrl {
                                    image_url: vox_openai::ImageUrl {
                                        url: Box::leak(url.into_boxed_str()),
                                    },
                                });
                            }
                            Err(e) => {
                                tracing::warn!(sha = %attachment.sha256, error = %e, "Failed to fetch attachment from CAS");
                            }
                        }
                    }
                }
            }
        }

        let user_content = if user_parts.len() > 1 {
            vox_openai::ChatMessageContent::Parts(user_parts)
        } else {
            vox_openai::ChatMessageContent::Text(final_user)
        };

        let temperature = temperature_override
            .or(match model.provider_type {
                ProviderType::GoogleDirect => vox_config::gemini_tuning_temperature(),
                ProviderType::Ollama => vox_config::ollama_tuning_temperature(),
                ProviderType::OpenRouter | ProviderType::Custom(_) => {
                    vox_config::openai_tuning_temperature()
                }
                ProviderType::Anthropic => vox_config::anthropic_tuning_temperature(),
                _ => None,
            })
            .unwrap_or(base_temperature);

        let top_p = top_p_override.or(match model.provider_type {
            ProviderType::GoogleDirect => vox_config::gemini_tuning_top_p(),
            ProviderType::Ollama => vox_config::ollama_tuning_top_p(),
            ProviderType::OpenRouter | ProviderType::Custom(_) => vox_config::openai_tuning_top_p(),
            ProviderType::Anthropic => vox_config::anthropic_tuning_top_p(),
            _ => None,
        });

        tracing::info!(
            target: "vox.mcp.llm.tuning",
            model_id = %model.id,
            tool = %tool,
            temperature = %temperature,
            top_p = ?top_p,
            "inference tuning active"
        );

        let infer_start = std::time::Instant::now();
        let infer_result = infer_via_provider_adapter(
            client,
            &model,
            final_system,
            user_content,
            max_t,
            Some(temperature),
            top_p,
            json_mode,
            tools.clone(),
            tool_choice.clone(),
        )
        .await;

        match infer_result {
            Ok(ProviderInferResult {
                text,
                prompt_tokens: pt,
                completion_tokens: ct,
                provider_request_id,
                provider_reported_cost_usd,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            }) => {
                let total_tok = (pt + ct) as u64;
                // For cost estimation, combine cache-read and cache-creation tokens since
                // estimated_cost_usd applies cache_read_cost_per_1k to the combined cached count.
                let cached_for_cost = match (cache_read_input_tokens, cache_creation_input_tokens) {
                    (None, None) => None,
                    (r, c) => Some(r.unwrap_or(0) + c.unwrap_or(0)),
                };
                let estimated_usd = estimated_cost_usd(&model, pt, ct, cached_for_cost);
                let (reconciled_usd, cost_source) = match provider_reported_cost_usd {
                    Some(provider_usd) => (provider_usd, "provider_reported"),
                    None => (estimated_usd, "estimated"),
                };
                let infer_latency_ms = infer_start.elapsed().as_millis() as u64;
                // Phase C: populate trace fields from the ambient TRACE_CTX set by
                // dispatch::handle_tool_call's TRACE_CTX::scope wrapper.  Outside any
                // dispatch scope the default context still provides a fresh UUID trace_id,
                // which preserves the prior per-call behavior for orphan callers.
                let trace_ctx = vox_telemetry::current_trace_ctx();

                vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::ModelCall(
                    vox_telemetry::ModelCallEvent {
                        model: model.id.clone(),
                        provider: format!("{:?}", model.provider_type),
                        route_profile: None,
                        prompt_tokens: pt,
                        completion_tokens: ct,
                        cache_read_input_tokens,
                        cache_creation_input_tokens,
                        latency_ms: infer_latency_ms,
                        cost_usd: reconciled_usd,
                        cost_source: cost_source.to_string(),
                        error_class: None,
                        retry_attempt,
                        task_id: trace_ctx.task_id,
                        parent_task_id: trace_ctx.parent_task_id,
                        trace_id: Some(trace_ctx.trace_id.to_string()),
                        caller_agent_id: trace_ctx.caller_agent_id,
                    }
                ));

                if let Some(cached) = cache_read_input_tokens {
                    tracing::debug!(
                        target: "vox.mcp.llm.cache",
                        model_id = %model.id,
                        tool = %tool,
                        cached_tokens = cached,
                        prompt_tokens = pt,
                        cache_pct = %format!("{:.1}%", (cached as f64 / pt.max(1) as f64) * 100.0),
                        "prompt cache hit"
                    );
                }

                if let Some(db) = state.db.as_ref() {
                    let tracker = if let Some(user_id) = routing.user_id {
                        UsageTracker::with_user(db.as_ref(), user_id)
                    } else {
                        UsageTracker::new_ref(db.as_ref())
                    };
                    let gate = BudgetGate::new(
                        state.budget_manager.as_ref(),
                        &tracker,
                        &state.orchestrator_config,
                    );
                    gate.record_usage_detailed(
                        MCP_GLOBAL_LLM_AGENT,
                        &usage,
                        pt as u64,
                        ct as u64,
                        reconciled_usd,
                        provider_request_id.as_deref(),
                        provider_reported_cost_usd,
                        Some(estimated_usd),
                        Some(reconciled_usd),
                        Some(cost_source),
                        None,
                    )
                    .await;
                }

                if should_emit_llm_cost_events(state) {
                    let orch = &state.orchestrator;
                    orch.event_bus().emit(AgentEventKind::CostIncurred {
                        agent_id: MCP_GLOBAL_LLM_AGENT,
                        provider: usage.provider.clone(),
                        model: model.id.clone(),
                        input_tokens: pt,
                        output_tokens: ct,
                        cost_usd: reconciled_usd,
                        temporal_context: Some(serde_json::json!({
                            "tool": tool,
                            "provider_request_id": provider_request_id,
                            "user_id": routing.user_id,
                            "cost_source": cost_source,
                            "cache_read_input_tokens": cache_read_input_tokens,
                            "cache_creation_input_tokens": cache_creation_input_tokens,
                        })),
                    });
                }

                if matches!(model.provider_type, ProviderType::PopuliMesh) {
                    if let Some(db) = state.db.as_ref() {
                        let parts: Vec<&str> = model.id.split('/').collect();
                        if parts.len() >= 2 {
                            let node_id = parts[1];
                            let _ = db.record_peer_reputation(node_id, "success").await;
                        }
                    }
                }

                return Ok((text, model.id, total_tok));
            }
            Err(e) => {
                if matches!(model.provider_type, ProviderType::PopuliMesh) {
                    if let Some(db) = state.db.as_ref() {
                        let parts: Vec<&str> = model.id.split('/').collect();
                        if parts.len() >= 2 {
                            let node_id = parts[1];
                            let event_type = if e.status == 408
                                || e.status == 504
                                || e.message.to_ascii_lowercase().contains("timeout")
                            {
                                "timeout"
                            } else {
                                "fail"
                            };
                            let _ = db.record_peer_reputation(node_id, event_type).await;
                        }
                    }
                }

                // Persist rate-limit state for budget tracking (unchanged).
                if e.status == 429 {
                    if let Some(db) = state.db.as_ref() {
                        let tracker = if let Some(user_id) = routing.user_id {
                            UsageTracker::with_user(db.as_ref(), user_id)
                        } else {
                            UsageTracker::new_ref(db.as_ref())
                        };
                        let _ = tracker
                            .mark_rate_limited(&usage.provider, &usage.model)
                            .await;
                    }
                }

                // Phase D: classify error once, then emit at each decision branch so
                // `retried` accurately reflects whether a fallback is actually taken.
                let error_class: &str = if e.status == 429 {
                    "rate-limited"
                } else if e.status == 408
                    || e.status == 504
                    || e.message.to_ascii_lowercase().contains("timeout")
                {
                    "connection-timeout"
                } else if e.status == 0 {
                    "transport-error"
                } else {
                    "llm-api-error"
                };
                let http_status_opt: Option<u16> =
                    if e.status > 0 { Some(e.status) } else { None };
                let provider_str = format!("{:?}", model.provider_type);

                if !tried_google_direct_fallback {
                    if let Some(fb) = google_direct_fallback_for_gemini(state, &model) {
                        emit_llm_error_event(
                            error_class,
                            http_status_opt,
                            retry_attempt,
                            true,
                            &model.id,
                            &provider_str,
                        );
                        model = fb;
                        tried_google_direct_fallback = true;
                        retry_attempt += 1;
                        continue;
                    }
                }
                if allow_ollama_fallback
                    && !tried_local_fallback
                    && !matches!(model.provider_type, ProviderType::Ollama)
                {
                    if let Some(fb) = best_ollama_model(state).await {
                        emit_llm_error_event(
                            error_class,
                            http_status_opt,
                            retry_attempt,
                            true,
                            &model.id,
                            &provider_str,
                        );
                        model = fb;
                        tried_local_fallback = true;
                        retry_attempt += 1;
                        continue;
                    }
                }
                if matches!(model.provider_type, ProviderType::Ollama) && !tried_secondary_cloud {
                    if let Some(fb) = best_non_ollama_model_except(state, &model.id).await {
                        emit_llm_error_event(
                            error_class,
                            http_status_opt,
                            retry_attempt,
                            true,
                            &model.id,
                            &provider_str,
                        );
                        model = fb;
                        tried_secondary_cloud = true;
                        retry_attempt += 1;
                        continue;
                    }
                }
                // No fallback available — terminal failure.
                emit_llm_error_event(
                    error_class,
                    http_status_opt,
                    retry_attempt,
                    false,
                    &model.id,
                    &provider_str,
                );
                return Err(e.to_string());
            }
        }
    }
}

/// High-level chat used by `vox_chat_message`.
pub async fn call_llm(
    state: &ServerState,
    system_prompt: &str,
    user_prompt: &str,
    user_id: Option<&str>,
    temperature_override: Option<f32>,
    top_p_override: Option<f32>,
    attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
) -> Result<(String, String, u64), String> {
    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => return Err(e.to_string()),
    };
    let (model, free_only, resolution_template) = {
        let orch = &state.orchestrator;
        let context_fill_ratio = super::model_route_policy::mcp_global_llm_context_fill_ratio(orch);
        let resolution_template = McpChatModelResolution {
            allow_cheapest_fallback: true,
            context_fill_ratio,
            ..Default::default()
        };
        let (model, free_only) = resolve_mcp_chat_model(
            state,
            user_prompt,
            pref.as_deref(),
            resolution_template.clone(),
            user_id,
        )
        .await?;
        (model, free_only, resolution_template)
    };

    let max_tokens = model.max_tokens.clamp(1, HTTP_MAX_OUTPUT_TOKENS_CAP);
    let routing = McpInferRouting {
        user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        user_id,
    };
    mcp_infer_completion(
        state,
        model,
        "mcp_chat",
        system_prompt,
        &routing,
        max_tokens,
        0.7,
        temperature_override,
        top_p_override,
        false,
        attachment_manifest,
    )
    .await
}
