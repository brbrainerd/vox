use std::collections::HashMap;

use super::metadata::HttpCallMetadata;
use crate::llm_bridge::error::HttpInferError;
use vox_openai::{
    ChatCompletionRequest as OpenAiChatRequest, ChatCompletionResponse as OpenAiChatResponse,
    ChatMessageTurn as OpenAiMsg,
};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn http_openai_compatible_with_headers(
    client: &reqwest::Client,
    url: &str,
    bearer: &str,
    model: &str,
    system: &str,
    user: vox_openai::ChatMessageContent<'_>,
    max_tokens: u64,
    temperature: Option<f32>,
    top_p: Option<f32>,
    json_mode: bool,
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
    extra_headers: &HashMap<String, String>,
) -> Result<(String, u32, u32, HttpCallMetadata), HttpInferError> {
    let mut messages = Vec::new();
    if !system.is_empty() {
        messages.push(OpenAiMsg {
            role: "system",
            content: vox_openai::ChatMessageContent::Text(system),
        });
    }
    messages.push(OpenAiMsg {
        role: "user",
        content: user,
    });

    let response_format = if json_mode {
        Some(serde_json::json!({ "type": "json_object" }))
    } else {
        None
    };

    let requested_tools = tools.is_some();
    let body = OpenAiChatRequest {
        model,
        messages,
        temperature,
        max_tokens,
        stream: false,
        top_p,
        response_format,
        tools,
        tool_choice,
    };

    let mut req = client.post(url).json(&body);
    if !bearer.is_empty() {
        req = req.bearer_auth(bearer);
    }
    for (k, v) in extra_headers {
        req = req.header(k, v);
    }

    let res = req.send().await.map_err(|e| HttpInferError {
        status: 0,
        message: format!("LLM HTTP: {e}"),
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

    let parsed: OpenAiChatResponse = res.json().await.map_err(|e| HttpInferError {
        status: code,
        message: format!("LLM JSON: {e}"),
        is_capability_gap: false,
    })?;

    let message = parsed.choices.into_iter().next().and_then(|c| c.message);

    fn coerce_json_fallback(mut s: &str) -> String {
        s = s.trim();
        if s.starts_with("```json") {
            s = s[7..].trim_start();
        } else if s.starts_with("```") {
            s = s[3..].trim_start();
        }
        if s.ends_with("```") {
            s = s[..s.len() - 3].trim_end();
        }
        s.to_string()
    }

    let text = if let Some(m) = message {
        if let Some(mut tc) = m.tool_calls {
            if let Some(first) = tc.pop() {
                first.function.arguments
            } else {
                let s = m.content.unwrap_or_default();
                if requested_tools || json_mode {
                    coerce_json_fallback(&s)
                } else {
                    s
                }
            }
        } else {
            let s = m.content.unwrap_or_default();
            if requested_tools || json_mode {
                coerce_json_fallback(&s)
            } else {
                s
            }
        }
    } else {
        String::new()
    };

    let u = parsed.usage.unwrap_or_default();
    let provider_reported_cost_usd = u.total_cost.or(u.cost);
    // Collect cache-hit tokens: prefer Anthropic-style `cache_read_input_tokens`, then
    // OpenAI/DeepSeek-style `prompt_tokens_details.cached_tokens`. A value of 0 is treated as None.
    let cache_read_input_tokens: Option<u32> =
        u.cache_read_input_tokens.filter(|&t| t > 0).or_else(|| {
            u.prompt_tokens_details
                .as_ref()
                .map(|d| d.cached_tokens)
                .filter(|&t| t > 0)
        });
    Ok((
        text,
        u.prompt_tokens,
        u.completion_tokens,
        HttpCallMetadata {
            provider_request_id: provider_request_id.or(parsed.id),
            provider_reported_cost_usd,
            cache_read_input_tokens,
            cache_creation_input_tokens: None, // OpenAI doesn't report cache creation separately
        },
    ))
}
