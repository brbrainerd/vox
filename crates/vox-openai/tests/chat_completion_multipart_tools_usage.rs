//! Multipart message serialization and tool-call / usage field parsing (`vox-openai-wire`).

use serde_json::json;
use vox_openai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessageContent, ChatMessagePart,
    ChatMessageTurn, ImageUrl,
};

#[test]
fn chat_completion_request_serializes_multipart_with_image_url() {
    let req = ChatCompletionRequest {
        model: "vision-model",
        messages: vec![ChatMessageTurn {
            role: "user",
            content: ChatMessageContent::Parts(vec![
                ChatMessagePart::Text { text: "describe" },
                ChatMessagePart::ImageUrl {
                    image_url: ImageUrl {
                        url: "https://example.com/x.png",
                    },
                },
            ]),
        }],
        temperature: None,
        max_tokens: 64,
        stream: false,
        top_p: None,
        response_format: None,
        tools: None,
        tool_choice: None,
    };
    let v = serde_json::to_value(&req).expect("serialize");
    let parts = &v["messages"][0]["content"];
    assert!(parts.is_array());
    assert_eq!(parts[0]["type"], "text");
    assert_eq!(parts[0]["text"], "describe");
    assert_eq!(parts[1]["type"], "image_url");
    assert_eq!(parts[1]["image_url"]["url"], "https://example.com/x.png");
}

#[test]
fn chat_completion_response_parses_tool_calls_and_usage_extras() {
    let body = json!({
        "choices": [{
            "message": {
                "content": null,
                "tool_calls": [{
                    "function": {
                        "name": "lookup",
                        "arguments": "{\"q\":\"rust\"}"
                    }
                }]
            }
        }],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 2,
            "cost": 0.001,
            "total_cost": 0.002,
            "prompt_tokens_details": { "cached_tokens": 7 },
            "cache_creation_input_tokens": 3,
            "cache_read_input_tokens": 4
        },
        "model": "m",
        "id": "cmpl-tools"
    });

    let parsed: ChatCompletionResponse = serde_json::from_value(body).expect("parse");
    let msg = parsed
        .choices
        .first()
        .and_then(|c| c.message.as_ref())
        .expect("assistant message");
    let calls = msg.tool_calls.as_ref().expect("tool_calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "lookup");
    assert_eq!(calls[0].function.arguments, "{\"q\":\"rust\"}");

    let u = parsed.usage.expect("usage");
    assert_eq!(u.prompt_tokens, 1);
    assert_eq!(u.completion_tokens, 2);
    assert_eq!(u.cost, Some(0.001));
    assert_eq!(u.total_cost, Some(0.002));
    let details = u.prompt_tokens_details.expect("prompt_tokens_details");
    assert_eq!(details.cached_tokens, 7);
    assert_eq!(u.cache_creation_input_tokens, Some(3));
    assert_eq!(u.cache_read_input_tokens, Some(4));
}
