use futures::StreamExt;
use vox_llm_egress::{
    ChatMessage, ChatParams, EgressError, EgressRequest, EgressToolCall, ToolDef, chat_once,
    embed_once, stream_once,
};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(base: String) -> EgressRequest {
    EgressRequest {
        base_url: base,
        api_key: "secret".into(),
        model: "test/model".into(),
        headers: vec![("X-Title".into(), "vox".into())],
        throttle_key: "test-or".into(),
        max_concurrent: 4,
        timeout_ms: Some(30_000),
    }
}

#[tokio::test]
async fn chat_once_sends_bearer_headers_and_parses_usage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            vox_http_client::bearer_auth_header_string("secret"),
        ))
        .and(header("x-title", "vox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "test/model",
            "choices": [{"message": {"role": "assistant", "content": "hi"}}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 2}
        })))
        .mount(&server)
        .await;

    let r = req(format!("{}/chat/completions", server.uri()));
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: "yo".into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    let out = chat_once(&r, &msgs, &ChatParams::default())
        .await
        .expect("ok");
    assert_eq!(out.content, "hi");
    assert_eq!(out.prompt_tokens, 5);
    assert_eq!(out.completion_tokens, 2);
    assert_eq!(out.model, "test/model");
    assert_eq!(
        out.tool_calls, None,
        "response with no tool_calls field must parse as None (no regression for \
         existing non-tool callers like ghost_text/inline_edit/plan)"
    );
}

#[tokio::test]
async fn chat_once_parses_tool_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "test/model",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\":\"Paris\"}"
                        }
                    }]
                }
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 2}
        })))
        .mount(&server)
        .await;

    let r = req(format!("{}/chat/completions", server.uri()));
    let out = chat_once(&r, &[], &ChatParams::default())
        .await
        .expect("ok");
    let calls = out.tool_calls.expect("tool_calls must be populated");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_abc123");
    assert_eq!(calls[0].name, "get_weather");
    assert_eq!(calls[0].arguments, serde_json::json!({"city": "Paris"}));
}

#[tokio::test]
async fn tool_calls_entry_missing_name_is_dropped() {
    // Locks in the intentional behavior documented at the `?` in `parse_tool_calls`:
    // an entry with no `function.name` is unnamed/unactionable and is silently
    // dropped rather than surfaced as a partial/garbage call. Here it's the only
    // entry, so `tool_calls` ends up `None`; a well-formed sibling entry would
    // still be kept (this is a per-entry filter, not an all-or-nothing one).
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "test/model",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_no_name",
                        "type": "function",
                        "function": {
                            "arguments": "{}"
                        }
                    }]
                }
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1}
        })))
        .mount(&server)
        .await;

    let r = req(format!("{}/chat/completions", server.uri()));
    let out = chat_once(&r, &[], &ChatParams::default())
        .await
        .expect("ok");
    assert_eq!(
        out.tool_calls, None,
        "entry with no function.name must be dropped, leaving no tool_calls"
    );
}

#[tokio::test]
async fn chat_once_parses_cache_tokens_and_body_cost() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "m",
            "choices": [{"message": {"content": "ok"}}],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 4,
                "cache_read_input_tokens": 6,
                "total_cost": 0.0021
            }
        })))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let out = chat_once(&r, &[], &ChatParams::default())
        .await
        .expect("ok");
    assert_eq!(out.cache_read_tokens, 6);
    assert_eq!(out.cost_usd, Some(0.0021));
}

#[tokio::test]
async fn chat_once_serializes_tools() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(serde_json::json!({
            "tools": [{"type": "function", "function": {"name": "get_weather"}}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{"message": {"content": "ok"}}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1}
        })))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let tools = vec![ToolDef {
        name: "get_weather".into(),
        description: None,
        parameters: serde_json::json!({"type": "object"}),
    }];
    let params = ChatParams {
        tools: Some(&tools),
        ..Default::default()
    };
    let out = chat_once(&r, &[], &params)
        .await
        .expect("tools request must match + succeed");
    assert_eq!(out.content, "ok");
}

/// Task 1.3b: a full assistant-tool_calls + tool-result turn must reach the wire with
/// `function.arguments` re-serialized as a JSON **string** (the inverse of
/// `EgressToolCall::arguments`'s eagerly-parsed `Value`), and the tool-result message's
/// `tool_call_id` correlating back to the call.
#[tokio::test]
async fn chat_once_serializes_assistant_tool_calls_and_tool_result_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(serde_json::json!({
            "messages": [
                {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}
                    }]
                },
                {
                    "role": "tool",
                    "content": "72F and sunny",
                    "tool_call_id": "call_1",
                    "name": "get_weather"
                }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{"message": {"content": "It's 72F and sunny in Paris."}}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1}
        })))
        .mount(&server)
        .await;

    let r = req(format!("{}/chat/completions", server.uri()));
    let msgs = vec![
        ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(vec![EgressToolCall {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: serde_json::json!({"city": "Paris"}),
            }]),
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "tool".into(),
            content: "72F and sunny".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            name: Some("get_weather".into()),
        },
    ];
    let out = chat_once(&r, &msgs, &ChatParams::default())
        .await
        .expect("request must match the mock's exact tool-call wire shape");
    assert_eq!(out.content, "It's 72F and sunny in Paris.");
}

#[tokio::test]
async fn chat_once_maps_429_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let err = chat_once(&r, &[], &ChatParams::default())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            EgressError::RateLimited {
                retry_after: Some(_)
            }
        ),
        "expected RateLimited with retry_after, got {err:?}"
    );
}

#[tokio::test]
async fn chat_once_maps_non_2xx_to_status_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let err = chat_once(&r, &[], &ChatParams::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, EgressError::Status { code: 500, .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn stream_once_assembles_sse_deltas() {
    let server = MockServer::start().await;
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n\
               data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let (mut s, _cost) = stream_once(&r, &[], &ChatParams::default())
        .await
        .expect("stream");
    let mut got = String::new();
    while let Some(chunk) = s.next().await {
        got.push_str(&chunk.expect("chunk"));
    }
    assert_eq!(got, "ab");
}

#[tokio::test]
async fn stream_once_surfaces_response_cost_header() {
    let server = MockServer::start().await;
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: [DONE]\n\n";
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .insert_header("x-response-cost", "0.0034")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let (mut s, cost) = stream_once(&r, &[], &ChatParams::default())
        .await
        .expect("stream");
    assert_eq!(
        cost,
        Some(0.0034),
        "streaming response cost must be surfaced"
    );
    let mut got = String::new();
    while let Some(c) = s.next().await {
        got.push_str(&c.expect("chunk"));
    }
    assert_eq!(got, "a");
}

#[tokio::test]
async fn embed_once_parses_first_embedding() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{"embedding": [0.1, 0.2, 0.3]}]
        })))
        .mount(&server)
        .await;
    let r = req(format!("{}/embeddings", server.uri()));
    let v = embed_once(&r, "hello").await.expect("embed");
    assert_eq!(v.len(), 3);
    assert!((v[0] - 0.1).abs() < 1e-6);
}
