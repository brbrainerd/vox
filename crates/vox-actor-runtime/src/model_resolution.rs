//! Chat-route → [`crate::llm::LlmConfig`] conversion (`chat_route_to_llm_config`),
//! telemetry labels (`route_telemetry_labels`), and [`RouteResolutionInput`] model
//! preferences for the research cascade builders.
//!
//! The former 7-way provider-route resolver (`resolve_chat_provider_route`) was
//! deleted 2026-07-16 (Axis GUI remediation F3); the single exercised selection
//! path is `vox_orchestrator::models::decide()` + the reactive fallback chain.
//!
//! ## Backend lane alignment (orchestrator / MCP)
//!
//! `ChatRouteBackend` (defined in `vox-orchestrator-types`) mirrors
//! `vox_orchestrator::models::ModelRouteBackend` semantics for telemetry and cross-surface dashboards.
//! `vox-actor-runtime` does **not** depend on `vox-orchestrator` (avoids cycles); keep the
//! two enums logically in sync with `vox_orchestrator::models::route_backend_for_model` for registry-backed models.
//! Chat-only routes add extra shapes (HF router/dedicated, manual OpenAI-compatible); those map to
//! `ChatRouteBackend::CascadeFallback` unless the manual URL is Google Generative Language API (→ `ChatRouteBackend::GeminiDirect`).

use crate::llm::LlmConfig;
pub use vox_orchestrator_types::{
    ChatProviderRouteKind, ChatRouteBackend, backend_telemetry_labels, route_backend_for_chat_route,
};

/// Model preferences threaded into the research cascade builders
/// ([`crate::llm::cascade`]). The former 7-way provider-route resolver that
/// consumed the full struct was deleted 2026-07-16 (Axis GUI remediation F3);
/// the single exercised selection path is `vox_orchestrator::models::decide()`
/// + the reactive fallback chain.
#[derive(Debug, Clone)]
pub struct RouteResolutionInput {
    /// Model tag to use with local Mens/Ollama when that lane is offered.
    pub mens_chat_model: String,
    /// Preferred OpenRouter model when that lane is offered.
    pub openrouter_model: String,
}

impl Default for RouteResolutionInput {
    fn default() -> Self {
        Self {
            mens_chat_model: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliModel)
                .expose()
                .filter(|s: &&str| !s.trim().is_empty())
                .map(|s: &str| s.to_string())
                .unwrap_or_else(|| "default-model".to_string()),
            openrouter_model: vox_config::inference::openrouter_chat_model_preference(),
        }
    }
}

/// Stable `(provider_family, route_choice)` labels — derived from [`route_backend_for_chat_route`] + [`backend_telemetry_labels`].
#[must_use]
pub fn route_telemetry_labels(route: &ChatProviderRouteKind) -> (&'static str, &'static str) {
    backend_telemetry_labels(route_backend_for_chat_route(route))
}

/// Convert a route into [`LlmConfig`] for [`crate::llm::llm_chat`].
#[must_use]
pub fn chat_route_to_llm_config(route: &ChatProviderRouteKind) -> LlmConfig {
    match route {
        ChatProviderRouteKind::ManualOpenAiCompatible {
            base_url,
            model,
            bearer,
        } => LlmConfig {
            provider: "openai_compatible".to_string(),
            model: model.clone(),
            cost_per_1k: None,
            base_url: Some(base_url.clone()),
            api_key: bearer.clone(),
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
        },
        ChatProviderRouteKind::PopuliLocal { base_url, model }
        | ChatProviderRouteKind::PopuliMesh { base_url, model } => {
            let base = base_url.trim_end_matches('/');
            LlmConfig {
                provider: "ollama".to_string(),
                model: model.clone(),
                cost_per_1k: None,
                base_url: Some(format!("{base}/v1/chat/completions")),
                api_key: None,
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
        ChatProviderRouteKind::HuggingFaceRouter(ep) => LlmConfig {
            provider: "hf_router".to_string(),
            model: ep.model.clone(),
            cost_per_1k: None,
            base_url: Some(ep.chat_completions_url.clone()),
            api_key: ep.bearer_token.clone(),
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
        },
        ChatProviderRouteKind::HuggingFaceDedicated(ep) => LlmConfig {
            provider: "hf_endpoint".to_string(),
            model: ep.model.clone(),
            cost_per_1k: None,
            base_url: Some(ep.chat_completions_url.clone()),
            api_key: ep.bearer_token.clone(),
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
        },
        ChatProviderRouteKind::OpenRouter { model } => LlmConfig::openrouter(model.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_env;

    #[test]
    fn llm_config_ollama_chat_url_trimmed() {
        let c = chat_route_to_llm_config(&ChatProviderRouteKind::PopuliLocal {
            base_url: "http://127.0.0.1:11434/".to_string(),
            model: "llama3.2".to_string(),
        });
        assert_eq!(c.provider, "ollama");
        assert_eq!(
            c.base_url.as_deref(),
            Some("http://127.0.0.1:11434/v1/chat/completions")
        );
    }

    #[test]
    fn llm_config_hf_router_matches_inference_env() {
        let ep = inference_env::resolve_huggingface_router("org/model");
        let c = chat_route_to_llm_config(&ChatProviderRouteKind::HuggingFaceRouter(ep.clone()));
        assert_eq!(c.provider, "hf_router");
        assert_eq!(c.model, ep.model);
        assert_eq!(
            c.base_url.as_deref(),
            Some(ep.chat_completions_url.as_str())
        );
    }

    #[test]
    fn telemetry_labels_openrouter_variant() {
        let r = ChatProviderRouteKind::OpenRouter {
            model: vox_config::OPENROUTER_AUTO.to_string(),
        };
        assert_eq!(route_telemetry_labels(&r), ("openrouter", "openrouter"));
        assert_eq!(
            route_backend_for_chat_route(&r),
            ChatRouteBackend::OpenRouter
        );
    }

    #[test]
    fn route_backend_manual_gemini_url_is_gemini_direct() {
        let r = ChatProviderRouteKind::ManualOpenAiCompatible {
            base_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
                .into(),
            model: "gemini-2.0-flash".into(),
            bearer: None,
        };
        assert_eq!(
            route_backend_for_chat_route(&r),
            ChatRouteBackend::GeminiDirect
        );
        assert_eq!(route_telemetry_labels(&r), ("google", "direct"));
    }

    #[test]
    fn route_backend_manual_openai_compatible_is_cascade() {
        let r = ChatProviderRouteKind::ManualOpenAiCompatible {
            base_url: "https://api.example/v1/chat/completions".into(),
            model: "x".into(),
            bearer: None,
        };
        assert_eq!(
            route_backend_for_chat_route(&r),
            ChatRouteBackend::CascadeFallback
        );
        assert_eq!(route_telemetry_labels(&r), ("custom", "cascade"));
    }

    #[test]
    fn route_backend_populi_is_ollama() {
        let r = ChatProviderRouteKind::PopuliLocal {
            base_url: "http://127.0.0.1:11434".into(),
            model: "llama3.2".into(),
        };
        assert_eq!(route_backend_for_chat_route(&r), ChatRouteBackend::Ollama);
    }
}
