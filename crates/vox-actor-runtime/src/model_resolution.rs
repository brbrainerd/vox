//! Single **policy-shaped** resolver: manual → Mens (GPU-prefer) → HF dedicated → HF router → OpenRouter → local Mens → bootstrap.
//!
//! Maps to [`crate::llm::LlmConfig`] for OpenAI-compatible HTTP chat only (including Ollama `/v1/chat/completions`).
//!
//! ## Backend lane alignment (orchestrator / MCP)
//!
//! `ChatRouteBackend` (defined in `vox-orchestrator-types`) mirrors
//! `vox_orchestrator::models::ModelRouteBackend` semantics for telemetry and cross-surface dashboards.
//! `vox-actor-runtime` does **not** depend on `vox-orchestrator` (avoids cycles); keep the
//! two enums logically in sync with `vox_orchestrator::models::route_backend_for_model` for registry-backed models.
//! Chat-only routes add extra shapes (HF router/dedicated, manual OpenAI-compatible); those map to
//! `ChatRouteBackend::CascadeFallback` unless the manual URL is Google Generative Language API (→ `ChatRouteBackend::GeminiDirect`).

use crate::inference_env::{self, PopuliCapabilitySnapshot};
use crate::llm::LlmConfig;
use crate::route_capability_policy::RouteCapabilityPolicySnapshot;
pub use vox_orchestrator_types::{
    ChatProviderRouteKind, ChatRouteBackend, backend_telemetry_labels, route_backend_for_chat_route,
};

/// Inputs for [`resolve_chat_provider_route`].
#[derive(Debug, Clone)]
pub struct RouteResolutionInput {
    /// When set, wins over automatic policy (interpreted as OpenRouter-style id if no manual base URL).
    pub manual_model: Option<String>,
    /// Full OpenAI-compatible chat URL when bypassing automatic discovery (`…/v1/chat/completions`).
    pub manual_base_url: Option<String>,
    /// Optional bearer token for the manual endpoint (otherwise unauthenticated).
    pub manual_bearer: Option<String>,
    /// When true, prefer local Mens only if probe reports GPU-capable runtime.
    pub prefer_populi_when_gpu: bool,
    /// Latest [`PopuliCapabilitySnapshot`] from [`inference_env::probe_populi_capabilities`], if any.
    pub populi_probe: Option<PopuliCapabilitySnapshot>,
    /// Model tag to use with local Mens/Ollama when that route wins.
    pub mens_chat_model: String,
    /// Pinned Inference Endpoint chat URL (`HF_DEDICATED_CHAT_URL` via [`vox_config::inference`]).
    pub hf_dedicated_chat_url: Option<String>,
    /// Model id for the dedicated endpoint (`HF_DEDICATED_CHAT_MODEL`).
    pub hf_dedicated_chat_model: Option<String>,
    /// Preferred HF Inference Providers router model id when a token is present.
    pub hf_router_model: Option<String>,
    /// Preferred OpenRouter model when that lane wins.
    pub openrouter_model: String,
}

impl Default for RouteResolutionInput {
    fn default() -> Self {
        Self {
            manual_model: None,
            manual_base_url: None,
            manual_bearer: None,
            prefer_populi_when_gpu: true,
            populi_probe: None,
            mens_chat_model: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliModel)
                .expose()
                .filter(|s: &&str| !s.trim().is_empty())
                .map(|s: &str| s.to_string())
                .unwrap_or_else(|| "default-model".to_string()),
            hf_dedicated_chat_url: vox_config::inference::hf_dedicated_chat_completions_url(),
            hf_dedicated_chat_model: vox_config::inference::hf_dedicated_chat_model(),
            hf_router_model: vox_config::inference::hf_chat_model_preference(),
            openrouter_model: vox_config::inference::openrouter_chat_model_preference(),
        }
    }
}

fn populi_model_plausible(snapshot: &PopuliCapabilitySnapshot, model: &str) -> bool {
    if model == "default-model" {
        return true;
    }
    snapshot.model_names.iter().any(|n| n == model)
}

/// Stable `(provider_family, route_choice)` labels — derived from [`route_backend_for_chat_route`] + [`backend_telemetry_labels`].
#[must_use]
pub fn route_telemetry_labels(route: &ChatProviderRouteKind) -> (&'static str, &'static str) {
    backend_telemetry_labels(route_backend_for_chat_route(route))
}

fn resolve_chat_provider_route_impl(
    input: &RouteResolutionInput,
    hf_token_present: bool,
) -> ChatProviderRouteKind {
    let policy = RouteCapabilityPolicySnapshot::from_env();
    if let Some(ref m) = input.manual_model {
        if let Some(ref base) = input.manual_base_url {
            if !policy.allow_net {
                tracing::warn!(
                    target: "vox_orchestrator::model_route",
                    event = "route_policy_denied",
                    route = "manual_openai_compatible",
                    reason = "allow_net=false",
                    "routing: manual endpoint denied by capability policy"
                );
                return ChatProviderRouteKind::OpenRouter {
                    model: vox_config::OPENROUTER_AUTO.to_string(),
                };
            }
            return ChatProviderRouteKind::ManualOpenAiCompatible {
                base_url: base.clone(),
                model: m.clone(),
                bearer: input.manual_bearer.clone(),
            };
        }
        if !policy.allow_provider_network || !policy.allow_net {
            tracing::warn!(
                target: "vox_orchestrator::model_route",
                event = "route_policy_denied",
                route = "openrouter_manual_model",
                reason = "provider/network disabled",
                "routing: manual OpenRouter model denied by capability policy"
            );
            return ChatProviderRouteKind::OpenRouter {
                model: vox_config::OPENROUTER_AUTO.to_string(),
            };
        }
        return ChatProviderRouteKind::OpenRouter { model: m.clone() };
    }

    if input.prefer_populi_when_gpu && policy.allow_local_model_http {
        if let Some(ref snap) = input.populi_probe {
            if snap.reachable
                && snap.gpu_capable == Some(true)
                && populi_model_plausible(snap, &input.mens_chat_model)
            {
                tracing::info!(
                    target: "vox_orchestrator::model_route",
                    event = "route_resolution",
                    choice = "populi_gpu",
                    model = %input.mens_chat_model,
                    "routing: Mens (GPU-prefer)"
                );
                return ChatProviderRouteKind::PopuliLocal {
                    base_url: snap.base_url.clone(),
                    model: input.mens_chat_model.clone(),
                };
            }
        }
    }

    if hf_token_present && policy.allow_provider_network && policy.allow_net {
        if let (Some(url), Some(mid)) =
            (&input.hf_dedicated_chat_url, &input.hf_dedicated_chat_model)
        {
            tracing::info!(
                target: "vox_orchestrator::model_route",
                event = "route_resolution",
                choice = "huggingface_dedicated",
                model = %mid,
                "routing: Hugging Face dedicated endpoint"
            );
            return ChatProviderRouteKind::HuggingFaceDedicated(
                inference_env::resolve_huggingface_dedicated(url.clone(), mid.clone()),
            );
        }
    }

    if hf_token_present && policy.allow_provider_network && policy.allow_net {
        if let Some(ref mid) = input.hf_router_model {
            tracing::info!(
                target: "vox_orchestrator::model_route",
                event = "route_resolution",
                choice = "huggingface_router",
                model = %mid,
                "routing: Hugging Face router"
            );
            return ChatProviderRouteKind::HuggingFaceRouter(
                inference_env::resolve_huggingface_router(mid.clone()),
            );
        }
    }

    if policy.allow_provider_network
        && policy.allow_net
        && vox_config::inference::openrouter_api_key().is_some()
    {
        tracing::info!(
            target: "vox_orchestrator::model_route",
            event = "route_resolution",
            choice = "openrouter",
            model = %input.openrouter_model,
            "routing: OpenRouter"
        );
        return ChatProviderRouteKind::OpenRouter {
            model: input.openrouter_model.clone(),
        };
    }

    if policy.allow_local_model_http
        && let Some(ref snap) = input.populi_probe
    {
        if snap.reachable && populi_model_plausible(snap, &input.mens_chat_model) {
            tracing::info!(
                target: "vox_orchestrator::model_route",
                event = "route_resolution",
                choice = "populi_any",
                model = %input.mens_chat_model,
                "routing: Mens (reachable)"
            );
            return ChatProviderRouteKind::PopuliLocal {
                base_url: snap.base_url.clone(),
                model: input.mens_chat_model.clone(),
            };
        }
    }

    tracing::info!(
        target: "vox_orchestrator::model_route",
        event = "route_resolution",
        choice = "openrouter_bootstrap",
        model = %vox_config::OPENROUTER_AUTO,
        "routing: OpenRouter bootstrap (no keys / no local)"
    );
    ChatProviderRouteKind::OpenRouter {
        model: vox_config::OPENROUTER_AUTO.to_string(),
    }
}

/// Apply SSOT precedence from the routing plan (manual → GPU Mens → HF dedicated → HF router → OpenRouter → any Mens → OpenRouter auto).
#[must_use]
pub fn resolve_chat_provider_route(input: &RouteResolutionInput) -> ChatProviderRouteKind {
    resolve_chat_provider_route_impl(input, inference_env::huggingface_hub_token().is_some())
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

    #[test]
    fn manual_wins() {
        let snap = PopuliCapabilitySnapshot {
            base_url: "http://localhost:11434".to_string(),
            reachable: true,
            model_names: vec!["default-model".to_string()],
            gpu_capable: Some(true),
            notes: String::new(),
        };
        let r = resolve_chat_provider_route(&RouteResolutionInput {
            manual_model: Some("x/y".to_string()),
            manual_base_url: Some("https://api.example/v1/chat/completions".to_string()),
            manual_bearer: Some("tok".to_string()),
            prefer_populi_when_gpu: true,
            populi_probe: Some(snap),
            mens_chat_model: "default-model".into(),
            hf_dedicated_chat_url: None,
            hf_dedicated_chat_model: None,
            hf_router_model: Some("hf/model".to_string()),
            openrouter_model: "openrouter/auto".into(),
        });
        assert_eq!(
            r,
            ChatProviderRouteKind::ManualOpenAiCompatible {
                base_url: "https://api.example/v1/chat/completions".to_string(),
                model: "x/y".to_string(),
                bearer: Some("tok".to_string()),
            }
        );
    }

    #[test]
    fn openrouter_id_without_base() {
        let r = resolve_chat_provider_route(&RouteResolutionInput {
            manual_model: Some("anthropic/claude".to_string()),
            manual_base_url: None,
            manual_bearer: None,
            prefer_populi_when_gpu: false,
            populi_probe: None,
            mens_chat_model: "m".into(),
            hf_dedicated_chat_url: None,
            hf_dedicated_chat_model: None,
            hf_router_model: None,
            openrouter_model: "openrouter/auto".into(),
        });
        assert_eq!(
            r,
            ChatProviderRouteKind::OpenRouter {
                model: "anthropic/claude".to_string()
            }
        );
    }

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
    fn dedicated_endpoint_before_shared_router_when_token_present() {
        let r = resolve_chat_provider_route_impl(
            &RouteResolutionInput {
                manual_model: None,
                manual_base_url: None,
                manual_bearer: None,
                prefer_populi_when_gpu: false,
                populi_probe: None,
                mens_chat_model: "m".into(),
                hf_dedicated_chat_url: Some("https://ep.example/v1/chat/completions".into()),
                hf_dedicated_chat_model: Some("deployed-model".into()),
                hf_router_model: Some("hf/router-model".into()),
                openrouter_model: "openrouter/auto".into(),
            },
            true,
        );
        assert_eq!(route_telemetry_labels(&r), ("custom", "cascade"));
        assert_eq!(
            route_backend_for_chat_route(&r),
            ChatRouteBackend::CascadeFallback
        );
        match r {
            ChatProviderRouteKind::HuggingFaceDedicated(ep) => {
                assert_eq!(ep.model, "deployed-model");
                assert_eq!(
                    ep.chat_completions_url,
                    "https://ep.example/v1/chat/completions"
                );
            }
            other => panic!("expected dedicated route, got {other:?}"),
        }
    }

    #[test]
    fn router_when_no_dedicated_fields() {
        let r = resolve_chat_provider_route_impl(
            &RouteResolutionInput {
                manual_model: None,
                manual_base_url: None,
                manual_bearer: None,
                prefer_populi_when_gpu: false,
                populi_probe: None,
                mens_chat_model: "m".into(),
                hf_dedicated_chat_url: None,
                hf_dedicated_chat_model: None,
                hf_router_model: Some("org/hf-only".into()),
                openrouter_model: "openrouter/auto".into(),
            },
            true,
        );
        match r {
            ChatProviderRouteKind::HuggingFaceRouter(ep) => assert_eq!(ep.model, "org/hf-only"),
            other => panic!("expected router, got {other:?}"),
        }
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
