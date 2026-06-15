//! The provider wire: OpenAI-compatible chat / streaming / embeddings egress.
//! Ported from `vox-actor-runtime/src/llm/{chat,stream,embed}.rs`, reading from a
//! resolved [`EgressRequest`] instead of `LlmConfig` and returning structured results.

use std::time::Instant;

use serde::Serialize;

use crate::{
    throttle, ChatMessage, ChatParams, ChatStream, EgressChatResponse, EgressError, EgressRequest,
};

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a serde_json::Value>,
    stream: bool,
}

fn build_request<'a>(req: &'a EgressRequest, params: &'a ChatParams<'a>, messages: &'a [ChatMessage], stream: bool) -> OpenAiChatRequest<'a> {
    OpenAiChatRequest {
        model: &req.model,
        messages,
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        response_format: params.response_format,
        tool_choice: params.tool_choice,
        stream,
    }
}

fn apply_auth_headers(
    mut http: reqwest::RequestBuilder,
    req: &EgressRequest,
) -> reqwest::RequestBuilder {
    if !req.api_key.is_empty() {
        http = http.bearer_auth(&req.api_key);
    }
    for (name, value) in &req.headers {
        http = http.header(name, value);
    }
    http
}

/// Non-streaming OpenAI-compatible chat completion.
pub async fn chat_once(
    req: &EgressRequest,
    messages: &[ChatMessage],
    params: &ChatParams<'_>,
) -> Result<EgressChatResponse, EgressError> {
    let client = vox_http_client::client();
    let _permit = throttle::acquire_permit(&req.throttle_key, req.max_concurrent).await;

    let body = build_request(req, params, messages, false);
    let http = apply_auth_headers(client.post(&req.base_url).json(&body), req);

    let start = Instant::now();
    let res = http.send().await.map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status { code: status.as_u16(), body });
    }
    let cost_usd = res
        .headers()
        .get("x-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok());
    let latency_ms = start.elapsed().as_millis() as u64;
    let json: serde_json::Value =
        res.json().await.map_err(|e| EgressError::Decode(e.to_string()))?;
    throttle::on_success(&req.throttle_key);

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let prompt_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
    let completion_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
    let model = json["model"].as_str().unwrap_or(&req.model).to_string();
    Ok(EgressChatResponse { content, prompt_tokens, completion_tokens, model, cost_usd, latency_ms })
}

/// Streaming OpenAI-compatible chat completion. Yields content deltas. Ported from
/// `vox-actor-runtime/src/llm/stream.rs` (same `vox_openai::sse` line framing).
pub async fn stream_once(
    req: &EgressRequest,
    messages: &[ChatMessage],
    params: &ChatParams<'_>,
) -> Result<ChatStream, EgressError> {
    use async_stream::stream;
    use futures_util::StreamExt;

    let client = vox_http_client::client();
    // Hold a permit across the whole stream lifetime by acquiring before the request;
    // the permit drops when the returned stream is dropped (it is moved into the closure).
    let permit = throttle::acquire_permit(&req.throttle_key, req.max_concurrent).await;

    let body = build_request(req, params, messages, true);
    let body_str = serde_json::to_string(&body).map_err(|e| EgressError::Decode(e.to_string()))?;
    let mut http = client
        .post(&req.base_url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .body(body_str);
    if !req.api_key.is_empty() {
        http = http.bearer_auth(&req.api_key);
    }
    for (name, value) in &req.headers {
        http = http.header(name, value);
    }

    let res = http.send().await.map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status { code: status.as_u16(), body });
    }
    throttle::on_success(&req.throttle_key);

    let byte_stream = res.bytes_stream();
    let out = stream! {
        // Keep the throttle permit alive for the duration of the stream.
        let _permit = permit;
        use vox_openai::sse::{sse_data_line_delta, Utf8LineBuffer};
        let mut buf = Utf8LineBuffer::new();
        futures_util::pin_mut!(byte_stream);
        while let Some(chunk_res) = byte_stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    let mut emitted: Vec<String> = Vec::new();
                    buf.push_lossy_bytes(&bytes, |line| {
                        if let Some(s) = sse_data_line_delta(line) {
                            emitted.push(s);
                        }
                    });
                    for s in emitted {
                        yield Ok(s);
                    }
                }
                Err(e) => {
                    yield Err(EgressError::Http(e.to_string()));
                    return;
                }
            }
        }
        let mut tail: Vec<String> = Vec::new();
        buf.flush_trailing(|line| {
            if let Some(s) = sse_data_line_delta(line) {
                tail.push(s);
            }
        });
        for s in tail {
            yield Ok(s);
        }
    };
    Ok(Box::pin(out))
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
}

/// Embeddings POST. `req.base_url` must be the embeddings endpoint (caller-resolved).
/// Ported from `vox-actor-runtime/src/llm/embed.rs`.
pub async fn embed_once(req: &EgressRequest, text: &str) -> Result<Vec<f32>, EgressError> {
    let client = vox_http_client::client();
    let _permit = throttle::acquire_permit(&req.throttle_key, req.max_concurrent).await;

    let body = EmbedRequest { model: &req.model, input: text };
    let http = apply_auth_headers(client.post(&req.base_url).json(&body), req);
    let res = http.send().await.map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status { code: status.as_u16(), body });
    }
    throttle::on_success(&req.throttle_key);
    let json: serde_json::Value =
        res.json().await.map_err(|e| EgressError::Decode(e.to_string()))?;
    let vector = json["data"][0]["embedding"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
        .unwrap_or_default();
    Ok(vector)
}
