//! Wire JSON shapes and API key resolution for chat / stream.

use serde::Serialize;
pub use vox_openai::{
    ChatCompletionResponse as OpenRouterResponse, ChatCompletionUsage as OpenRouterUsage,
};

use super::types::{ChatMessage, LlmConfig};

#[derive(Serialize)]
pub(super) struct OpenRouterRequest<'a> {
    pub(super) model: &'a str,
    pub(super) messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) response_format: Option<&'a serde_json::Value>,
    pub(super) stream: bool,
}

pub(super) fn resolve_chat_api_key(config: &LlmConfig) -> String {
    config
        .api_key
        .clone()
        .unwrap_or_else(|| match config.provider.as_str() {
            "openrouter" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
                .expose()
                .unwrap_or_default()
                .to_string(),
            "openai" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiApiKey)
                .expose()
                .unwrap_or_default()
                .to_string(),
            "anthropic" => vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicApiKey)
                .expose()
                .unwrap_or_default()
                .to_string(),
            "hf_router" | "huggingface" | "hf_endpoint" => {
                vox_config::inference::huggingface_hub_token().unwrap_or_default()
            }
            _ => String::new(),
        })
}

pub(super) fn chat_requires_nonempty_api_key(provider: &str) -> bool {
    matches!(provider, "openrouter" | "openai" | "anthropic")
}
