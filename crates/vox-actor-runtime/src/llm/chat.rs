//! Durable chat completion and multi-candidate retry.

use std::future::Future;
use std::pin::Pin;

use super::types::{ChatMessage, LlmConfig, LlmResponse};
use super::wire::{
    OpenRouterRequest, OpenRouterResponse, chat_requires_nonempty_api_key, resolve_chat_api_key,
};
use crate::inference_env::HF_ROUTER_CHAT_COMPLETIONS_URL;
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
            let api_key = resolve_chat_api_key(&config);

            if chat_requires_nonempty_api_key(&config.provider) && api_key.is_empty() {
                return Ok(Err("No API key available for LLM provider".to_string()));
            }

            let base_url = config
                .base_url
                .clone()
                .unwrap_or_else(|| match config.provider.as_str() {
                    "openrouter" => vox_config::OPENROUTER_CHAT_COMPLETIONS_URL.to_string(),
                    "openai" => vox_config::OPENAI_CHAT_COMPLETIONS_URL.to_string(),
                    "hf_router" | "huggingface" => HF_ROUTER_CHAT_COMPLETIONS_URL.to_string(),
                    _ => vox_config::OPENROUTER_CHAT_COMPLETIONS_URL.to_string(),
                });
            if matches!(config.provider.as_str(), "hf_endpoint")
                && (base_url.trim().is_empty()
                    || !base_url.contains("chat/completions"))
            {
                return Ok(Err(
                    "hf_endpoint requires a non-empty chat completions base_url (e.g. …/v1/chat/completions)"
                        .to_string(),
                ));
            }

            let client = vox_http_client::client();
            let req_body = OpenRouterRequest {
                model: &config.model,
                messages: &messages,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                response_format: config.response_format.as_ref(),
                stream: false,
            };

            let mut req = client.post(&base_url).json(&req_body);
            if !api_key.is_empty() {
                req = req.bearer_auth(api_key);
            }
            let start = std::time::Instant::now();
            let res = req
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<no body>"));
                let err_msg = format!("LLM API returned error ({}): {}", status, err_text);
                let latency = start.elapsed().as_millis() as i64;

                let _ = record_telemetry_attempt(&config, "error", latency, Some(&status.to_string())).await;
                {
                    let trace_ctx = vox_telemetry::current_trace_ctx();
                    let error_class = if status.as_u16() == 429 {
                        "rate-limited"
                    } else if status.as_u16() >= 500 {
                        "server-error"
                    } else {
                        "client-error"
                    };
                    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
                        vox_telemetry::ErrorEvent {
                            subsystem: "llm.http".into(),
                            error_class: error_class.into(),
                            http_status: Some(status.as_u16()),
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
                        latency,
                        false,
                    ).await;
                }

                return Ok(Err(err_msg));
            }

            let llm_res: OpenRouterResponse = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

            let content = llm_res
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message)
                .and_then(|m| m.content)
                .unwrap_or_default();

            let usage = llm_res.usage.unwrap_or_default();
            let prompt_tokens = usage.prompt_tokens as i64;
            let completion_tokens = usage.completion_tokens as i64;
            let cache_read_tokens = usage.cache_read_input_tokens
                .or_else(|| usage.prompt_tokens_details.as_ref().map(|d| d.cached_tokens))
                .unwrap_or(0) as i64;
            let provider_cost = usage.total_cost.or(usage.cost);
            let cost_usd = provider_cost.or_else(|| {
                config.cost_per_1k.map(|c| {
                    ((prompt_tokens + completion_tokens) as f64 / 1000.0) * c
                })
            });

            let model_id = llm_res.model.unwrap_or_else(|| config.model.clone());
            let latency = start.elapsed().as_millis() as i64;

            // Telemetry recording
            let _ = record_telemetry_attempt(&config, "success", latency, None).await;

            if !config.telemetry_skip_interaction {
                let _ = record_telemetry_outcome(
                    &config,
                    &messages,
                    &content,
                    &model_id,
                    prompt_tokens,
                    completion_tokens,
                    cache_read_tokens,
                    cost_usd,
                    latency,
                    true,
                ).await;
            }

            Ok(Ok(LlmResponse {
                content,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                model: model_id,
            }))
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
                    None,
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
