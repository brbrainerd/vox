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

/// Returns OpenRouter attribution / routing headers for a `(provider, model)` pair.
///
/// Mirrors the orchestrator bridge's `extra_headers_for`
/// (`vox-orchestrator-mcp::llm_bridge::provider_auth`) so that the facade streaming
/// and non-streaming paths emit the same app attribution that the bridge does:
/// - `HTTP-Referer` (only if [`SecretId::VoxOpenrouterHttpReferer`] is non-empty)
/// - `X-Title` (only if [`SecretId::VoxOpenrouterAppTitle`] is non-empty)
/// - `X-OpenRouter-Provider-Preferences` route hint, only for the `openrouter/auto`
///   virtual model.
///
/// Returns an empty vec for non-OpenRouter providers, leaving them unchanged.
pub(super) fn openrouter_extra_headers(provider: &str, model: &str) -> Vec<(&'static str, String)> {
    let mut headers = Vec::new();
    if provider != "openrouter" {
        return headers;
    }

    if let Some(v) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenrouterHttpReferer).expose()
    {
        if !v.trim().is_empty() {
            headers.push(("HTTP-Referer", v.to_string()));
        }
    }
    if let Some(v) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenrouterAppTitle).expose()
    {
        if !v.trim().is_empty() {
            headers.push(("X-Title", v.to_string()));
        }
    }
    // For the virtual auto-routing model, inject the cost-preference route hint so
    // OpenRouter's broker picks the provider matching our intent.
    if model == vox_config::OPENROUTER_AUTO {
        let hint = openrouter_route_hint_from_env();
        headers.push((
            "X-OpenRouter-Provider-Preferences",
            format!("{{\"route\":\"{}\"}}", hint.as_route_str()),
        ));
    }
    headers
}

/// Resolve the [`vox_config::OpenRouterRouteHint`] from the route-hint / cost-preference
/// secrets. Mirrors the bridge's `openrouter_route_hint_from_env`.
fn openrouter_route_hint_from_env() -> vox_config::OpenRouterRouteHint {
    use vox_config::{OpenRouterRouteHint, RouteCostPreference, derive_openrouter_route_hint};
    let raw = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenrouterRouteHint)
        .expose()
        .unwrap_or("")
        .to_string();
    match raw.trim().to_ascii_lowercase().as_str() {
        "price" | "economy" | "cheap" => OpenRouterRouteHint::Price,
        "quality" | "performance" | "best" => OpenRouterRouteHint::Quality,
        "fallback" | "resilience" => OpenRouterRouteHint::Fallback,
        _ => {
            let pref_raw = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCostPreference)
                .expose()
                .unwrap_or("")
                .to_string();
            let pref = match pref_raw.trim().to_ascii_lowercase().as_str() {
                "performance" | "quality" => RouteCostPreference::Performance,
                _ => RouteCostPreference::Economy,
            };
            derive_openrouter_route_hint(pref)
        }
    }
}
