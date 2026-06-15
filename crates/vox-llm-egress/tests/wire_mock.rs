use futures::StreamExt;
use vox_llm_egress::{
    ChatMessage, ChatParams, EgressError, EgressRequest, ToolDef, chat_once, embed_once,
    stream_once,
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
        .and(header("authorization", "Bearer secret"))
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
    }];
    let out = chat_once(&r, &msgs, &ChatParams::default())
        .await
        .expect("ok");
    assert_eq!(out.content, "hi");
    assert_eq!(out.prompt_tokens, 5);
    assert_eq!(out.completion_tokens, 2);
    assert_eq!(out.model, "test/model");
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
    let mut s = stream_once(&r, &[], &ChatParams::default())
        .await
        .expect("stream");
    let mut got = String::new();
    while let Some(chunk) = s.next().await {
        got.push_str(&chunk.expect("chunk"));
    }
    assert_eq!(got, "ab");
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
