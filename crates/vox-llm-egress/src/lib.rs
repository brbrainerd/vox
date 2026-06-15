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

pub use throttle::{
    acquire_permit, on_rate_limited, on_success, retry_after_from_headers, Permit,
};
pub use wire::{chat_once, embed_once, stream_once};

/// A fully-resolved provider request. No resolution happens in this crate.
#[derive(Clone, Debug)]
pub struct EgressRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub headers: Vec<(String, String)>,
    pub throttle_key: String,
    /// Max concurrent in-flight requests for this provider's throttle (resolved from
    /// VoxConfig by `vox_config::resolve_egress`; first call per provider wins).
    pub max_concurrent: usize,
}

/// One chat message on the wire (OpenAI-compatible).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Tool definition passed through to the provider.
#[derive(Clone, Debug, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

/// Per-call generation parameters.
#[derive(Clone, Debug, Default)]
pub struct ChatParams<'a> {
    pub temperature: Option<f32>,
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
        };
        assert_eq!(r.throttle_key, "openrouter");
    }
}
