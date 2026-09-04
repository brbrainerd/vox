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
    /// Human-readable reason the model was selected (free-tier router), recorded
    /// on the ModelCall telemetry event. `None` for the ordinary scorer path.
    pub selection_rationale: Option<String>,
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

/// Emit one `orch.cache.miss` `ResearchMetric` event when an LLM call returned
/// without any prompt-cache hit. A miss is defined as "no
/// `cache_read_input_tokens` field, or zero cached tokens". The payload mirrors
/// the relevant subset of `ModelCallEvent` so consumers can join hit/miss rows
/// without extra trace context lookups.
#[allow(clippy::too_many_arguments)]
pub fn emit_cache_miss_if_applicable(
    model_id: &str,
    provider: &str,
    tool: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    cache_read_input_tokens: Option<u32>,
    cache_creation_input_tokens: Option<u32>,
    task_id: Option<u64>,
    trace_id: &str,
) {
    let cache_miss = cache_read_input_tokens.unwrap_or(0) == 0;
    if !cache_miss {
        return;
    }
    let metadata_json = serde_json::json!({
        "model": model_id,
        "provider": provider,
        "tool": tool,
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "cache_creation_input_tokens": cache_creation_input_tokens,
        "task_id": task_id,
        "trace_id": trace_id,
    })
    .to_string();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::ResearchMetric(
        vox_telemetry::ResearchMetricEvent {
            session_id: format!("mcp:{model_id}"),
            metric_type: vox_telemetry::METRIC_TYPE_ORCH_CACHE_MISS.into(),
            metric_value: Some(prompt_tokens as f64),
            metadata_json: Some(metadata_json),
        }
    ));
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

/// Prefix `mcp_infer_tool_completion` prepends to its terminal-failure `String`
/// error when retries are exhausted on a `"rate-limited"` (HTTP 429) failure —
/// this funnel's sibling to `vox_actor_runtime::llm::chat::RATE_LIMITED_PREFIX`.
/// This module's dispatch loop (raw `reqwest`, its own Google-direct/Ollama/
/// secondary-cloud fallback chain — see the module doc for why this funnel is
/// separate from `vox-actor-runtime`'s `llm_chat`) has no structured error type
/// that survives past `e.to_string()` at the terminal-failure return, so — same
/// fix as the context-overflow precedent in `chat.rs` — a plain-text marker
/// callers can `starts_with()` on. Previously this class was only ever surfaced
/// by `vox doctor`'s diagnostic path (Task 12); this prefix is what lets the
/// *live* dispatch path (`call_llm`, `ghost_text`, `inline_edit`, `plan`, etc. —
/// everything routed through `mcp_infer_tool_completion`) distinguish it too.
pub(crate) const RATE_LIMITED_PREFIX: &str = "RATE_LIMITED: ";

/// Applies [`RATE_LIMITED_PREFIX`] to `msg` when `error_class` is `"rate-limited"`.
/// Pulled out of the terminal-failure return in `mcp_infer_tool_completion` as a
/// small pure function so the prefixing decision has cheap, exhaustive unit
/// coverage over every `error_class` branch without standing up a mock server per
/// case. The end-to-end wiring — a real HTTP 429 reaching this call inside the
/// live retry loop — is separately covered by
/// `tests::mcp_infer_tool_completion_prefixes_real_429_from_custom_provider`,
/// which drives a `wiremock::MockServer` through `ProviderType::Custom`'s
/// caller-controlled base URL and the real `infer_via_provider_adapter` dispatch
/// path.
fn apply_rate_limited_prefix(error_class: &str, msg: String) -> String {
    if error_class == "rate-limited" {
        format!("{RATE_LIMITED_PREFIX}{msg}")
    } else {
        msg
    }
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

    // Budget gate, before any dispatch work: this function is the sole caller of
    // `infer_via_provider_adapter`, so it's the real universal convergence point for
    // every caller of `mcp_infer_completion`/`mcp_infer_tool_completion` — including
    // ones that bypass `resolve_chat_llm_model` entirely (`call_llm`, and therefore
    // `browser_tools.rs`'s three direct `call_llm` call sites). See
    // `chat_model_resolve`'s module docs for the full map of why the guard runs at
    // three separate call sites in this crate rather than one.
    crate::chat_model_resolve::enforce_budget_guard(state, routing.user_id).await?;

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
                    return Err(apply_rate_limited_prefix(
                        "rate-limited",
                        if allow_ollama_fallback {
                            "LLM daily quota or rate limit active for this provider; try a local Ollama model or wait."
                        } else {
                            "LLM daily quota or rate limit active for this provider; configure cloud keys, set vox_populi::inference_PROFILE=desktop_ollama or lan_gateway to allow Ollama fallback, or wait."
                        }
                        .into(),
                    ));
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
                        selection_rationale: routing.selection_rationale.clone(),
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

                // Emit one `orch.cache.miss` event when the call ran without
                // any prompt-cache hit (so miss rates can be computed online
                // alongside `cache_read_input_tokens` hits on ModelCallEvent).
                emit_cache_miss_if_applicable(
                    &model.id,
                    &format!("{:?}", model.provider_type),
                    tool,
                    pt,
                    ct,
                    cache_read_input_tokens,
                    cache_creation_input_tokens,
                    trace_ctx.task_id,
                    &trace_ctx.trace_id.to_string(),
                );

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
                let http_status_opt: Option<u16> = if e.status > 0 { Some(e.status) } else { None };
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
                return Err(apply_rate_limited_prefix(error_class, e.to_string()));
            }
        }
    }
}

/// High-level chat used by `vox_chat_message`.
///
/// Sibling of [`call_llm_with_pref`] with no per-request model
/// override/tier — see that function's doc comment for why the two exist.
pub async fn call_llm(
    state: &ServerState,
    system_prompt: &str,
    user_prompt: &str,
    user_id: Option<&str>,
    temperature_override: Option<f32>,
    top_p_override: Option<f32>,
    attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
) -> Result<(String, String, u64), String> {
    call_llm_with_pref(
        state,
        system_prompt,
        user_prompt,
        user_id,
        temperature_override,
        top_p_override,
        attachment_manifest,
        None,
        None,
    )
    .await
}

// vox:defactored-from chat_tools::chat::message 2026-08-31 -- `chat_tools::chat::message`
// is a private submodule of `chat_tools::chat` (only `chat_message` is
// re-exported), so its `effective_model_pref`/`resolution_for_tier` helpers
// cannot be called from here without widening that module's visibility, which
// is out of scope for this bugfix. These are exact copies of the fixed logic;
// keep in sync with
// `crate::chat_tools::chat::message::{effective_model_pref, resolution_for_tier}`
// (see that module's doc comments for the full rationale of each tier arm).
fn fallback_effective_model_pref(request: Option<&str>, global: Option<&str>) -> Option<String> {
    request
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| global.map(str::trim).filter(|s| !s.is_empty()))
        .map(str::to_string)
}

fn fallback_resolution_for_tier(
    tier: Option<&str>,
    base: McpChatModelResolution,
) -> McpChatModelResolution {
    match tier.map(str::trim) {
        Some("local") => McpChatModelResolution {
            enforce_free_tier_only: true,
            allow_cheapest_fallback: true,
            ..base
        },
        Some("mesh") => McpChatModelResolution {
            free_tier_latency_critical: true,
            allow_cheapest_fallback: true,
            ..base
        },
        Some("cloud") => McpChatModelResolution {
            allow_cheapest_fallback: false,
            enforce_free_tier_only: false,
            complexity: base.complexity.max(8),
            ..base
        },
        Some(other) => {
            tracing::warn!(
                target: "vox_mcp::chat_tier",
                tier = other,
                "unrecognized chat tier; falling back to auto/default resolution"
            );
            McpChatModelResolution {
                allow_cheapest_fallback: true,
                ..base
            }
        }
        None => McpChatModelResolution {
            allow_cheapest_fallback: true,
            ..base
        },
    }
}

/// Bug 2 fix (chat-harness-bugfix-and-completion, already-merged-code review):
/// sibling of [`call_llm`] that also accepts the per-request model override and
/// composer tier that `try_run_agent_turn` (`chat_tools::chat::message`)
/// resolves with.
///
/// Before this, when `try_run_agent_turn` failed to resolve a model (e.g. its
/// pick wasn't in the registry, or `enforce_free_tier_only` made a paid pick
/// unresolvable) and `chat_message` fell through to plain `call_llm`, the
/// user's per-request `model_override`/`tier` were silently discarded:
/// `call_llm` resolved using ONLY `state.mcp_chat_model_override` (the
/// process-global override) with `allow_cheapest_fallback: true`
/// unconditionally and no free-tier enforcement. A turn could therefore
/// resolve to a completely different model than the one the user picked (or
/// silently escape a `tier: "local"` free-tier-only request) with no error
/// surfaced. Threading the same preference through both resolution paths
/// means a turn either resolves consistently via one shared preference, or
/// fails consistently -- no more silent substitution.
pub async fn call_llm_with_pref(
    state: &ServerState,
    system_prompt: &str,
    user_prompt: &str,
    user_id: Option<&str>,
    temperature_override: Option<f32>,
    top_p_override: Option<f32>,
    attachment_manifest: Option<vox_orchestrator::attachment_manifest::AttachmentManifest>,
    request_model_override: Option<&str>,
    tier: Option<&str>,
) -> Result<(String, String, u64), String> {
    let global_pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => return Err(e.to_string()),
    };
    let pref = fallback_effective_model_pref(request_model_override, global_pref.as_deref());
    let (model, free_only, resolution_template, selection_rationale) = {
        let orch = &state.orchestrator;
        let context_fill_ratio = super::model_route_policy::mcp_global_llm_context_fill_ratio(orch);
        let resolution_template = fallback_resolution_for_tier(
            tier,
            McpChatModelResolution {
                context_fill_ratio,
                ..Default::default()
            },
        );
        let choice = super::model_route_policy::resolve_mcp_chat_model_with_rationale(
            state,
            user_prompt,
            pref.as_deref(),
            resolution_template.clone(),
            user_id,
        )
        .await?;
        (
            choice.model,
            choice.is_free,
            resolution_template,
            choice.rationale,
        )
    };

    let max_tokens = model.max_tokens.clamp(1, HTTP_MAX_OUTPUT_TOKENS_CAP);
    let routing = McpInferRouting {
        user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        user_id,
        selection_rationale,
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

#[cfg(test)]
#[allow(unsafe_code)] // test-only std::env::set_var (unsafe on edition 2024); serialized via #[serial]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use vox_db::store::types::ModelOutcome;
    use vox_db::{DbConfig, VoxDb};

    fn dummy_model() -> ModelSpec {
        ModelSpec {
            id: "test-model".into(),
            canonical_slug: "test/test-model".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1000,
            cost_per_1k: 0.01,
            cost_per_1k_input: 0.01,
            cost_per_1k_output: 0.01,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![vox_orchestrator::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    /// Spec-review finding: `resolve_chat_llm_model`'s budget gate protects only its
    /// own callers — `call_llm` (used by both `chat_message`'s fallback branches AND
    /// `browser_tools.rs`'s three direct call sites) resolves via
    /// `resolve_mcp_chat_model_with_rationale` directly, bypassing it. This test
    /// proves the *real* universal point for this HTTP path —
    /// `mcp_infer_tool_completion`, the sole caller of `infer_via_provider_adapter` —
    /// refuses before any dispatch work, regardless of which caller (or which model
    /// resolver) got here. No network call is reachable in this test: a
    /// budget-exceeded state must return `Err` before `infer_via_provider_adapter`
    /// would ever be invoked (which would otherwise fail differently — no real HTTP
    /// client/API key in this test environment — proving the gate, not network
    /// failure, is what's being asserted).
    #[tokio::test]
    #[serial]
    async fn mcp_infer_completion_refuses_when_daily_budget_exceeded() {
        let prior = std::env::var("VOX_BUDGET_USD").ok();
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::set_var("VOX_BUDGET_USD", "0.01") };
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db");
        db.record_llm_outcome(ModelOutcome {
            session_id: "infer-completion-test",
            user_id: None,
            tenant_id: None,
            prompt: "p",
            response: "r",
            model_id: "m",
            provider: "openrouter",
            task_category: "general",
            strength_tag: "generalist",
            latency_ms: Some(10),
            input_tokens: Some(5),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            trace_id: None,
            context_utilization_pct: None,
            success: true,
            cost_usd: Some(0.02),
            quality_score: Some(1.0),
            ttft_ms: None,
            tpot_ms: None,
        })
        .await
        .expect("record spend");

        let state = crate::server_state::ServerState::new_test()
            .await
            .with_db_initialized(Arc::new(db))
            .await;

        let routing = McpInferRouting {
            user_prompt: "hello",
            sticky_model_pref: None,
            resolution_template: McpChatModelResolution::default(),
            free_only: false,
            allow_cloud_ollama_fallback: false,
            user_id: Some("infer-completion-test"),
            selection_rationale: None,
        };
        let result = mcp_infer_completion(
            &state,
            dummy_model(),
            "test-tool",
            "system",
            &routing,
            100,
            0.7,
            None,
            None,
            false,
            None,
        )
        .await;

        // SAFETY: `#[serial]` — restore prior env state before asserting/panicking.
        unsafe {
            match &prior {
                Some(v) => std::env::set_var("VOX_BUDGET_USD", v),
                None => std::env::remove_var("VOX_BUDGET_USD"),
            }
        }
        vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

        let err = result.expect_err("expected budget guard to refuse dispatch");
        assert!(
            err.to_lowercase().contains("budget"),
            "expected error to mention budget (not a network/API-key error), got: {err}"
        );
    }

    /// Cross-cutting-review fix: the *local* (pre-dispatch) `GateResult::RateLimited`
    /// branch — driven by `UsageTracker`'s already-recorded rate-limit state via
    /// `BudgetGate::allow_with_pilot_attention`, not by a live HTTP 429 — must also
    /// get `RATE_LIMITED_PREFIX` applied. This is the path a user who already
    /// exhausted their free-tier quota hits on their *next* request (arguably more
    /// common in practice than a fresh live 429), and previously fell through to a
    /// plain, unprefixed string that the GUI's `RATE_LIMITED_PREFIX`-keyed toast
    /// logic wouldn't recognize. Seeds the tracker via `mark_rate_limited` (the same
    /// helper the real HTTP-429 path uses to persist state) so this test drives the
    /// real `allow_with_pilot_attention` -> `GateResult::RateLimited` branch, not a
    /// mocked gate.
    #[tokio::test]
    #[serial]
    async fn mcp_infer_completion_prefixes_local_rate_limited_gate() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db");

        // `is_free: true` so `llm_usage_key()` maps to provider "openrouter",
        // model ":free" — the aggregate key `resolve_provider_limits()`
        // actually tracks a daily limit for by default. A non-free model id
        // has no matching limit row and `allow_with_pilot_attention` would
        // fall through to `Allowed` regardless of `mark_rate_limited`,
        // silently not exercising the branch under test.
        let mut model = dummy_model();
        model.is_free = true;
        let usage = model.llm_usage_key();
        let tracker = vox_orchestrator::usage::UsageTracker::with_user(&db, "infer-rl-test");
        tracker
            .mark_rate_limited(&usage.provider, &usage.model)
            .await
            .expect("seed rate-limited state");

        let state = crate::server_state::ServerState::new_test()
            .await
            .with_db_initialized(Arc::new(db))
            .await;

        let routing = McpInferRouting {
            user_prompt: "hello",
            sticky_model_pref: None,
            resolution_template: McpChatModelResolution::default(),
            free_only: false,
            allow_cloud_ollama_fallback: false,
            user_id: Some("infer-rl-test"),
            selection_rationale: None,
        };
        let result = mcp_infer_completion(
            &state,
            model,
            "test-tool",
            "system",
            &routing,
            100,
            0.7,
            None,
            None,
            false,
            None,
        )
        .await;

        let err = result.expect_err("expected local rate-limit gate to refuse dispatch");
        assert!(
            err.starts_with(RATE_LIMITED_PREFIX),
            "expected RATE_LIMITED_PREFIX-marked error from the local pre-dispatch gate, got: {err}"
        );
    }

    /// Spec-compliance follow-up to Task 12b: a genuine end-to-end proof that a
    /// live HTTP 429 reaches `mcp_infer_tool_completion`'s terminal-failure return
    /// with `RATE_LIMITED_PREFIX` applied — not just the extracted
    /// `apply_rate_limited_prefix` helper in isolation. A `wiremock::MockServer`
    /// always answers 429; the model is `ProviderType::Custom(mock_server.uri())`,
    /// whose base URL is fully caller-controlled (`provider_endpoints::endpoint_for`),
    /// and `OpenAiCompatAdapter::supports()` accepts every provider type except
    /// `GoogleDirect`/`Ollama`, so this routes through the real `reqwest` dispatch
    /// path in `infer_via_provider_adapter`. `allow_cloud_ollama_fallback: false`
    /// and a non-Ollama, non-OpenRouter-Gemini model mean none of the three
    /// fallback branches in the retry loop intercept the error before the
    /// terminal `return Err(apply_rate_limited_prefix(..))` at the bottom of the
    /// loop — so a passing assertion here proves the *wiring*, not just the pure
    /// helper.
    #[tokio::test]
    #[serial]
    async fn mcp_infer_tool_completion_prefixes_real_429_from_custom_provider() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limit exceeded"))
            .mount(&server)
            .await;

        // `OpenAiCompatAdapter::infer` requires `bearer_for(model)` to resolve, which
        // for `ProviderType::Custom` reads `CUSTOM_OPENAI_API_KEY`. Set it for the
        // duration of this test only, restoring the prior value afterward —
        // `#[serial]` (shared with the budget-exceeded test above) prevents
        // concurrent env mutation within this crate's test binary.
        let prior_key = std::env::var("CUSTOM_OPENAI_API_KEY").ok();
        // SAFETY: `#[serial]` — no concurrent env mutation in this crate's tests.
        unsafe { std::env::set_var("CUSTOM_OPENAI_API_KEY", "test-key") };

        let state = crate::server_state::ServerState::new_test().await;

        let mut model = dummy_model();
        model.provider_type = ProviderType::Custom(server.uri());

        let routing = McpInferRouting {
            user_prompt: "hello",
            sticky_model_pref: None,
            resolution_template: McpChatModelResolution::default(),
            free_only: false,
            allow_cloud_ollama_fallback: false,
            user_id: Some("infer-tool-completion-429-test"),
            selection_rationale: None,
        };
        let result = mcp_infer_completion(
            &state,
            model,
            "test-tool",
            "system",
            &routing,
            100,
            0.7,
            None,
            None,
            false,
            None,
        )
        .await;

        // SAFETY: `#[serial]` — restore prior env state before asserting/panicking.
        unsafe {
            match &prior_key {
                Some(v) => std::env::set_var("CUSTOM_OPENAI_API_KEY", v),
                None => std::env::remove_var("CUSTOM_OPENAI_API_KEY"),
            }
        }

        let err = result.expect_err("expected terminal failure from exhausted 429 retries");
        assert!(
            err.starts_with(RATE_LIMITED_PREFIX),
            "expected RATE_LIMITED_PREFIX-marked error from a real 429 dispatch, got: {err}"
        );
        assert!(
            err.contains("429"),
            "expected the underlying HTTP 429 status to survive into the message, got: {err}"
        );
    }

    // Task 12b (free-tier onboarding plan): `mcp_infer_tool_completion`'s terminal
    // failure (`return Err(...)` after all fallbacks are exhausted) must be
    // prefixed with `RATE_LIMITED_PREFIX` when the exhausted error_class is
    // "rate-limited" (HTTP 429), so live dispatch callers (call_llm, ghost_text,
    // inline_edit, plan, ...) can distinguish OpenRouter's free-tier cap from any
    // other backend failure — mirrors `vox_actor_runtime::llm::chat`'s identical
    // fix for its own funnel. `mcp_infer_tool_completion_prefixes_real_429_from_custom_provider`
    // above drives the real HTTP retry-exhaustion path end-to-end via a wiremock
    // 429; this test complements it by exercising the pure prefixing helper's
    // full branch coverage (all error classes) without the overhead of a mock
    // server per case.
    #[test]
    fn rate_limited_error_class_gets_prefixed() {
        let msg = apply_rate_limited_prefix(
            "rate-limited",
            "LLM API error 429: rate limit exceeded".to_string(),
        );
        assert!(
            msg.starts_with(RATE_LIMITED_PREFIX),
            "expected prefixed message, got: {msg}"
        );
        assert_eq!(
            msg,
            format!("{RATE_LIMITED_PREFIX}LLM API error 429: rate limit exceeded")
        );
    }

    #[test]
    fn non_rate_limited_error_classes_are_not_prefixed() {
        for class in ["connection-timeout", "transport-error", "llm-api-error"] {
            let msg = apply_rate_limited_prefix(class, "some error".to_string());
            assert!(
                !msg.starts_with(RATE_LIMITED_PREFIX),
                "error_class {class:?} should not be rate-limited-prefixed, got: {msg}"
            );
            assert_eq!(msg, "some error");
        }
    }

    /// Bug 2 regression test: `call_llm` used to resolve using ONLY
    /// `state.mcp_chat_model_override` (the process-global override), discarding
    /// any per-request pick. Registers two distinct OpenRouter models, sets the
    /// *global* override to one and passes a *different* model id as the
    /// per-request `request_model_override` to `call_llm_with_pref`, then
    /// asserts the model actually dispatched to is the per-request pick — proving
    /// the fallback path now honors the same preference `try_run_agent_turn`
    /// would have used instead of silently substituting the global model.
    #[tokio::test]
    #[allow(unsafe_code)] // env var mutation under a process-wide lock, like other chat tests
    #[allow(clippy::await_holding_lock)]
    async fn call_llm_with_pref_honors_request_override_over_process_global() {
        let _env_guard = crate::chat_tools::chat::agent_loop::CHAT_MESSAGE_ENV_LOCK
            .lock()
            .expect("env lock");
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": "chatcmpl-test",
                    "model": "test-model",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "hi"},
                        "finish_reason": "stop",
                    }],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5},
                })),
            )
            .mount(&server)
            .await;

        let prev_base = std::env::var("OPENROUTER_BASE_URL").ok();
        let prev_key = std::env::var("OPENROUTER_API_KEY").ok();
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", server.uri());
            std::env::set_var("OPENROUTER_API_KEY", "test-key");
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        let state = crate::server_state::ServerState::new_test().await;
        let global_model_id = "call-llm-pref-test-global";
        let request_model_id = "call-llm-pref-test-request";
        {
            let handle = state.orchestrator.models_handle();
            let mut registry = handle.write().expect("models registry lock");
            let mut global = dummy_model();
            global.id = global_model_id.to_string();
            global.canonical_slug = global_model_id.to_string();
            registry.register(global);
            let mut requested = dummy_model();
            requested.id = request_model_id.to_string();
            requested.canonical_slug = request_model_id.to_string();
            registry.register(requested);
        }
        *state.mcp_chat_model_override.write() = Some(global_model_id.to_string());

        let result = call_llm_with_pref(
            &state,
            "system",
            "hello",
            Some("call-llm-pref-test"),
            None,
            None,
            None,
            Some(request_model_id),
            None,
        )
        .await;

        unsafe {
            match prev_base {
                Some(v) => std::env::set_var("OPENROUTER_BASE_URL", v),
                None => std::env::remove_var("OPENROUTER_BASE_URL"),
            }
            match prev_key {
                Some(v) => std::env::set_var("OPENROUTER_API_KEY", v),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        let (_, model_used, _) = result.expect("call_llm_with_pref should succeed");
        assert_eq!(
            model_used, request_model_id,
            "the per-request model_override must win over the process-global override; \
             got {model_used} — the global model was silently substituted instead"
        );
    }
}
