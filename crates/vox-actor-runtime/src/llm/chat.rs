//! Durable chat completion and multi-candidate retry.

use std::future::Future;
use std::pin::Pin;

use super::types::{ChatMessage, LlmConfig, LlmResponse};
use crate::{ActivityOptions, ActivityResult, execute_activity};

type LlmChatActivityFuture =
    Pin<Box<dyn Future<Output = Result<Result<LlmResponse, String>, String>> + Send>>;

/// Core durable wrapper for LLM chat (single complete response).
pub async fn llm_chat(
    options: &ActivityOptions,
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> ActivityResult<Result<LlmResponse, String>> {
    let activity_name = format!("llm_chat_{}_{}", config.provider, config.model);

    execute_activity(&activity_name, options, || {
        let messages = messages.clone();
        let config = config.clone();

        let fut = async move {
            // Resolve the provider request once (single-source resolution), then issue
            // the wire call through the sanctioned egress core. The durable-activity
            // wrapper, telemetry, and cost-per-1k fallback stay here in the facade.
            let input = vox_config::resolve_egress::EgressResolveInput {
                provider: config.provider.clone(),
                model: config.model.clone(),
                base_url_override: config.base_url.clone(),
                timeout_ms: config.timeout_ms,
            };
            let ereq = match vox_config::resolve_egress::resolve_egress(&input) {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            let wire_msgs: Vec<vox_llm_egress::ChatMessage> = messages
                .iter()
                .map(|m| vox_llm_egress::ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();
            let wire_tools: Option<Vec<vox_llm_egress::ToolDef>> =
                config.tools.as_ref().map(|ts| {
                    ts.iter()
                        .map(|t| vox_llm_egress::ToolDef {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                        })
                        .collect()
                });
            let params = vox_llm_egress::ChatParams {
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                response_format: config.response_format.as_ref(),
                tools: wire_tools.as_deref(),
                tool_choice: config.tool_choice.as_ref(),
            };

            // Timeout (origin/main feature) is now carried by EgressRequest.timeout_ms,
            // resolved in resolve_egress and applied inside chat_once (unary only).
            match vox_llm_egress::chat_once(&ereq, &wire_msgs, &params).await {
                Ok(resp) => {
                    let prompt_tokens = resp.prompt_tokens as i64;
                    let completion_tokens = resp.completion_tokens as i64;
                    // Provider-reported cost, else the single estimate_cost helper.
                    let cost_usd = resp.cost_usd.or_else(|| {
                        config.cost_per_1k.map(|c| {
                            vox_llm_egress::estimate_cost(
                                resp.prompt_tokens,
                                resp.completion_tokens,
                                c,
                            )
                        })
                    });
                    let latency = resp.latency_ms as i64;

                    let _ = record_telemetry_attempt(&config, "success", latency, None).await;
                    if !config.telemetry_skip_interaction {
                        let _ = record_telemetry_outcome(
                            &config,
                            &messages,
                            &resp.content,
                            &resp.model,
                            prompt_tokens,
                            completion_tokens,
                            resp.cache_read_tokens as i64,
                            cost_usd,
                            latency,
                            true,
                        )
                        .await;
                    }

                    Ok(Ok(LlmResponse {
                        content: resp.content,
                        prompt_tokens: resp.prompt_tokens,
                        completion_tokens: resp.completion_tokens,
                        model: resp.model,
                        cost_usd,
                    }))
                }
                Err(e) => {
                    let (err_msg, http_status, error_class) = match &e {
                        vox_llm_egress::EgressError::RateLimited { .. } => {
                            (e.to_string(), Some(429u16), "rate-limited")
                        }
                        vox_llm_egress::EgressError::Status { code, body } => (
                            format!("LLM API returned error ({}): {}", code, body),
                            Some(*code),
                            if *code >= 500 {
                                "server-error"
                            } else {
                                "client-error"
                            },
                        ),
                        vox_llm_egress::EgressError::Http(m) => (
                            format!("HTTP request failed: {}", m),
                            None,
                            "transport-error",
                        ),
                        vox_llm_egress::EgressError::Decode(m) => (
                            format!("Failed to parse response JSON: {}", m),
                            None,
                            "decode-error",
                        ),
                    };
                    let status_str = http_status.map(|s| s.to_string());
                    let _ =
                        record_telemetry_attempt(&config, "error", 0, status_str.as_deref()).await;
                    {
                        let trace_ctx = vox_telemetry::current_trace_ctx();
                        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
                            vox_telemetry::ErrorEvent {
                                subsystem: "llm.http".into(),
                                error_class: error_class.into(),
                                http_status,
                                retry_attempt: 0,
                                retried: false,
                                model: Some(config.model.clone()),
                                provider: None,
                                task_id: trace_ctx.task_id,
                                trace_id: Some(trace_ctx.trace_id.to_string()),
                            }
                        ));
                    }
                    if !config.telemetry_skip_interaction {
                        let _ = record_telemetry_outcome(
                            &config,
                            &messages,
                            &err_msg,
                            &config.model,
                            0,
                            0,
                            0,
                            None,
                            0,
                            false,
                        )
                        .await;
                    }
                    Ok(Err(err_msg))
                }
            }
        };
        let fut_typed: LlmChatActivityFuture = Box::pin(fut);
        fut_typed
    })
    .await
}

#[allow(unused_variables)]
async fn record_telemetry_outcome(
    config: &LlmConfig,
    messages: &[ChatMessage],
    response: &str,
    model_id: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    cache_read_tokens: i64,
    cost_usd: Option<f64>,
    latency_ms: i64,
    success: bool,
) -> Result<(), String> {
    #[cfg(feature = "database")]
    {
        let session_id = config
            .telemetry_session_id
            .clone()
            .unwrap_or_else(|| "anon-session".to_string());
        let user_id = config.telemetry_user_id.clone();
        let task_category = config
            .telemetry_task_category
            .clone()
            .unwrap_or_else(|| "general".to_string());
        let strength_tag = config
            .telemetry_strength_tag
            .clone()
            .unwrap_or_else(|| "medium".to_string());
        let trace_id = config.telemetry_trace_id.clone();
        let provider = config.provider.clone();
        let model_id_owned = model_id.to_string();
        let response_owned = response.to_string();
        let prompt_owned = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");

        tokio::spawn(async move {
            if let Ok(db) = crate::db::get_db().await {
                let outcome = vox_db::store::types::ModelOutcome {
                    session_id: &session_id,
                    user_id: user_id.as_deref(),
                    prompt: &prompt_owned,
                    response: &response_owned,
                    model_id: &model_id_owned,
                    provider: &provider,
                    task_category: &task_category,
                    strength_tag: &strength_tag,
                    latency_ms: Some(latency_ms),
                    input_tokens: Some(prompt_tokens),
                    output_tokens: Some(completion_tokens),
                    cache_read_tokens: Some(cache_read_tokens),
                    trace_id: trace_id.as_deref(),
                    context_utilization_pct: None,
                    success,
                    cost_usd,
                    quality_score: Some(if success { 1.0 } else { 0.0 }),
                };

                let _ = db.record_unified_llm_turn(outcome, None).await;
            }
        });
    }
    Ok(())
}

#[allow(unused_variables)]
async fn record_telemetry_attempt(
    config: &LlmConfig,
    outcome: &str,
    latency_ms: i64,
    error_class: Option<&str>,
) -> Result<(), String> {
    #[cfg(feature = "database")]
    {
        let trace_id = config
            .telemetry_trace_id
            .clone()
            .unwrap_or_else(|| "anon-trace".to_string());
        let attempt_number = config.telemetry_attempt_number.unwrap_or(1);
        let model_id = config.model.clone();
        let provider = config.provider.clone();
        let outcome_owned = outcome.to_string();
        let error_class_owned = error_class.map(|s| s.to_string());

        tokio::spawn(async move {
            if let Ok(db) = crate::db::get_db().await {
                let attempt = vox_db::store::types::ModelAttempt {
                    trace_id: &trace_id,
                    attempt_number,
                    model_id: &model_id,
                    provider: &provider,
                    outcome: &outcome_owned,
                    latency_ms: Some(latency_ms),
                    error_class: error_class_owned.as_deref(),
                };
                let _ = db.record_llm_attempt(attempt).await;
            }
        });
    }
    Ok(())
}

/// Exhaustive retry loop over multiple candidate LLM configurations.
/// Used for robust agent fallback routing. Iterates models sequentially until
/// one succeeds, skipping specific candidates on 401s or continuing on 429/timeout.
pub async fn infer_with_retry(
    options: &ActivityOptions,
    messages: Vec<ChatMessage>,
    candidates: Vec<LlmConfig>,
) -> ActivityResult<Result<(LlmResponse, LlmConfig), String>> {
    let mut last_error = "No LLM candidates provided".to_string();
    // Inherit trace_id from the ambient TRACE_CTX if one is active (set by dispatch scope);
    // otherwise mint a fresh UUID so orphan calls outside any task still have a trace_id.
    let trace_id = vox_telemetry::current_trace_ctx().trace_id.to_string();
    let mut attempt_number = 0;

    let terminal_fallback = candidates.first().cloned();

    for mut candidate in candidates {
        attempt_number += 1;
        candidate.telemetry_trace_id = Some(trace_id.clone());
        candidate.telemetry_attempt_number = Some(attempt_number);
        candidate.telemetry_skip_interaction = true;

        match llm_chat(options, messages.clone(), candidate.clone()).await {
            ActivityResult::Ok(Ok(response)) => {
                // Record final interaction success
                let _ = record_telemetry_outcome(
                    &candidate,
                    &messages,
                    &response.content,
                    &response.model,
                    response.prompt_tokens as i64,
                    response.completion_tokens as i64,
                    0,
                    response.cost_usd,
                    0,
                    true,
                )
                .await;

                return ActivityResult::Ok(Ok((response, candidate)));
            }
            ActivityResult::Ok(Err(api_err)) => {
                last_error = format!("Candidate {} failed: {}", candidate.model, api_err);
            }
            ActivityResult::Failed(activity_err) => {
                last_error = format!(
                    "Candidate {} activity error: {:?}",
                    candidate.model, activity_err
                );
            }
            ActivityResult::Cancelled => {
                return ActivityResult::Cancelled;
            }
        }
    }

    // Record terminal failure interaction
    if let Some(mut terminal_config) = terminal_fallback {
        terminal_config.telemetry_trace_id = Some(trace_id);
        let _ = record_telemetry_outcome(
            &terminal_config,
            &messages,
            &last_error,
            &terminal_config.model,
            0,
            0,
            0,
            None,
            0,
            false,
        )
        .await;
    }

    ActivityResult::Ok(Err(last_error))
}
