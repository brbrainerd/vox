//! Routing capability policy alignment with `VOX_ROUTE_*` and `contracts/orchestration/model-routing.v1.yaml`.
//!
//! Shared by MCP model resolution and CLI/model explain surfaces.

use crate::models::{ModelSpec, ProviderType};
use vox_actor_runtime::route_capability_policy::{
    RouteCapabilityPolicySnapshot, exclusion_reason_for_llm_lane,
};

#[inline]
fn is_local_http_provider(provider_type: &ProviderType) -> bool {
    matches!(
        provider_type,
        ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
    )
}

/// Returns a static denial code when [`ModelSpec`] cannot be used under the current route policy.
#[must_use]
pub fn route_policy_exclusion_reason(model: &ModelSpec) -> Option<&'static str> {
    let snap = RouteCapabilityPolicySnapshot::from_env();
    exclusion_reason_for_llm_lane(is_local_http_provider(&model.provider_type), &snap)
}

#[must_use]
pub fn route_policy_allows_model(model: &ModelSpec) -> bool {
    route_policy_exclusion_reason(model).is_none()
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn local_http_providers_match_only_local_lanes() {
        // True arm: each of the three local HTTP provider variants.
        assert!(is_local_http_provider(&ProviderType::Ollama));
        assert!(is_local_http_provider(&ProviderType::PopuliMesh));
        assert!(is_local_http_provider(&ProviderType::VoxLocal));

        // False arm: a remote/cloud provider variant is not a local HTTP lane.
        assert!(!is_local_http_provider(&ProviderType::Anthropic));
    }

    #[test]
    fn local_http_providers_classified_true_others_false() {
        assert!(is_local_http_provider(&ProviderType::Ollama));
        assert!(is_local_http_provider(&ProviderType::PopuliMesh));
        assert!(is_local_http_provider(&ProviderType::VoxLocal));
        assert!(!is_local_http_provider(&ProviderType::OpenRouter));
    }
}
