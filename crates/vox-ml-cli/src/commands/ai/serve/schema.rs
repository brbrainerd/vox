//! HTTP request/response types for the inference API.

#[cfg(feature = "execution-api")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "execution-api")]
#[derive(Debug, Deserialize, Clone)]
pub struct GenerateRequest {
    pub prompt: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub model: Option<String>,
    /// P015: Optional output mode for constrained decoding (strict_json, jsonl_records, tool_args_json).
    #[serde(default)]
    pub output_mode: Option<String>,
    /// P017: Max retries when output_mode is set and validation fails (default 3).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// P016: Optional JSON schema for post-generation validation (when output_mode is set).
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    /// P018: Optional flag to stream the response via SSE.
    #[serde(default)]
    pub stream: bool,
}

#[cfg(feature = "execution-api")]
fn default_max_tokens() -> usize {
    256
}
#[cfg(feature = "execution-api")]
fn default_temperature() -> f32 {
    0.7
}
#[cfg(feature = "execution-api")]
fn default_max_retries() -> u32 {
    3
}

#[cfg(feature = "execution-api")]
#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub text: String,
    /// Alias for `text` — the orchestrator's VoxLocalAdapter (provider_endpoints.rs +
    /// provider_adapter.rs) deserializes `{ code, valid, errors }` from this endpoint.
    /// Emitting both lets the same server satisfy both callers without a protocol break.
    pub code: String,
    pub tokens_generated: usize,
    pub model: String,
    pub object: &'static str,
    pub choices: Vec<Choice>,
    /// P017: Number of repair retries when output_mode validation failed (0 if none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_attempts: Option<u32>,
    /// Validation status expected by VoxLocalAdapter: true when output parsed cleanly.
    pub valid: bool,
    /// Validation errors expected by VoxLocalAdapter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

#[cfg(feature = "execution-api")]
#[derive(Debug, Serialize)]
pub struct Choice {
    pub text: String,
    pub index: usize,
    pub finish_reason: &'static str,
}
