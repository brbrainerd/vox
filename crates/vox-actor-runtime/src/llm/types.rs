//! Public LLM config, messages, metrics, and response types.

use serde::{Deserialize, Serialize};

use crate::inference_env::HF_ROUTER_CHAT_COMPLETIONS_URL;

/// OpenAI-compatible tool definition for chat completions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmToolDef {
    /// Function/tool name exposed to the provider.
    pub name: String,
    /// Optional human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema object describing tool arguments.
    pub parameters: serde_json::Value,
}

/// Message format for the LLM chat API wire protocol (OpenAI-compatible).
///
/// Mirrors `vox_llm_egress::ChatMessage`'s additive tool-calling fields (Task 1.3b):
/// `tool_calls` on an assistant message, `tool_call_id`/`name` on a `role: "tool"`
/// result message. All `skip_serializing_if = "Option::is_none"` so existing plain
/// `{role, content}` callers are unaffected.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmChatMessage {
    /// Chat role string (`system`, `user`, `assistant`, …).
    pub role: String,
    /// Message body text.
    pub content: String,
    /// Set on an assistant message that requested tool calls. Reuses
    /// `vox_llm_egress::EgressToolCall` directly (this crate already depends on
    /// vox-llm-egress) rather than duplicating the type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<vox_llm_egress::EgressToolCall>>,
    /// Set on a `role: "tool"` result message: the id of the call this result answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optionally set on a `role: "tool"` result message alongside `tool_call_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Deprecated alias kept for callers within this crate during the rename.
#[allow(dead_code)]
pub(crate) type ChatMessage = LlmChatMessage;

/// A configuration block for an LLM provider integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider key (e.g. `openrouter`, `openai`, `anthropic`, `hf_router`).
    pub provider: String,
    /// Provider-specific model id (e.g. `anthropic/claude-3.5-sonnet`).
    pub model: String,
    /// Estimated cost per 1000 tokens for this model.
    pub cost_per_1k: Option<f64>,
    /// Override chat completions URL; defaults are chosen from `provider`.
    pub base_url: Option<String>,
    /// API key or bearer token when the provider requires one.
    pub api_key: Option<String>,
    /// Sampling temperature when supported by the endpoint.
    pub temperature: Option<f32>,
    /// Sampling top_p when supported by the endpoint.
    pub top_p: Option<f32>,
    /// Maximum tokens to generate when supported.
    pub max_tokens: Option<u64>,
    /// Optional JSON Schema / response-format object for structured output.
    pub response_format: Option<serde_json::Value>,
    /// Optional function tools forwarded to OpenAI-compatible chat APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<LlmToolDef>>,
    /// Optional tool choice directive (`auto`, `none`, `required`, or function object).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    /// Optional HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional telemetry session identifier for database attribution.
    pub telemetry_session_id: Option<String>,
    /// Optional telemetry user identifier.
    pub telemetry_user_id: Option<String>,
    /// Optional task category for model scoreboard aggregation.
    pub telemetry_task_category: Option<String>,
    /// Optional strength tag for model scoreboard aggregation.
    pub telemetry_strength_tag: Option<String>,
    /// Optional trace identifier for distributed tracing.
    pub telemetry_trace_id: Option<String>,
    /// Optional attempt number within a retry chain.
    pub telemetry_attempt_number: Option<i32>,
    /// Whether to skip recording the final interaction in leaf calls.
    pub telemetry_skip_interaction: bool,
}

impl LlmConfig {
    pub fn openrouter(model: impl Into<String>) -> Self {
        Self {
            provider: "openrouter".into(),
            model: model.into(),
            cost_per_1k: None,
            base_url: Some(vox_config::openrouter_chat_completions_url()),
            api_key: vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
                .expose()
                .map(std::string::ToString::to_string),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: None,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: false,
        }
    }

    pub fn openai(model: impl Into<String>) -> Self {
        Self {
            provider: "openai".into(),
            model: model.into(),
            cost_per_1k: None,
            base_url: Some(vox_config::openai_chat_completions_url()),
            api_key: vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiApiKey)
                .expose()
                .map(std::string::ToString::to_string),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: None,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: false,
        }
    }

    pub fn huggingface_router(model: impl Into<String>) -> Self {
        Self {
            provider: "hf_router".into(),
            model: model.into(),
            cost_per_1k: None,
            base_url: Some(HF_ROUTER_CHAT_COMPLETIONS_URL.to_string()),
            api_key: vox_config::inference::huggingface_hub_token(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: None,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: false,
        }
    }

    /// Resolve from a model registry alias.
    ///
    /// `registry` maps alias names (e.g. `"fast"`, `"smart"`) to
    /// `(provider, model_id, temperature, api_key_env)` tuples.
    pub fn from_registry(
        alias: &str,
        registry: &std::collections::HashMap<String, ModelRegistryEntry>,
    ) -> Result<Self, String> {
        let entry = registry
            .get(alias)
            .ok_or_else(|| format!("Unknown model alias: {}", alias))?;
        let api_key = match entry.provider.as_str() {
            "openrouter" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
                .expose()
                .map(std::string::ToString::to_string),
            "openai" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiApiKey)
                .expose()
                .map(std::string::ToString::to_string),
            "anthropic" => vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicApiKey)
                .expose()
                .map(std::string::ToString::to_string),
            "hf_router" | "huggingface" | "hf_endpoint" => {
                vox_config::inference::huggingface_hub_token()
            }
            _ => None,
        }
        .or_else(|| {
            // Compatibility escape hatch for custom providers not yet mapped into secrets `SecretId`.
            entry
                .api_key_env
                .as_deref()
                .and_then(|env_name| std::env::var(env_name).ok())
        });
        let base_url = entry
            .base_url
            .clone()
            .or_else(|| match entry.provider.as_str() {
                "openrouter" => Some(vox_config::openrouter_chat_completions_url()),
                "openai" => Some(vox_config::openai_chat_completions_url()),
                "hf_router" | "huggingface" => Some(HF_ROUTER_CHAT_COMPLETIONS_URL.to_string()),
                "hf_endpoint" => None,
                _ => None,
            });
        Ok(Self {
            provider: entry.provider.clone(),
            model: entry.model.clone(),
            cost_per_1k: None,
            base_url,
            api_key,
            temperature: entry.temperature,
            top_p: entry.top_p,
            max_tokens: entry.max_tokens,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: entry.timeout_ms,
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: None,
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: false,
        })
    }
}

/// An entry in a Vox `@config model_registry:` block, deserialized at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryEntry {
    /// Provider family for this alias.
    pub provider: String,
    /// Model id passed to the provider API.
    pub model: String,
    /// Default temperature for this alias.
    pub temperature: Option<f32>,
    /// Default top_p for this alias.
    pub top_p: Option<f32>,
    /// Default max output tokens for this alias.
    pub max_tokens: Option<u64>,
    /// Name of an environment variable holding the API key, if any.
    pub api_key_env: Option<String>,
    /// Optional override for the chat completions URL.
    pub base_url: Option<String>,
    /// Optional HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// Tracks token usage and cost per LLM call — stored in @table ModelMetric.
/// Serializable so it can be persisted to VoxDB directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetric {
    /// Millisecond-timestamp of the completion.
    pub ts: u64,
    /// Model id as reported by the provider response.
    pub model: String,
    /// Provider key used for the call.
    pub provider: String,
    /// Prompt (input) token count from usage metadata.
    pub input_tokens: u32,
    /// Completion (output) token count from usage metadata.
    pub output_tokens: u32,
    /// Estimated cost in USD (computed from a model registry lookup if available).
    pub estimated_cost_usd: f64,
}

impl ModelMetric {
    /// Build from an LlmResponse, computing cost at `cost_per_1k` rate.
    pub fn from_response(res: &LlmResponse, provider: &str, cost_per_1k: f64) -> Self {
        Self {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            model: res.model.clone(),
            provider: provider.to_string(),
            input_tokens: res.prompt_tokens,
            output_tokens: res.completion_tokens,
            // Single source for the estimate (vox-llm-egress::estimate_cost).
            estimated_cost_usd: vox_llm_egress::estimate_cost(
                res.prompt_tokens,
                res.completion_tokens,
                cost_per_1k,
            ),
        }
    }
}

/// The standard parsed response from an LLM chat operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Assistant message text from the first choice.
    pub content: String,
    /// Prompt token usage when the API returned it.
    pub prompt_tokens: u32,
    /// Completion token usage when the API returned it.
    pub completion_tokens: u32,
    /// Model id from the response body, or the configured model as fallback.
    pub model: String,
    /// Cost of this call in USD when derivable: the provider-reported
    /// `usage.total_cost`/`usage.cost`, else a `cost_per_1k` token estimate,
    /// else `None`. This is the same value recorded to telemetry; surfacing it
    /// on the response lets callers (e.g. Scientia pipeline phases) attribute
    /// per-phase spend without re-deriving it.
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// Tool calls the model requested, threaded through from
    /// `vox_llm_egress::EgressChatResponse::tool_calls` (reused directly rather than
    /// duplicated here, since this crate already depends on `vox-llm-egress`).
    /// `#[serde(default)]` so legacy payloads without this field deserialize to `None`,
    /// mirroring the `cost_usd` addition above.
    #[serde(default)]
    pub tool_calls: Option<Vec<vox_llm_egress::EgressToolCall>>,
    /// Wall-clock latency of the egress call in milliseconds, as measured by
    /// `vox_llm_egress`. Surfaced here so multi-candidate callers such as
    /// `infer_with_retry` can record the real elapsed time instead of the
    /// hardcoded `0` they used to write to `llm_interactions.latency_ms`.
    #[serde(default)]
    pub latency_ms: u64,
    /// Cache-read prompt tokens the provider reported, threaded through for the
    /// same reason as `latency_ms`.
    #[serde(default)]
    pub cache_read_tokens: u32,
    /// Task M3: time to first token, in ms — wall-clock from issuing the request to the
    /// first content chunk received. Only genuinely measured on the streaming path
    /// (`agent_loop.rs`'s `stream_final_answer`); the non-streaming path has no partial
    /// data, so it reports `latency_ms` here (time-to-first-token equals time-to-whole-
    /// response when nothing streams). `None` when not computed at all (e.g. an error path).
    #[serde(default)]
    pub ttft_ms: Option<u64>,
    /// Task M3: time per output token, in ms — `(latency_ms - ttft_ms) / completion_tokens`
    /// on the streaming path (the generation phase after the first token), or
    /// `latency_ms / completion_tokens` non-streaming. `None` when `completion_tokens == 0`.
    #[serde(default)]
    pub tpot_ms: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::{LlmConfig, LlmResponse, ModelRegistryEntry};
    use std::collections::HashMap;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn llm_response_carries_and_roundtrips_cost_usd() {
        let resp = LlmResponse {
            content: "hi".into(),
            prompt_tokens: 10,
            completion_tokens: 5,
            model: "test-model".into(),
            cost_usd: Some(0.0123),
            tool_calls: None,
            latency_ms: 42,
            cache_read_tokens: 7,
            ttft_ms: Some(15),
            tpot_ms: Some(5.4),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let back: LlmResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.cost_usd, Some(0.0123));

        // Back-compat: payloads written before the field existed deserialize to
        // None (the field is `#[serde(default)]`).
        let legacy = r#"{"content":"x","prompt_tokens":1,"completion_tokens":1,"model":"m"}"#;
        let legacy_resp: LlmResponse = serde_json::from_str(legacy).expect("legacy deserialize");
        assert_eq!(legacy_resp.cost_usd, None);
        assert_eq!(legacy_resp.tool_calls, None);
        assert_eq!(legacy_resp.latency_ms, 0);
        assert_eq!(legacy_resp.cache_read_tokens, 0);
        assert_eq!(legacy_resp.ttft_ms, None);
        assert_eq!(legacy_resp.tpot_ms, None);
        assert_eq!(back.latency_ms, 42);
        assert_eq!(back.cache_read_tokens, 7);
        assert_eq!(back.ttft_ms, Some(15));
        assert_eq!(back.tpot_ms, Some(5.4));
    }

    #[test]
    fn llm_response_carries_and_roundtrips_tool_calls() {
        let resp = LlmResponse {
            content: String::new(),
            prompt_tokens: 10,
            completion_tokens: 5,
            model: "test-model".into(),
            cost_usd: None,
            tool_calls: Some(vec![vox_llm_egress::EgressToolCall {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: serde_json::json!({"city": "Paris"}),
            }]),
            latency_ms: 0,
            cache_read_tokens: 0,
            ttft_ms: None,
            tpot_ms: None,
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let back: LlmResponse = serde_json::from_str(&json).expect("deserialize");
        let calls = back.tool_calls.expect("tool_calls must roundtrip");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, serde_json::json!({"city": "Paris"}));

        // Back-compat: legacy payloads without tool_calls deserialize to None.
        let legacy = r#"{"content":"x","prompt_tokens":1,"completion_tokens":1,"model":"m"}"#;
        let legacy_resp: LlmResponse = serde_json::from_str(legacy).expect("legacy deserialize");
        assert_eq!(legacy_resp.tool_calls, None);
    }

    // (OpenAI tool/tool_choice serialization is now owned + tested by `vox-llm-egress`
    // — see its `chat_once_serializes_tools` wiremock test. The old wire-based shape
    // test was removed with `wire.rs` when request-building moved to the egress core.)

    #[test]
    #[allow(unsafe_code)]
    fn openrouter_registry_resolution_respects_secrets_profile_modes() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let openrouter_key = "OPENROUTER_API_KEY";
        let prev_key = std::env::var(openrouter_key).ok();
        let prev_backend = std::env::var("VOX_SECRETS_BACKEND").ok();
        let prev_profile = std::env::var("VOX_SECRETS_PROFILE").ok();
        const DB_REMOTE_ALIAS_URL_ENV: &str = concat!("VOX_", "TURSO", "_URL");
        let prev_url = std::env::var(DB_REMOTE_ALIAS_URL_ENV).ok();
        let prev_cloudless_path = std::env::var("VOX_SECRETS_CLOUDLESS_DB_PATH").ok();
        let prev_account_id = std::env::var("VOX_ACCOUNT_ID").ok();
        let mut registry = HashMap::new();
        registry.insert(
            "fast".to_string(),
            ModelRegistryEntry {
                provider: "openrouter".to_string(),
                model: "openrouter/auto".to_string(),
                temperature: None,
                top_p: None,
                max_tokens: None,
                api_key_env: None,
                base_url: None,
                timeout_ms: None,
            },
        );
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "runtime-env-token");
            std::env::set_var("VOX_SECRETS_BACKEND", "vox_cloud");
            std::env::set_var("VOX_SECRETS_PROFILE", "dev");
            std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let tmp = std::env::temp_dir()
                .join(format!("vox-secrets-runtime-strict-lenient-{unique}.db"));
            std::env::set_var(
                "VOX_SECRETS_CLOUDLESS_DB_PATH",
                tmp.to_string_lossy().to_string(),
            );
            std::env::set_var("VOX_ACCOUNT_ID", "runtime-strict-lenient-test");
        }
        let lenient =
            LlmConfig::from_registry("fast", &registry).expect("lenient registry resolution");
        assert_eq!(lenient.api_key.as_deref(), Some("runtime-env-token"));

        unsafe {
            std::env::set_var("VOX_SECRETS_PROFILE", "hard_cut");
            std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
        }
        let strict = LlmConfig::from_registry("fast", &registry).expect("strict resolution");
        // OpenRouterApiKey has allow_env_in_strict=true in its SecretMetadata, so the canonical
        // env var remains readable in hard_cut profile (only deprecated aliases are blocked).
        assert_eq!(strict.api_key.as_deref(), Some("runtime-env-token"));

        unsafe {
            match prev_key {
                Some(v) => std::env::set_var("OPENROUTER_API_KEY", v),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }
            match prev_backend {
                Some(v) => std::env::set_var("VOX_SECRETS_BACKEND", v),
                None => std::env::remove_var("VOX_SECRETS_BACKEND"),
            }
            match prev_profile {
                Some(v) => std::env::set_var("VOX_SECRETS_PROFILE", v),
                None => std::env::remove_var("VOX_SECRETS_PROFILE"),
            }
            match prev_url {
                Some(v) => std::env::set_var(DB_REMOTE_ALIAS_URL_ENV, v),
                None => std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV),
            }
            match prev_cloudless_path {
                Some(v) => std::env::set_var("VOX_SECRETS_CLOUDLESS_DB_PATH", v),
                None => std::env::remove_var("VOX_SECRETS_CLOUDLESS_DB_PATH"),
            }
            match prev_account_id {
                Some(v) => std::env::set_var("VOX_ACCOUNT_ID", v),
                None => std::env::remove_var("VOX_ACCOUNT_ID"),
            }
        }
    }
}
