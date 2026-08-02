//! Routing capability policy alignment with `VOX_ROUTE_*` and `contracts/orchestration/model-routing.v1.yaml`.
//!
//! Shared by MCP model resolution and CLI/model explain surfaces.

use crate::models::{ModelSpec, ProviderType};
use vox_actor_runtime::route_capability_policy::{
    RouteCapabilityPolicySnapshot, exclusion_reason_for_llm_lane,
};

/// True for providers that run without a paid cloud credential (local inference over HTTP).
/// Canonical home for this predicate — reuse it rather than re-deriving the variant list
/// elsewhere (e.g. `vox-cli`'s `model list` command), so a future local provider only needs
/// updating in one place.
#[inline]
#[must_use]
pub fn is_local_http_provider(provider_type: &ProviderType) -> bool {
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

/// Pure privacy decision core, relocated from `vox-orchestrator-mcp`'s
/// `llm_bridge::local_health` so it's reachable from this crate's own
/// model-selection filter chain (`models::registry::best_for_internal`),
/// closing a real coverage gap: the mcp-crate-only original was reachable
/// only from the synchronous chat path, never from `AiTaskProcessor` (the
/// pipeline handling the bulk of Vox's real agentic work), since
/// `vox-orchestrator` cannot depend on `vox-orchestrator-mcp`.
///
/// A hard filter, not a ranking hint: when `local_only`, non-local-provider
/// models are excluded from candidates entirely (see [`is_local_http_provider`]
/// for the local/cloud split reused here), never merely deprioritized.
#[must_use]
pub fn privacy_allows_model_for_mode(m: &ModelSpec, local_only: bool) -> bool {
    if local_only {
        return is_local_http_provider(&m.provider_type);
    }
    true
}

/// Test-only seam for `VOX_INFERENCE_PRIVACY`, mirroring the pattern used by
/// `vox-orchestrator-mcp`'s `TEST_HEALTH_OVERRIDE` — avoids mutating the real
/// process env (which is racy under parallel `cargo test`) while still
/// exercising the real decision logic in [`inference_privacy_local_only_from_env`].
///
/// Gated on `cfg(test)` OR the `test-support` feature (never on by default):
/// `vox-orchestrator-mcp`'s test suite (`llm_bridge::local_health`) needs to
/// drive this override too, and that crate compiles `vox-orchestrator` as an
/// ordinary (non-`cfg(test)`) dev-dependency with `test-support` enabled — a
/// bare `#[cfg(test)]` gate here would compile out for that caller entirely,
/// silently breaking the seam, while a plain `pub fn` would leak a
/// privacy-bypass knob into every release build's public API. The static
/// itself stays always-compiled (its own `Mutex` is a negligible cost) so the
/// getter (`inference_privacy_local_only_from_env`) doesn't need its own gate.
static TEST_PRIVACY_OVERRIDE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(any(test, feature = "test-support"))]
pub fn set_test_privacy_override(v: Option<&str>) {
    *TEST_PRIVACY_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = v.map(str::to_string);
}

/// Reads `VOX_INFERENCE_PRIVACY` (`any` [default] | `local_only`), or the
/// test-only override set via [`set_test_privacy_override`], and returns
/// whether `local_only` is in effect.
#[must_use]
pub fn inference_privacy_local_only_from_env() -> bool {
    if let Some(v) = TEST_PRIVACY_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return v.trim().eq_ignore_ascii_case("local_only");
    }
    std::env::var("VOX_INFERENCE_PRIVACY")
        .map(|v| v.trim().eq_ignore_ascii_case("local_only"))
        .unwrap_or(false)
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

    fn priv_spec(id: &str, provider_type: ProviderType) -> ModelSpec {
        use crate::models::{ModelCapabilities, spec::PricingSource};
        ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type,
            max_tokens: 8_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn privacy_allows_model_for_mode_blocks_cloud_under_local_only_and_allows_local() {
        let cloud = priv_spec("cloud-model", ProviderType::OpenRouter);
        let local = priv_spec("local-model", ProviderType::Ollama);
        assert!(!privacy_allows_model_for_mode(&cloud, true));
        assert!(privacy_allows_model_for_mode(&local, true));
        assert!(privacy_allows_model_for_mode(&cloud, false));
    }
}
