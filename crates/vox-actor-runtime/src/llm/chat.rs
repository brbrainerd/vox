//! Durable chat completion and multi-candidate retry.
//!
//! ## Context-overflow signal (Task 2.3)
//!
//! `llm_chat`'s error type is a plain `String` (`Result<LlmResponse, String>`), so
//! there is no structured slot to carry "this failed because the prompt exceeded the
//! model's context window" across that boundary. Rather than widening the error type
//! (a larger, more invasive change — every caller of `llm_chat` across the codebase
//! matches on `String` today), the error message is prefixed with the documented
//! [`CONTEXT_EXCEEDED_PREFIX`] marker whenever
//! `vox_llm_egress::EgressError::is_context_exceeded()` reports true. Callers that
//! care can check `err_msg.starts_with(CONTEXT_EXCEEDED_PREFIX)`; callers that don't
//! see an ordinary human-readable error string (the prefix is plain text, not a
//! sentinel byte).
//!
//! ## Rate-limit signal (Task 12b)
//!
//! Same problem, same fix, for `vox_llm_egress::EgressError::RateLimited` (e.g.
//! OpenRouter's free-tier 50/day cap): the error message is prefixed with
//! [`RATE_LIMITED_PREFIX`]. Callers check `err_msg.starts_with(RATE_LIMITED_PREFIX)`.
//! Previously this class was only ever surfaced by `vox doctor`'s diagnostic path
//! (Task 12); this prefix is what lets the *live* chat dispatch path
//! (`try_run_agent_turn`, via `llm_chat`) distinguish it too.

use std::future::Future;
use std::pin::Pin;

use super::types::{ChatMessage, LlmConfig, LlmResponse};
use crate::{ActivityOptions, ActivityResult, execute_activity};

/// Prefix `llm_chat` prepends to its `String` error when
/// `vox_llm_egress::EgressError::is_context_exceeded()` classifies the underlying
/// provider failure as a context-window overflow. See the module doc for why a
/// string prefix rather than a richer error type. Kept `pub` so callers can detect
/// this class of failure (e.g. to surface an actionable message, or as the anchor
/// point for a future retry-with-larger-context-model cascade) without re-parsing the
/// message body themselves.
pub const CONTEXT_EXCEEDED_PREFIX: &str = "CONTEXT_LENGTH_EXCEEDED: ";

/// `error_class` tag `map_egress_error` assigns to `vox_llm_egress::EgressError::RateLimited`.
/// Kept `pub` (and re-exported via `vox_actor_runtime::llm`) so downstream callers —
/// e.g. `vox-cli`'s `vox doctor` rate-limit check, which reads this same tag back out of
/// the `llm_attempts.error_class` DB column written by [`record_telemetry_attempt`] —
/// can match on this constant instead of re-guessing the string literal.
pub const RATE_LIMITED_ERROR_CLASS: &str = "rate-limited";

/// Prefix `llm_chat` prepends to its `String` error when the underlying provider
/// failure is `vox_llm_egress::EgressError::RateLimited` (e.g. OpenRouter's
/// free-tier 50/day cap) — the live-dispatch sibling of [`CONTEXT_EXCEEDED_PREFIX`],
/// same rationale: no structured error type to carry this across the `Result<_,
/// String>` boundary, so a plain-text marker callers can `starts_with()` on. See
/// `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`'s
/// `RATE_LIMITED_PREFIX` for this crate's sibling funnel (raw-`reqwest` dispatch,
/// a separate HTTP-issuing path — see that module's doc for why there are two).
pub const RATE_LIMITED_PREFIX: &str = "RATE_LIMITED: ";

/// Maps a `vox_llm_egress::EgressError` to `llm_chat`'s `(message, http_status,
/// error_class)` triple. Pulled out of the `llm_chat` closure body so the
/// context-overflow prefixing (and the rest of the classification) is unit-testable
/// without spinning up the whole durable-activity/telemetry plumbing.
fn map_egress_error(e: &vox_llm_egress::EgressError) -> (String, Option<u16>, &'static str) {
    match e {
        vox_llm_egress::EgressError::RateLimited { .. } => (
            format!("{RATE_LIMITED_PREFIX}{e}"),
            Some(429u16),
            RATE_LIMITED_ERROR_CLASS,
        ),
        vox_llm_egress::EgressError::Status { code, body } => {
            let msg = format!("LLM API returned error ({}): {}", code, body);
            if e.is_context_exceeded() {
                (
                    format!("{CONTEXT_EXCEEDED_PREFIX}{msg}"),
                    Some(*code),
                    "context-exceeded",
                )
            } else {
                (
                    msg,
                    Some(*code),
                    if *code >= 500 {
                        "server-error"
                    } else {
                        "client-error"
                    },
                )
            }
        }
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
    }
}

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
                api_key_override: config.api_key.clone(),
            };
            let ereq = match vox_config::resolve_egress::resolve_egress(&input) {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            // `tool_calls`/`tool_call_id`/`name` pass straight through unchanged:
            // `LlmChatMessage::tool_calls` already reuses `vox_llm_egress::EgressToolCall`
            // (no JSON-string/Value conversion needed at this layer — that conversion is
            // the wire layer's job, in `vox_llm_egress::wire::build_request`, since it's
            // the one place with a concrete outbound wire format to target).
            let wire_msgs: Vec<vox_llm_egress::ChatMessage> = messages
                .iter()
                .map(|m| vox_llm_egress::ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    tool_calls: m.tool_calls.clone(),
                    tool_call_id: m.tool_call_id.clone(),
                    name: m.name.clone(),
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
                top_p: config.top_p,
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
                    // Non-streaming: no partial data exists, so "first token" and "whole
                    // response" arrive at the same instant (mirrors the LlmResponse
                    // construction below).
                    let tpot_ms =
                        (completion_tokens > 0).then(|| latency as f64 / completion_tokens as f64);

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
                            Some(latency),
                            tpot_ms,
                        )
                        .await;
                    }

                    Ok(Ok(LlmResponse {
                        content: resp.content,
                        prompt_tokens: resp.prompt_tokens,
                        completion_tokens: resp.completion_tokens,
                        model: resp.model,
                        cost_usd,
                        tool_calls: resp.tool_calls,
                        latency_ms: resp.latency_ms,
                        cache_read_tokens: resp.cache_read_tokens,
                        ttft_ms: Some(resp.latency_ms),
                        tpot_ms,
                    }))
                }
                Err(e) => {
                    let (err_msg, http_status, error_class) = map_egress_error(&e);
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
                            None,
                            None,
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
#[allow(clippy::too_many_arguments)]
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
    ttft_ms: Option<i64>,
    tpot_ms: Option<f64>,
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
                    tenant_id: None,
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
                    // WARNING (Task M2): this is not a quality signal. It is a restatement
                    // of `success`, and the only other writer of
                    // `model_scoreboard.quality_score` is `COALESCE(AVG(llm_feedback.rating)
                    // / 5.0, 1.0)` over a table with zero rows, i.e. a constant 1.0. Do not
                    // rank, render, or reward on it until M2 gives it a definition.
                    quality_score: Some(if success { 1.0 } else { 0.0 }),
                    ttft_ms,
                    tpot_ms,
                };

                let _ = db.record_unified_llm_turn(outcome, None).await;
            }
        });
    }
    Ok(())
}

#[allow(unused_variables)]
pub(super) async fn record_telemetry_attempt(
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

/// Sequential fallback over multiple candidate LLM configurations.
///
/// Tries each candidate exactly once, in order, and returns the first success.
/// There is deliberately no per-candidate retry and no error classification
/// here: 401, 429, 5xx, timeout, and transport failures all take the same
/// branch and advance to the next candidate. Cancellation is the one
/// exception -- it returns immediately rather than advancing.
///
/// Rate-limit backoff is not this function's job. `vox_llm_egress::wire` hands
/// the `Retry-After` header to `throttle::on_rate_limited` before surfacing
/// `EgressError::RateLimited`; that halves the provider's concurrency window
/// and sets a cooldown which the next `acquire_permit` awaits.
///
/// Callers needing genuine provider fallback must pass a multi-candidate
/// vector; `vec![cfg]` yields exactly one attempt with no fallback.
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
                // Record final interaction success. `latency_ms` and
                // `cache_read_tokens` used to be hardcoded `0` here, which dragged
                // every latency aggregate in `model_scoreboard` toward zero; they now
                // come from the egress response the served candidate produced.
                let _ = record_telemetry_outcome(
                    &candidate,
                    &messages,
                    &response.content,
                    &response.model,
                    response.prompt_tokens as i64,
                    response.completion_tokens as i64,
                    response.cache_read_tokens as i64,
                    response.cost_usd,
                    response.latency_ms as i64,
                    true,
                    response.ttft_ms.map(|v| v as i64),
                    response.tpot_ms,
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
            None,
            None,
        )
        .await;
    }

    ActivityResult::Ok(Err(last_error))
}

#[cfg(test)]
mod context_exceeded_tests {
    use super::*;

    #[test]
    fn status_error_with_overflow_body_is_prefixed_and_classified() {
        let e = vox_llm_egress::EgressError::Status {
            code: 400,
            body: "This model's maximum context length is 4096 tokens.".into(),
        };
        let (msg, status, class) = map_egress_error(&e);
        assert!(
            msg.starts_with(CONTEXT_EXCEEDED_PREFIX),
            "expected prefixed message, got: {msg}"
        );
        assert_eq!(status, Some(400));
        assert_eq!(class, "context-exceeded");
    }

    #[test]
    fn status_error_with_unrelated_body_is_not_prefixed() {
        let e = vox_llm_egress::EgressError::Status {
            code: 401,
            body: "Invalid API key provided.".into(),
        };
        let (msg, status, class) = map_egress_error(&e);
        assert!(!msg.starts_with(CONTEXT_EXCEEDED_PREFIX));
        assert_eq!(status, Some(401));
        assert_eq!(class, "client-error");
    }

    #[test]
    fn rate_limited_and_transport_errors_are_never_context_exceeded() {
        let (_, _, class) =
            map_egress_error(&vox_llm_egress::EgressError::RateLimited { retry_after: None });
        assert_eq!(class, RATE_LIMITED_ERROR_CLASS);

        let (_, _, class) = map_egress_error(&vox_llm_egress::EgressError::Http(
            "connection reset".into(),
        ));
        assert_eq!(class, "transport-error");

        let (_, _, class) =
            map_egress_error(&vox_llm_egress::EgressError::Decode("bad json".into()));
        assert_eq!(class, "decode-error");
    }
}

#[cfg(test)]
mod rate_limited_prefix_tests {
    use super::*;

    #[test]
    fn rate_limited_error_is_prefixed_and_classified() {
        let e = vox_llm_egress::EgressError::RateLimited { retry_after: None };
        let (msg, status, class) = map_egress_error(&e);
        assert!(
            msg.starts_with(RATE_LIMITED_PREFIX),
            "expected prefixed message, got: {msg}"
        );
        assert_eq!(status, Some(429));
        assert_eq!(class, RATE_LIMITED_ERROR_CLASS);
    }

    #[test]
    fn non_rate_limited_errors_are_never_rate_limited_prefixed() {
        let (msg, _, _) = map_egress_error(&vox_llm_egress::EgressError::Status {
            code: 400,
            body: "This model's maximum context length is 4096 tokens.".into(),
        });
        assert!(!msg.starts_with(RATE_LIMITED_PREFIX));

        let (msg, _, _) = map_egress_error(&vox_llm_egress::EgressError::Status {
            code: 401,
            body: "Invalid API key provided.".into(),
        });
        assert!(!msg.starts_with(RATE_LIMITED_PREFIX));

        let (msg, _, _) = map_egress_error(&vox_llm_egress::EgressError::Http(
            "connection reset".into(),
        ));
        assert!(!msg.starts_with(RATE_LIMITED_PREFIX));

        let (msg, _, _) = map_egress_error(&vox_llm_egress::EgressError::Decode("bad json".into()));
        assert!(!msg.starts_with(RATE_LIMITED_PREFIX));
    }
}

/// Regression: `infer_with_retry` must record the real elapsed time.
///
/// Its success arm passed a literal `0` for both `latency_ms` and
/// `cache_read_tokens` into `record_telemetry_outcome`, so every
/// `llm_interactions.latency_ms` row written through the retry/fallback path
/// (the path the orchestrator actually uses) was zero.
#[cfg(test)]
mod infer_with_retry_latency_tests {
    use super::*;
    use crate::llm::types::LlmConfig;

    fn mock_config(base_url: String) -> LlmConfig {
        LlmConfig {
            provider: "openrouter".into(),
            model: "test-model".into(),
            cost_per_1k: None,
            base_url: Some(base_url),
            api_key: Some("test-key".into()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: None,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: true,
        }
    }

    #[tokio::test]
    async fn served_response_carries_nonzero_latency() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = serde_json::json!({
            "model": "test-model",
            "choices": [{"message": {"role": "assistant", "content": "hi"}}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 1}
        });
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(body)
                    .set_delay(std::time::Duration::from_millis(120)),
            )
            .mount(&server)
            .await;

        let cfg = mock_config(format!("{}/chat/completions", server.uri()));
        let result = infer_with_retry(&ActivityOptions::new(), vec![], vec![cfg]).await;

        let (response, _cfg) = match result {
            ActivityResult::Ok(Ok(pair)) => pair,
            other => panic!("expected a served response, got {other:?}"),
        };
        assert!(
            response.latency_ms >= 100,
            "infer_with_retry must surface the real elapsed time, got {}ms",
            response.latency_ms
        );
    }
}
