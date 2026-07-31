//! Sanctioned low-layer LLM provider egress. Pure wire: callers pass a fully-resolved
//! [`EgressRequest`] (resolution lives in `vox_config::resolve_egress`); this crate does
//! throttle + HTTP + 429 handling + response parsing. It owns NO config/secret resolution
//! and NO telemetry-to-db (both pull higher layers) — `chat_once` returns the tokens/cost
//! callers need to record telemetry themselves.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub mod throttle;
mod wire;

pub use throttle::{Permit, acquire_permit, on_rate_limited, on_success, retry_after_from_headers};
pub use wire::{chat_once, embed_once, stream_once};

/// A fully-resolved provider request. No resolution happens in this crate.
///
/// `Debug` is hand-written to **redact `api_key`** so the bearer token can never leak into
/// logs/traces/error messages. `headers` here carry only non-secret attribution
/// (HTTP-Referer / X-Title); the bearer is applied separately from `api_key`.
#[derive(Clone)]
pub struct EgressRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub headers: Vec<(String, String)>,
    pub throttle_key: String,
    /// Max concurrent in-flight requests for this provider's throttle (resolved from
    /// VoxConfig by `vox_config::resolve_egress`; first call per provider wins).
    pub max_concurrent: usize,
    /// Per-request HTTP timeout for **unary** calls (chat/embed); resolved by
    /// `vox_config::resolve_egress`. `None` = no deadline. `stream_once` ignores this
    /// (a whole-request deadline would truncate long SSE streams).
    pub timeout_ms: Option<u64>,
}

impl std::fmt::Debug for EgressRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Show only whether a key is present, never its value.
        let api_key = if self.api_key.is_empty() { "" } else { "***" };
        f.debug_struct("EgressRequest")
            .field("base_url", &self.base_url)
            .field("api_key", &api_key)
            .field("model", &self.model)
            .field("headers", &self.headers)
            .field("throttle_key", &self.throttle_key)
            .field("max_concurrent", &self.max_concurrent)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

/// One chat message on the wire (OpenAI-compatible).
///
/// The three trailing fields are additive plumbing for a full tool-calling turn (Task
/// 1.3b): an assistant message MAY carry `tool_calls` (the calls it requested), and a
/// `role: "tool"` result message carries `tool_call_id` (and optionally `name`) to
/// correlate the result back to the specific call. All three are
/// `skip_serializing_if = "Option::is_none"` so every existing plain `{role, content}`
/// caller (ghost_text, inline_edit, plan, eval, judge, route, …) continues to serialize
/// an unchanged wire payload — no `tool_calls`/`tool_call_id`/`name` keys at all, not
/// even `null`, when left `None`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// Set on an assistant message that requested tool calls. Reuses
    /// [`EgressToolCall`] (the inbound/parsed shape from Task 1.3a); `build_request`
    /// re-serializes `arguments` back to a JSON **string** for the outbound wire
    /// format (the inverse of the inbound parse), since `EgressToolCall::arguments` is
    /// eagerly parsed to `serde_json::Value` for callers' convenience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<EgressToolCall>>,
    /// Set on a `role: "tool"` result message: the id of the call this result answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optionally set on a `role: "tool"` result message: some OpenAI-compatible APIs
    /// expect the tool's name alongside `tool_call_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Tool definition passed through to the provider.
#[derive(Clone, Debug, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

/// A single tool invocation the model requested, parsed from the OpenAI-compatible
/// `message.tool_calls` wire shape: `{"id": "...", "type": "function", "function":
/// {"name": "...", "arguments": "<json-encoded string>"}}`.
///
/// This crate defines its own minimal type here rather than reusing
/// `vox_openai::chat_completion::ChatCompletionToolCall` (which this crate already
/// depends on for other reasons) for two reasons: (1) that type has no `id` field, which
/// a future tool-dispatch loop needs to correlate a call with its result message, and
/// (2) `chat_once` parses the whole response body as a raw `serde_json::Value` rather
/// than through `vox_openai`'s typed `ChatCompletionResponse` — pulling in a typed
/// sub-struct for just this one field would be inconsistent with the rest of this file.
/// A future consolidation task may want to unify these; not done here to keep this
/// change to pure additive plumbing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EgressToolCall {
    /// Provider-assigned id for this tool call (used to correlate a subsequent
    /// tool-result message back to this invocation).
    pub id: String,
    /// Tool/function name requested.
    pub name: String,
    /// Parsed JSON arguments. The wire format sends `function.arguments` as a
    /// JSON-encoded **string**, not a nested object; we eagerly parse it here so
    /// callers receive a `serde_json::Value` directly. If the string fails to parse
    /// as JSON (malformed provider output), this falls back to `Value::Null` rather
    /// than failing the whole response — deciding how to handle empty/invalid
    /// arguments is left to the tool-dispatch loop (a separate task).
    pub arguments: serde_json::Value,
}

/// Per-call generation parameters.
#[derive(Clone, Debug, Default)]
pub struct ChatParams<'a> {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u64>,
    pub response_format: Option<&'a serde_json::Value>,
    pub tools: Option<&'a [ToolDef]>,
    pub tool_choice: Option<&'a serde_json::Value>,
}

/// Parsed chat result. Carries usage/cost/latency so callers record telemetry.
#[derive(Clone, Debug)]
pub struct EgressChatResponse {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Cached prompt tokens (`usage.cache_read_input_tokens` or
    /// `usage.prompt_tokens_details.cached_tokens`); 0 when absent.
    pub cache_read_tokens: u32,
    pub model: String,
    /// Provider-reported cost (`usage.total_cost`/`usage.cost`, else the
    /// `x-response-cost` header). `None` when the provider reports none — callers may
    /// apply their own cost-per-1k estimate.
    pub cost_usd: Option<f64>,
    pub latency_ms: u64,
    /// Tool calls the model requested (`message.tool_calls`), when tools were passed
    /// in the request and the model chose to invoke one or more. `None` for the common
    /// case of a plain text response (no tools requested, or the model answered in
    /// text instead of calling a tool).
    pub tool_calls: Option<Vec<EgressToolCall>>,
}

/// Structured egress failure so callers map to their own error types.
#[derive(Debug)]
pub enum EgressError {
    RateLimited { retry_after: Option<Duration> },
    Http(String),
    Status { code: u16, body: String },
    Decode(String),
}

impl std::fmt::Display for EgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EgressError::RateLimited { retry_after } => {
                write!(f, "rate limited (retry_after={retry_after:?})")
            }
            EgressError::Http(e) => write!(f, "http error: {e}"),
            EgressError::Status { code, body } => write!(f, "provider status {code}: {body}"),
            EgressError::Decode(e) => write!(f, "decode error: {e}"),
        }
    }
}
impl std::error::Error for EgressError {}

/// Streaming item type for [`stream_once`].
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String, EgressError>> + Send>>;

/// The ONE cost estimate: `(prompt+completion tokens)/1000 * cost_per_1k`. Used only when
/// the provider reports no cost. Callers MUST NOT re-implement this math (single source).
#[must_use]
pub fn estimate_cost(prompt_tokens: u32, completion_tokens: u32, cost_per_1k: f64) -> f64 {
    ((prompt_tokens + completion_tokens) as f64 / 1000.0) * cost_per_1k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egress_request_is_constructible() {
        let r = EgressRequest {
            base_url: "https://openrouter.ai/api/v1/chat/completions".into(),
            api_key: "k".into(),
            model: "x".into(),
            headers: vec![("X-Title".into(), "vox".into())],
            throttle_key: "openrouter".into(),
            max_concurrent: 8,
            timeout_ms: None,
        };
        assert_eq!(r.throttle_key, "openrouter");
    }

    #[test]
    fn estimate_cost_is_tokens_over_1k_times_rate() {
        assert!((estimate_cost(700, 300, 2.0) - 2.0).abs() < 1e-9); // 1000/1000 * 2.0
        assert_eq!(estimate_cost(0, 0, 5.0), 0.0);
    }

    #[test]
    fn plain_text_message_serializes_with_no_tool_keys() {
        // Old-style plain-text construction (pre-1.3b shape): tool fields left `None`.
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        let obj = json.as_object().expect("object");
        assert_eq!(obj.len(), 2, "only role+content must be present: {obj:?}");
        assert!(obj.contains_key("role"));
        assert!(obj.contains_key("content"));
        assert!(!obj.contains_key("tool_calls"));
        assert!(!obj.contains_key("tool_call_id"));
        assert!(!obj.contains_key("name"));
    }

    #[test]
    fn assistant_message_carries_tool_calls_with_parsed_value_arguments() {
        // `ChatMessage::tool_calls` carries `EgressToolCall` directly, so at THIS type's
        // own serialization boundary `arguments` is still the parsed `serde_json::Value`
        // (matching 1.3a's inbound shape) — the JSON-string conversion for the actual
        // outbound wire happens one layer down, in `vox_llm_egress::wire::build_request`
        // (via its private `WireToolCall`), exercised by
        // `chat_once_serializes_assistant_tool_calls_and_tool_result_message` in
        // `tests/wire_mock.rs`. This test locks in that `ChatMessage` itself is a
        // faithful passthrough (no premature/duplicate re-encoding at this layer).
        let msg = ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(vec![EgressToolCall {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: serde_json::json!({"city": "Paris"}),
            }]),
            tool_call_id: None,
            name: None,
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        let calls = json["tool_calls"].as_array().expect("tool_calls array");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["name"], "get_weather");
        assert_eq!(calls[0]["arguments"], serde_json::json!({"city": "Paris"}));
    }

    #[test]
    fn tool_result_message_serializes_tool_call_id_and_name() {
        let msg = ChatMessage {
            role: "tool".into(),
            content: "72F and sunny".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            name: Some("get_weather".into()),
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["role"], "tool");
        assert_eq!(json["content"], "72F and sunny");
        assert_eq!(json["tool_call_id"], "call_1");
        assert_eq!(json["name"], "get_weather");
        assert!(json.get("tool_calls").is_none());
    }

    #[test]
    fn debug_redacts_api_key() {
        let r = EgressRequest {
            base_url: "https://x/api".into(),
            api_key: "sk-supersecret-token".into(),
            model: "m".into(),
            headers: vec![("X-Title".into(), "vox".into())],
            throttle_key: "openrouter".into(),
            max_concurrent: 8,
            timeout_ms: None,
        };
        let dbg = format!("{r:?}");
        assert!(
            !dbg.contains("sk-supersecret-token"),
            "api_key must never appear in Debug: {dbg}"
        );
        assert!(dbg.contains("***"), "present key should render as ***");
        // An empty key renders empty (distinguishable), still no secret.
        let empty = EgressRequest {
            api_key: String::new(),
            ..r
        };
        assert!(!format!("{empty:?}").contains("***"));
    }
}
