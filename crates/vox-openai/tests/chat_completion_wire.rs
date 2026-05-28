//! Serialize requests and parse minimal completion JSON (`vox-openai-wire`).

use serde_json::json;
use vox_openai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessageContent, ChatMessageTurn,
};

#[test]
fn chat_completion_request_serializes_expected_keys() {
    let req = ChatCompletionRequest {
        model: "test-model",
        messages: vec![ChatMessageTurn {
            role: "user",
            content: ChatMessageContent::Text("hello"),
        }],
        temperature: Some(0.2),
        max_tokens: 128,
        stream: false,
        top_p: None,
        response_format: None,
        tools: None,
        tool_choice: None,
    };
    let v = serde_json::to_value(&req).expect("serialize request");
    assert_eq!(v["model"], "test-model");
    assert_eq!(v["stream"], false);
    assert_eq!(v["max_tokens"], 128);
    assert!(v["messages"].is_array());
}

#[test]
fn chat_completion_response_deserializes_minimal_success_body() {
    let body = json!({
        "choices": [{ "message": { "content": "world", "tool_calls": null } }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 5 },
        "model": "m",
        "id": "cmpl-1"
    });
    let parsed: ChatCompletionResponse = serde_json::from_value(body).expect("parse response");
    assert_eq!(parsed.choices.len(), 1);
    let msg = parsed.choices[0].message.as_ref().expect("message");
    assert_eq!(msg.content.as_deref(), Some("world"));
    let usage = parsed.usage.expect("usage");
    assert_eq!(usage.prompt_tokens, 3);
    assert_eq!(usage.completion_tokens, 5);
}
