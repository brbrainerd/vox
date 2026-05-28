use super::metadata::HttpCallMetadata;
use crate::llm_bridge::error::HttpInferError;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "str::is_empty")]
    system: &'a str,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

pub(crate) async fn http_anthropic_direct(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    spec: &vox_orchestrator::models::ModelSpec,
    model: &str,
    system: &str,
    user: vox_openai::ChatMessageContent<'_>,
    max_tokens: u64,
    temperature: Option<f32>,
    top_p: Option<f32>,
) -> Result<(String, u32, u32, HttpCallMetadata), HttpInferError> {
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

    let body = AnthropicRequest {
        model,
        max_tokens: max_tokens.max(1024), // Anthropic requires max_tokens > 0
        temperature,
        top_p,
        system,
        messages: vec![AnthropicMessage {
            role: "user",
            content: user_text,
        }],
    };

    let res = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| HttpInferError {
            status: 0,
            message: format!("LLM HTTP: {e}"),
            is_capability_gap: false,
        })?;

    let status = res.status();
    let code = status.as_u16();

    let provider_request_id = res
        .headers()
        .get("request-id")
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

    let parsed: AnthropicResponse = res.json().await.map_err(|e| HttpInferError {
        status: code,
        message: format!("LLM JSON: {e}"),
        is_capability_gap: false,
    })?;

    let mut text = String::new();
    for block in parsed.content {
        if block.block_type == "text" {
            if let Some(t) = block.text {
                text.push_str(&t);
            }
        }
    }

    let input_tokens = parsed.usage.input_tokens;
    let output_tokens = parsed.usage.output_tokens;
    let cache_read_input_tokens = parsed.usage.cache_read_input_tokens.filter(|&t| t > 0);
    let cache_creation_input_tokens = parsed.usage.cache_creation_input_tokens.filter(|&t| t > 0);

    // Anthropic bills cache reads at cache_read_cost_per_1k and newly-created cache entries at
    // cache_creation_cost_per_1k. Account for these in the estimated cost when reported.
    let non_cached = input_tokens - cache_read_input_tokens.unwrap_or(0).min(input_tokens);
    let cache_read_cost =
        cache_read_input_tokens.unwrap_or(0) as f64 / 1000.0 * spec.cache_read_cost_per_1k;
    let cache_create_cost =
        cache_creation_input_tokens.unwrap_or(0) as f64 / 1000.0 * spec.cache_creation_cost_per_1k;
    let estimated_usd = (non_cached as f64 / 1000.0) * spec.cost_per_1k_input
        + (output_tokens as f64 / 1000.0) * spec.cost_per_1k_output
        + cache_read_cost
        + cache_create_cost;

    Ok((
        text,
        input_tokens,
        output_tokens,
        HttpCallMetadata {
            provider_request_id: provider_request_id.or(Some(parsed.id)),
            provider_reported_cost_usd: Some(estimated_usd),
            cache_read_input_tokens,
            cache_creation_input_tokens,
        },
    ))
}
