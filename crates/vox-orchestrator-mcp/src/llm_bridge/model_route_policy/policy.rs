use vox_config::{
    GeminiRoutePolicy, gemini_route_targets_from_env, inference_profile_allows_local_ollama_http,
};
use vox_orchestrator::models::{ModelRegistry, ModelSpec, ProviderType};

use super::types::McpChatModelResolution;

pub(super) fn enforce_free_tier_if_needed(
    registry: &ModelRegistry,
    res: &McpChatModelResolution,
    spec: ModelSpec,
) -> Result<ModelSpec, String> {
    if !res.enforce_free_tier_only || spec.is_free {
        return Ok(spec);
    }
    let task = res.task_category;
    registry
        .best_free_for_with_filter(task, mcp_local_model_allowed)
        .or_else(|| registry.cheapest_free_with_filter(mcp_local_model_allowed))
        .ok_or_else(|| {
            "No free-tier model available (enforce_free_tier_only) after vox_populi::inference_PROFILE rules; clear sticky override, allow desktop_ollama/lan_gateway for Ollama, or add a non-Ollama free model in models.toml".to_string()
        })
}

#[must_use]
pub(super) fn mcp_local_model_allowed(m: &ModelSpec) -> bool {
    let is_local = matches!(
        m.provider_type,
        ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
    );
    !is_local || inference_profile_allows_local_ollama_http()
}

#[must_use]
pub(super) fn apply_gemini_policy(
    registry: &ModelRegistry,
    chosen: ModelSpec,
    sticky_override: bool,
) -> ModelSpec {
    if sticky_override {
        return chosen;
    }
    let targets = gemini_route_targets_from_env();
    let is_gemini = chosen.id.to_ascii_lowercase().contains("gemini");
    if !is_gemini {
        return chosen;
    }
    match GeminiRoutePolicy::from_env() {
        GeminiRoutePolicy::RegistryDefault => chosen,
        GeminiRoutePolicy::OpenRouterFirst => {
            if !matches!(chosen.provider_type, ProviderType::OpenRouter)
                && vox_config::openrouter_api_key().is_some()
            {
                registry.get(&targets.openrouter_model).unwrap_or(chosen)
            } else {
                chosen
            }
        }
        GeminiRoutePolicy::GoogleDirectOnly => {
            if !matches!(chosen.provider_type, ProviderType::GoogleDirect) {
                registry.get(&targets.google_direct_model).unwrap_or(chosen)
            } else {
                chosen
            }
        }
    }
}
