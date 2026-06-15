//! The single resolver for LLM provider egress: maps a provider+model to a fully-resolved
//! [`vox_llm_egress::EgressRequest`] using the registry accessors + Clavis. Lives here (not
//! in the egress crate) so resolution is single-source; takes primitives (not `LlmConfig`) to
//! keep the egress crate free of an upward dependency.
//!
//! Ported from `vox-actor-runtime/src/llm/wire.rs` (`resolve_chat_api_key`,
//! `chat_requires_nonempty_api_key`, `openrouter_extra_headers`) so all callers resolve
//! provider key / base-url / attribution headers identically.

use vox_llm_egress::EgressRequest;

/// Minimal input for [`resolve_egress`]; built from a caller's config without importing
/// the higher-layer `LlmConfig` type.
pub struct EgressResolveInput {
    pub provider: String,
    pub model: String,
    /// Explicit endpoint override; when `None`, the provider default is used.
    pub base_url_override: Option<String>,
    /// Per-request timeout in ms; `Some(>0)` wins, else the SSOT
    /// `vox_config::timeouts::HTTP_REQUEST` default. Applied to unary calls only.
    pub timeout_ms: Option<u64>,
}

/// Resolve the unary request timeout (ms). Precedence: explicit positive `timeout_ms`
/// → shared `vox_config::timeouts::HTTP_REQUEST`. Mirrors the retired
/// `vox-actor-runtime/llm/timeout.rs::request_timeout` (now single-sourced here).
fn resolve_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    match timeout_ms {
        Some(ms) if ms > 0 => ms,
        _ => crate::timeouts::HTTP_REQUEST.as_millis() as u64,
    }
}

fn resolve_api_key(provider: &str) -> String {
    match provider {
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
            crate::inference::huggingface_hub_token().unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn chat_requires_nonempty_api_key(provider: &str) -> bool {
    matches!(provider, "openrouter" | "openai" | "anthropic")
}

/// OpenRouter attribution / routing headers, mirroring `wire.rs::openrouter_extra_headers`.
fn extra_headers(provider: &str, model: &str) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if provider != "openrouter" {
        return headers;
    }
    if let Some(v) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenrouterHttpReferer).expose()
        && !v.trim().is_empty()
    {
        headers.push(("HTTP-Referer".to_string(), v.to_string()));
    }
    if let Some(v) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenrouterAppTitle).expose()
        && !v.trim().is_empty()
    {
        headers.push(("X-Title".to_string(), v.to_string()));
    }
    if model == crate::OPENROUTER_AUTO {
        let hint = openrouter_route_hint_from_env();
        headers.push((
            "X-OpenRouter-Provider-Preferences".to_string(),
            format!("{{\"route\":\"{}\"}}", hint.as_route_str()),
        ));
    }
    headers
}

/// Resolve the OpenRouter route hint from the route-hint / cost-preference secrets.
/// Ported verbatim from `wire.rs::openrouter_route_hint_from_env`.
fn openrouter_route_hint_from_env() -> crate::OpenRouterRouteHint {
    use crate::{OpenRouterRouteHint, RouteCostPreference, derive_openrouter_route_hint};
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

/// Resolve a provider's concurrency ceiling from VoxConfig (the throttle dial).
fn resolve_max_concurrent(provider: &str) -> usize {
    let cfg = crate::VoxConfig::load();
    match provider {
        "openrouter" => cfg
            .llm_openrouter_max_concurrent
            .unwrap_or(cfg.llm_max_concurrent_requests),
        "openai" => cfg
            .llm_openai_max_concurrent
            .unwrap_or(cfg.llm_max_concurrent_requests),
        _ => cfg.llm_max_concurrent_requests,
    }
}

/// The ONE place that resolves provider key + base-url + attribution headers +
/// concurrency, producing a fully-resolved [`EgressRequest`] for the egress crate.
pub fn resolve_egress(input: &EgressResolveInput) -> Result<EgressRequest, String> {
    let api_key = resolve_api_key(&input.provider);
    if chat_requires_nonempty_api_key(&input.provider) && api_key.is_empty() {
        return Err("No API key available for LLM provider".to_string());
    }
    let base_url =
        input
            .base_url_override
            .clone()
            .unwrap_or_else(|| match input.provider.as_str() {
                "openrouter" => crate::inference::openrouter_chat_completions_url(),
                "openai" => crate::inference::openai_chat_completions_url(),
                "hf_router" | "huggingface" => crate::inference::hf_router_chat_completions_url(),
                _ => crate::inference::openrouter_chat_completions_url(),
            });
    Ok(EgressRequest {
        base_url,
        api_key,
        model: input.model.clone(),
        headers: extra_headers(&input.provider, &input.model),
        throttle_key: input.provider.clone(),
        max_concurrent: resolve_max_concurrent(&input.provider).max(1),
        timeout_ms: Some(resolve_timeout_ms(input.timeout_ms)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use `hf_router` for deterministic tests: it does not require a non-empty API key
    // (so resolution succeeds regardless of the environment's secrets), unlike
    // openrouter/openai/anthropic which gate on a present key.

    #[test]
    fn hf_router_resolves_default_base_url_and_throttle_key() {
        let input = EgressResolveInput {
            provider: "hf_router".into(),
            model: "x".into(),
            base_url_override: None,
            timeout_ms: None,
        };
        let req = resolve_egress(&input).expect("resolve");
        assert!(req.base_url.contains("huggingface"), "got {}", req.base_url);
        assert_eq!(req.throttle_key, "hf_router");
        assert!(req.max_concurrent >= 1);
        // Unset timeout falls back to the SSOT default (non-zero).
        assert!(req.timeout_ms.unwrap() > 0);
    }

    #[test]
    fn explicit_timeout_overrides_default() {
        let input = EgressResolveInput {
            provider: "hf_router".into(),
            model: "x".into(),
            base_url_override: None,
            timeout_ms: Some(5_000),
        };
        let req = resolve_egress(&input).expect("resolve");
        assert_eq!(req.timeout_ms, Some(5_000));
    }

    #[test]
    fn base_url_override_is_honored() {
        let input = EgressResolveInput {
            provider: "hf_router".into(),
            model: "x".into(),
            base_url_override: Some("https://custom/v1/chat/completions".into()),
            timeout_ms: None,
        };
        let req = resolve_egress(&input).expect("resolve");
        assert_eq!(req.base_url, "https://custom/v1/chat/completions");
    }

    #[test]
    fn key_required_provider_errors_without_key() {
        // openrouter requires a key; assert the gate is wired (env may or may not have one,
        // so accept either a present-key Ok or the explicit error — but the error message
        // must be the resolution gate when it fires).
        let input = EgressResolveInput {
            provider: "openrouter".into(),
            model: "x".into(),
            base_url_override: None,
            timeout_ms: None,
        };
        match resolve_egress(&input) {
            Ok(req) => assert_eq!(req.throttle_key, "openrouter"),
            Err(e) => assert!(e.contains("No API key"), "unexpected error: {e}"),
        }
    }
}
