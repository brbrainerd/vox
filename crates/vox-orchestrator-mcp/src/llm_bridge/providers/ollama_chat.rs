use super::metadata::{HttpCallMetadata, ollama_base_url};
use super::types::{
    OllamaChatMsg, OllamaChatRequest, OllamaChatResponse, OllamaMsg, OllamaOptions,
};
use crate::llm_bridge::error::HttpInferError;

pub(crate) async fn http_ollama_with_metadata(
    client: &reqwest::Client,
    model: &str,
    system: &str,
    user: vox_openai::ChatMessageContent<'_>,
    max_tokens: u64,
    temperature: Option<f32>,
    top_p: Option<f32>,
    json_mode: bool,
) -> Result<(String, u32, u32, HttpCallMetadata), HttpInferError> {
    let base = ollama_base_url();
    let url = format!("{}/api/chat", base.trim_end_matches('/'));

    let mut messages = Vec::new();
    let user_text = match user {
        vox_openai::ChatMessageContent::Text(t) => t,
        vox_openai::ChatMessageContent::Parts(ref p) => p
            .iter()
            .find_map(|part| match part {
                vox_openai::ChatMessagePart::Text { text } => Some(*text),
                _ => None,
            })
            .unwrap_or(""),
    };

    if !system.is_empty() {
        messages.push(OllamaChatMsg {
            role: "system",
            content: system,
        });
    }
    messages.push(OllamaChatMsg {
        role: "user",
        content: user_text,
    });

    let format = if json_mode {
        Some(serde_json::json!("json"))
    } else {
        None
    };

    let body = OllamaChatRequest {
        model,
        messages,
        stream: false,
        format,
        options: OllamaOptions {
            temperature,
            top_p,
            num_predict: max_tokens.min(i32::MAX as u64) as i32,
        },
    };

    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| HttpInferError {
            status: 0,
            message: format!("Ollama HTTP: {e}"),
            is_capability_gap: false,
        })?;
    let status = res.status();
    let code = status.as_u16();
    let provider_request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);

    if !status.is_success() {
        let t = res.text().await.unwrap_or_default();
        return Err(HttpInferError {
            status: code,
            message: t,
            is_capability_gap: false,
        });
    }

    let parsed: OllamaChatResponse = res.json().await.map_err(|e| HttpInferError {
        status: code,
        message: format!("Ollama JSON: {e}"),
        is_capability_gap: false,
    })?;

    let text = parsed
        .message
        .and_then(|m: OllamaMsg| m.content)
        .unwrap_or_default();
    Ok((
        text,
        parsed.prompt_eval_count,
        parsed.eval_count,
        HttpCallMetadata {
            provider_request_id,
            provider_reported_cost_usd: None,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        },
    ))
}
