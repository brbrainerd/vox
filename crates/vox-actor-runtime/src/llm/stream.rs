//! SSE streaming chat completions.

use std::future::Future;
use std::pin::Pin;

use futures_util::StreamExt;
use tokio_stream::Stream;

use super::types::{ChatMessage, LlmConfig};
use crate::{ActivityOptions, ActivityResult, execute_activity};

/// Token-by-token streaming implementation.
///
/// This is the raw, unwrapped path: no retry, no telemetry. Kept for callers that
/// already own their own retry/observability (e.g. `vox-gamify`'s cascade, which
/// retries across *providers* rather than within one). New callers that want the
/// same durability/observability properties `chat_once`/`llm_chat` has should use
/// [`llm_stream_activity`] instead.
pub async fn llm_stream(
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
    // Resolve once (single-source) and stream via the sanctioned egress core.
    let input = vox_config::resolve_egress::EgressResolveInput {
        provider: config.provider.clone(),
        model: config.model.clone(),
        base_url_override: config.base_url.clone(),
        // Resolved but ignored by stream_once (a whole-request deadline would sever SSE).
        timeout_ms: config.timeout_ms,
    };
    let ereq = vox_config::resolve_egress::resolve_egress(&input)?;
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
    let wire_tools: Option<Vec<vox_llm_egress::ToolDef>> = config.tools.as_ref().map(|ts| {
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

    // Streaming cost is gamify's concern (it has the cost_reporter); the facade records
    // cost via its non-streaming telemetry path, so it ignores the surfaced cost here.
    let (inner, _cost_usd) = vox_llm_egress::stream_once(&ereq, &wire_msgs, &params)
        .await
        .map_err(|e| e.to_string())?;
    // Map the core's structured error item type to the facade's String error.
    let mapped = inner.map(|item| item.map_err(|e| e.to_string()));
    Ok(Box::pin(mapped))
}

type LlmStreamActivityFuture = Pin<
    Box<
        dyn Future<
                Output = Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String>,
            > + Send,
    >,
>;

/// Durable wrapper around [`llm_stream`]: same `ActivityOptions`/`execute_activity`
/// retry+timeout envelope and per-attempt telemetry that [`super::chat::llm_chat`]
/// already has for the unary path.
///
/// Only *connection establishment* (the initial `stream_once` call, up to and
/// including the response headers) is retried — once the first byte of the SSE
/// body has started flowing, further attempts would duplicate output, so
/// mid-stream transport errors are surfaced to the caller as a terminal `Err`
/// item on the stream rather than triggering another attempt. This mirrors how
/// `chat_once`'s retry can only ever apply to a whole, not-yet-observed attempt.
pub async fn llm_stream_activity(
    options: &ActivityOptions,
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> ActivityResult<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>> {
    let activity_name = format!("llm_stream_{}_{}", config.provider, config.model);

    execute_activity(&activity_name, options, || {
        let messages = messages.clone();
        let config = config.clone();

        let fut = async move {
            let start = std::time::Instant::now();
            match llm_stream(messages, config.clone()).await {
                Ok(stream) => {
                    let latency = start.elapsed().as_millis() as i64;
                    let _ =
                        super::chat::record_telemetry_attempt(&config, "success", latency, None)
                            .await;
                    Ok(stream)
                }
                Err(err_msg) => {
                    let latency = start.elapsed().as_millis() as i64;
                    let _ = super::chat::record_telemetry_attempt(
                        &config,
                        "error",
                        latency,
                        Some("stream-connect-error"),
                    )
                    .await;
                    {
                        let trace_ctx = vox_telemetry::current_trace_ctx();
                        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
                            vox_telemetry::ErrorEvent {
                                subsystem: "llm.stream".into(),
                                error_class: "stream-connect-error".into(),
                                http_status: None,
                                retry_attempt: 0,
                                retried: false,
                                model: Some(config.model.clone()),
                                provider: None,
                                task_id: trace_ctx.task_id,
                                trace_id: Some(trace_ctx.trace_id.to_string()),
                            }
                        ));
                    }
                    // Returning Err here (rather than Ok(Err(..))) lets `execute_activity`
                    // retry the connection attempt, mirroring `llm_chat`'s retry envelope.
                    Err(err_msg)
                }
            }
        };
        let fut_typed: LlmStreamActivityFuture = Box::pin(fut);
        fut_typed
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActivityError;

    fn test_config(provider: &str, base_url: String) -> LlmConfig {
        LlmConfig {
            provider: provider.into(),
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

    /// RED test 1: llm_stream_activity retries a failed connection attempt the same
    /// way llm_chat retries a failed unary call — mirrors
    /// `execute_activity_succeeds_on_second_attempt_when_one_retry_allowed` in
    /// `activity.rs` and `llm_chat`'s retry-simulation pattern, but for the
    /// streaming connect path.
    #[tokio::test]
    async fn llm_stream_activity_retries_failed_connection_then_succeeds() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // First attempt: 500 (connection/handshake failure). Second attempt: SSE success.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n";
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse),
            )
            .mount(&server)
            .await;

        let config = test_config("openrouter", format!("{}/chat/completions", server.uri()));
        let options = ActivityOptions::new().with_retries(1);
        let result = llm_stream_activity(&options, vec![], config).await;

        match result {
            ActivityResult::Ok(mut stream) => {
                let mut got = String::new();
                while let Some(item) = stream.next().await {
                    got.push_str(&item.expect("chunk"));
                }
                assert_eq!(got, "hi", "must recover on the retried attempt");
            }
            ActivityResult::Failed(e) => {
                panic!("expected Ok(stream) after retry, got Failed({e:?})")
            }
            ActivityResult::Cancelled => panic!("expected Ok(stream) after retry, got Cancelled"),
        }
    }

    /// RED test 1b: with zero retries configured, a connection failure surfaces as
    /// `ActivityResult::Failed` after exactly one attempt — mirroring `llm_chat`'s
    /// exhausted-retries behavior for the unary path.
    #[tokio::test]
    async fn llm_stream_activity_exhausts_retries_and_reports_failure() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let config = test_config("openrouter", format!("{}/chat/completions", server.uri()));
        let options = ActivityOptions::new(); // no retries
        let result = llm_stream_activity(&options, vec![], config).await;

        match result {
            ActivityResult::Failed(ActivityError::RetriesExhausted { attempts, .. }) => {
                assert_eq!(attempts, 1);
            }
            ActivityResult::Ok(_) => panic!("expected Failed(RetriesExhausted), got Ok"),
            ActivityResult::Failed(e) => panic!("expected RetriesExhausted, got Failed({e:?})"),
            ActivityResult::Cancelled => panic!("expected Failed(RetriesExhausted), got Cancelled"),
        }
    }
}
