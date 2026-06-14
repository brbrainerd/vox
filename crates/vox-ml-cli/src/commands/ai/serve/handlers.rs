//! Axum HTTP handlers for the inference API.

#[cfg(feature = "execution-api")]
use super::prompt::{prompt_for_output_mode, validate_structured_output_with_reason};
#[cfg(feature = "execution-api")]
use super::schema::{Choice, GenerateRequest, GenerateResponse};
#[cfg(feature = "execution-api")]
use super::worker::InferenceRequest;
#[cfg(feature = "execution-api")]
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
};
#[cfg(feature = "execution-api")]
use std::convert::Infallible;
#[cfg(feature = "execution-api")]
use std::sync::Arc;
#[cfg(feature = "execution-api")]
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
#[cfg(feature = "execution-api")]
use vox_corpus::corpus::structured_eval::StructuredFailReason;

#[cfg(feature = "execution-api")]
fn parse_output_mode_label(raw: &str) -> Option<&'static str> {
    match raw.trim().to_lowercase().as_str() {
        "strict_json" | "strict-json" => Some("strict_json"),
        "jsonl_records" | "jsonl-records" => Some("jsonl_records"),
        "tool_args_json" | "tool-args-json" => Some("tool_args_json"),
        _ => None,
    }
}

#[cfg(feature = "execution-api")]
#[derive(Clone)]
pub struct AppState {
    pub tx: std::sync::mpsc::SyncSender<InferenceRequest>,
    pub model_name: Arc<str>,
}

#[cfg(feature = "execution-api")]
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "ok", "service": "vox-ml-cli"})),
    )
}

/// Readiness probe — server is up and accepting requests.
/// Model loading happens in worker thread; first inference may block until ready.
#[cfg(feature = "execution-api")]
pub async fn ready() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({"ready": true, "service": "vox-ml-cli"})),
    )
}

#[cfg(feature = "execution-api")]
pub async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [{
            "id": &*state.model_name,
            "object": "model",
            "owned_by": "vox-ml-cli"
        }]
    }))
}

#[cfg(feature = "execution-api")]
pub async fn do_generate(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> (StatusCode, Json<GenerateResponse>) {
    if let Some(ref requested) = req.model {
        if requested.as_str() != state.model_name.as_ref() {
            tracing::debug!(
                requested = %requested,
                actual = %*state.model_name,
                "Client requested different model; serving loaded model"
            );
        }
    }
    let output_mode = req.output_mode.as_deref().and_then(parse_output_mode_label);
    let max_retries = req.max_retries.max(1);
    let schema = req.schema.as_ref();

    let mut attempt = 0u32;
    #[allow(unused_assignments)]
    let mut last_text = String::new();
    #[allow(unused_assignments)]
    let mut last_error: Option<String> = None;
    let mut total_tokens = 0usize;

    loop {
        let prompt = if attempt == 0 {
            prompt_for_output_mode(&req.prompt, output_mode)
        } else {
            let err_hint = last_error.as_deref().unwrap_or("validation failed");
            let preview = last_text.chars().take(200).collect::<String>();
            format!(
                "Fix the JSON. Error: {}. Invalid output: {}\n\nOutput valid JSON only:\n\n{}",
                err_hint,
                if preview.len() < last_text.len() {
                    format!("{}...", preview)
                } else {
                    preview
                },
                prompt_for_output_mode(&req.prompt, output_mode)
            )
        };

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let ir = InferenceRequest {
            prompt,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            top_k: 40,
            output_mode: output_mode.map(String::from),
            reply: reply_tx,
            stream_tx: None,
        };
        let tx = state.tx.clone();
        let send_ok = tokio::task::spawn_blocking(move || tx.send(ir))
            .await
            .map(|r: Result<(), std::sync::mpsc::SendError<InferenceRequest>>| r.is_ok())
            .unwrap_or(false);
        if !send_ok {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(GenerateResponse {
                    text: "Inference worker unavailable".into(),
                    code: "Inference worker unavailable".into(),
                    tokens_generated: 0,
                    model: state.model_name.to_string(),
                    object: "text_completion",
                    choices: vec![],
                    repair_attempts: None,
                    valid: false,
                    errors: vec!["Inference worker unavailable".into()],
                }),
            );
        }
        let text = reply_rx
            .await
            .unwrap_or_else(|_| Err("Worker dropped".into()))
            .unwrap_or_else(|e| format!("[error: {e}]"));
        let tokens = text.split_whitespace().count();
        total_tokens += tokens;
        last_text = text.clone();

        let validation: Option<StructuredFailReason> = if output_mode.is_some() {
            validate_structured_output_with_reason(&text, output_mode, schema).err()
        } else {
            None
        };
        let valid = validation.is_none();
        if let Some(ref e) = validation {
            last_error = Some(e.to_string());
        }
        if output_mode.is_none() || valid || attempt >= max_retries - 1 {
            let repair_attempts = if output_mode.is_some() && attempt > 0 {
                Some(attempt)
            } else {
                None
            };
            let err_list: Vec<String> = last_error.take().into_iter().collect();
            let resp = GenerateResponse {
                code: last_text.clone(),
                text: last_text.clone(),
                tokens_generated: total_tokens,
                model: state.model_name.to_string(),
                object: "text_completion",
                choices: vec![Choice {
                    text: last_text,
                    index: 0,
                    finish_reason: "stop",
                }],
                repair_attempts,
                valid,
                errors: err_list,
            };
            return (StatusCode::OK, Json(resp));
        }
        attempt += 1;
    }
}

/// SSE Streaming variant of the completions endpoint.
#[cfg(feature = "execution-api")]
pub async fn do_completions_stream(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let output_mode = req.output_mode.as_deref().and_then(parse_output_mode_label);

    let (stream_tx, stream_rx) = tokio::sync::mpsc::channel(32);
    let (reply_tx, _) = tokio::sync::oneshot::channel();
    let prompt = prompt_for_output_mode(&req.prompt, output_mode);

    let ir = InferenceRequest {
        prompt,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_k: 40,
        output_mode: output_mode.map(String::from),
        reply: reply_tx,
        stream_tx: Some(stream_tx),
    };

    let tx = state.tx.clone();
    let model_name = state.model_name.to_string();

    tokio::task::spawn_blocking(move || {
        let _ = tx.send(ir);
    });

    let stream = ReceiverStream::new(stream_rx).map(move |chunk_result| match chunk_result {
        Ok(chunk) => {
            let json = serde_json::json!({
                "id": "compl-stream",
                "object": "text_completion",
                "model": model_name,
                "choices": [{
                    "text": chunk,
                    "index": 0,
                    "finish_reason": null
                }]
            });
            Ok(Event::default()
                .data(serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string())))
        }
        Err(e) => Ok(Event::default().data(format!("{{\"error\": \"{}\"}}", e))),
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

#[cfg(feature = "execution-api")]
#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn parse_output_mode_label_strict_json_underscore() {
        assert_eq!(parse_output_mode_label("strict_json"), Some("strict_json"));
    }

    #[test]
    fn parse_output_mode_label_strict_json_hyphen() {
        assert_eq!(parse_output_mode_label("strict-json"), Some("strict_json"));
    }

    #[test]
    fn parse_output_mode_label_jsonl_records_underscore() {
        assert_eq!(
            parse_output_mode_label("jsonl_records"),
            Some("jsonl_records")
        );
    }

    #[test]
    fn parse_output_mode_label_jsonl_records_hyphen() {
        assert_eq!(
            parse_output_mode_label("jsonl-records"),
            Some("jsonl_records")
        );
    }

    #[test]
    fn parse_output_mode_label_tool_args_json_underscore() {
        assert_eq!(
            parse_output_mode_label("tool_args_json"),
            Some("tool_args_json")
        );
    }

    #[test]
    fn parse_output_mode_label_tool_args_json_hyphen() {
        assert_eq!(
            parse_output_mode_label("tool-args-json"),
            Some("tool_args_json")
        );
    }

    #[test]
    fn parse_output_mode_label_uppercase_normalizes() {
        assert_eq!(parse_output_mode_label("STRICT_JSON"), Some("strict_json"));
        assert_eq!(parse_output_mode_label("Strict-Json"), Some("strict_json"));
    }

    #[test]
    fn parse_output_mode_label_trims_whitespace() {
        assert_eq!(
            parse_output_mode_label("  strict_json  "),
            Some("strict_json")
        );
    }

    #[test]
    fn parse_output_mode_label_unknown_returns_none() {
        assert_eq!(parse_output_mode_label("unknown"), None);
        assert_eq!(parse_output_mode_label(""), None);
        assert_eq!(parse_output_mode_label("json"), None);
    }
}
