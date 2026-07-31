//! The provider wire: OpenAI-compatible chat / streaming / embeddings egress.
//! Ported from `vox-actor-runtime/src/llm/{chat,stream,embed}.rs`, reading from a
//! resolved [`EgressRequest`] instead of `LlmConfig` and returning structured results.

use std::time::Instant;

use serde::Serialize;

use crate::{
    ChatMessage, ChatParams, ChatStream, EgressChatResponse, EgressError, EgressRequest,
    EgressToolCall, throttle,
};

#[derive(Serialize)]
struct OpenAiToolFunction<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    parameters: &'a serde_json::Value,
}

#[derive(Serialize)]
struct OpenAiTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiToolFunction<'a>,
}

/// Outbound shape for `function.arguments` — a JSON-encoded **string**, the inverse of
/// `EgressToolCall::arguments`'s eagerly-parsed `serde_json::Value` (see 1.3a). We
/// re-serialize here rather than changing `EgressToolCall`, which stays the shared
/// inbound/outbound type per the task's instruction to reuse it.
#[derive(Serialize)]
struct WireToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireToolCallFunction,
}

impl From<&EgressToolCall> for WireToolCall {
    fn from(tc: &EgressToolCall) -> Self {
        WireToolCall {
            id: tc.id.clone(),
            kind: "function",
            function: WireToolCallFunction {
                name: tc.name.clone(),
                // Best-effort: if `arguments` somehow fails to re-serialize (it came
                // from `serde_json::Value`, so in practice this never fails), fall
                // back to an empty JSON object string rather than panicking.
                arguments: serde_json::to_string(&tc.arguments)
                    .unwrap_or_else(|_| "{}".to_string()),
            },
        }
    }
}

/// Per-message wire shape mirroring `ChatMessage` but with `tool_calls` re-serialized
/// to the outbound `WireToolCall` shape (JSON-string arguments) instead of the parsed
/// `Value` shape `ChatMessage`/`EgressToolCall` carry for caller convenience.
#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

impl<'a> From<&'a ChatMessage> for WireMessage<'a> {
    fn from(m: &'a ChatMessage) -> Self {
        WireMessage {
            role: &m.role,
            content: &m.content,
            tool_calls: m
                .tool_calls
                .as_ref()
                .map(|calls| calls.iter().map(WireToolCall::from).collect()),
            tool_call_id: m.tool_call_id.as_deref(),
            name: m.name.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a serde_json::Value>,
    stream: bool,
}

fn build_request<'a>(
    req: &'a EgressRequest,
    params: &'a ChatParams<'a>,
    messages: &'a [ChatMessage],
    stream: bool,
) -> OpenAiChatRequest<'a> {
    let tools = params.tools.map(|ts| {
        ts.iter()
            .map(|t| OpenAiTool {
                kind: "function",
                function: OpenAiToolFunction {
                    name: &t.name,
                    description: t.description.as_deref(),
                    parameters: &t.parameters,
                },
            })
            .collect()
    });
    OpenAiChatRequest {
        model: &req.model,
        messages: messages.iter().map(WireMessage::from).collect(),
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        response_format: params.response_format,
        tools,
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
    let mut http = apply_auth_headers(client.post(&req.base_url).json(&body), req);
    if let Some(ms) = req.timeout_ms {
        http = http.timeout(std::time::Duration::from_millis(ms));
    }

    let start = Instant::now();
    let res = http
        .send()
        .await
        .map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status {
            code: status.as_u16(),
            body,
        });
    }
    let header_cost = res
        .headers()
        .get("x-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok());
    let latency_ms = start.elapsed().as_millis() as u64;
    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| EgressError::Decode(e.to_string()))?;
    throttle::on_success(&req.throttle_key);

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let usage = &json["usage"];
    let prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
    let completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0) as u32;
    // Cached tokens: prefer `cache_read_input_tokens`, else `prompt_tokens_details.cached_tokens`.
    let cache_read_tokens = usage["cache_read_input_tokens"]
        .as_u64()
        .or_else(|| usage["prompt_tokens_details"]["cached_tokens"].as_u64())
        .unwrap_or(0) as u32;
    // Provider-reported cost from the body (`total_cost`/`cost`), else the header.
    let cost_usd = usage["total_cost"]
        .as_f64()
        .or_else(|| usage["cost"].as_f64())
        .or(header_cost);
    let model = json["model"].as_str().unwrap_or(&req.model).to_string();
    let tool_calls = parse_tool_calls(&json["choices"][0]["message"]["tool_calls"]);
    Ok(EgressChatResponse {
        content,
        prompt_tokens,
        completion_tokens,
        cache_read_tokens,
        model,
        cost_usd,
        latency_ms,
        tool_calls,
    })
}

/// Parse `message.tool_calls` (an array or absent/null) into `EgressToolCall`s.
/// Returns `None` when the field is absent/null/not-an-array (the common case for
/// callers that pass no tools), so this never changes behavior for existing callers.
fn parse_tool_calls(value: &serde_json::Value) -> Option<Vec<EgressToolCall>> {
    let arr = value.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let calls: Vec<EgressToolCall> = arr
        .iter()
        .filter_map(|tc| {
            // Provider omitted `id`: default to empty string rather than dropping the
            // call — correlating (or rejecting) a call with no id is left to the
            // tool-dispatch loop (a separate task), not this pure wire-parsing layer.
            let id = tc["id"].as_str().unwrap_or_default().to_string();
            // Entries with no `function.name` are dropped via `?` (intentional): a
            // tool call this crate can't even name isn't actionable by any caller, so
            // we silently skip it rather than surfacing a partially-populated/garbage
            // call — see `tool_calls_entry_missing_name_is_dropped` for the locked-in
            // behavior.
            let name = tc["function"]["name"].as_str()?.to_string();
            let arguments = tc["function"]["arguments"]
                .as_str()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            Some(EgressToolCall {
                id,
                name,
                arguments,
            })
        })
        .collect();
    // If every entry was dropped (e.g. all missing `function.name`), report `None`
    // rather than `Some(vec![])` — an empty vec would misleadingly read as "tools were
    // requested but the model called none", when really nothing usable was parsed.
    if calls.is_empty() { None } else { Some(calls) }
}

/// Streaming OpenAI-compatible chat completion. Yields content deltas. Ported from
/// `vox-actor-runtime/src/llm/stream.rs` (same `vox_openai::sse` line framing).
pub async fn stream_once(
    req: &EgressRequest,
    messages: &[ChatMessage],
    params: &ChatParams<'_>,
) -> Result<(ChatStream, Option<f64>), EgressError> {
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

    let res = http
        .send()
        .await
        .map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status {
            code: status.as_u16(),
            body,
        });
    }
    throttle::on_success(&req.throttle_key);

    // Provider-reported cost from the response header (available before the body streams).
    let cost_usd = res
        .headers()
        .get("x-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok());

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
    Ok((Box::pin(out), cost_usd))
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

    let body = EmbedRequest {
        model: &req.model,
        input: text,
    };
    let mut http = apply_auth_headers(client.post(&req.base_url).json(&body), req);
    if let Some(ms) = req.timeout_ms {
        http = http.timeout(std::time::Duration::from_millis(ms));
    }
    let res = http
        .send()
        .await
        .map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status {
            code: status.as_u16(),
            body,
        });
    }
    throttle::on_success(&req.throttle_key);
    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| EgressError::Decode(e.to_string()))?;
    let vector = json["data"][0]["embedding"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect()
        })
        .unwrap_or_default();
    Ok(vector)
}
