use vox_llm_egress::{chat_once, ChatMessage, ChatParams, EgressError, EgressRequest};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(base: String) -> EgressRequest {
    EgressRequest {
        base_url: base,
        api_key: "secret".into(),
        model: "test/model".into(),
        headers: vec![("X-Title".into(), "vox".into())],
        throttle_key: "test-or".into(),
        max_concurrent: 4,
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
    let msgs = vec![ChatMessage { role: "user".into(), content: "yo".into() }];
    let out = chat_once(&r, &msgs, &ChatParams::default()).await.expect("ok");
    assert_eq!(out.content, "hi");
    assert_eq!(out.prompt_tokens, 5);
    assert_eq!(out.completion_tokens, 2);
    assert_eq!(out.model, "test/model");
}

#[tokio::test]
async fn chat_once_maps_429_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let err = chat_once(&r, &[], &ChatParams::default()).await.unwrap_err();
    assert!(
        matches!(err, EgressError::RateLimited { retry_after: Some(_) }),
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
    let err = chat_once(&r, &[], &ChatParams::default()).await.unwrap_err();
    assert!(matches!(err, EgressError::Status { code: 500, .. }), "got {err:?}");
}
