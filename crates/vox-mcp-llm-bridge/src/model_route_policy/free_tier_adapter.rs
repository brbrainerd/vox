//! Adapter that routes latency-critical free-tier selection through the
//! `vox-research-shim` [`FreeTierRouter`] instead of the registry's context-size
//! sort.
//!
//! The router applies hard capability constraints (vision / JSON / FIM) and a
//! `ModelTier::Fast` latency bonus that the legacy `best_free_for_with_filter`
//! path ignored entirely. This is a thin, pure adapter over a slice of free
//! models so it is unit-testable without a live `ModelRegistry` (no network,
//! no config/cache I/O). It is model-agnostic: it keys only on the
//! `ProviderType` enum + `is_free` catalog flag, never a vendor hostname.

use vox_orchestrator::models::{Capability, ModelSpec, ModelTier};
use vox_research_shim::selection::{FreeTierRouteRequest, FreeTierRouter};

use super::types::McpChatModelResolution;

/// Pick the best latency-critical free model from `free_models`, honoring the
/// caller's capability requirements and an `accept` gate (local-Ollama /
/// routing-profile rules the router itself does not know about).
///
/// Returns the top [`FreeTierRouter`] candidate that also satisfies `accept`,
/// or `None` when no free model qualifies (letting the caller fall through to
/// its `cheapest_free` fallback).
pub fn route_free_tier_latency(
    free_models: &[ModelSpec],
    res: &McpChatModelResolution,
    required_capabilities: &[Capability],
    accept: impl Fn(&ModelSpec) -> bool,
) -> Option<(ModelSpec, &'static str)> {
    let req = FreeTierRouteRequest {
        task: res.task_category,
        context_tokens: 0,
        requires_vision: required_capabilities.contains(&Capability::SupportsVision),
        requires_structured_output: required_capabilities.contains(&Capability::SupportsJson),
        requires_fill_in_middle: res.free_tier_fill_in_middle,
        latency_critical: res.free_tier_latency_critical,
        max_candidates: 8,
    };

    // Carry each candidate's human-readable rationale alongside the spec so the
    // caller can surface WHY a free model was chosen (tracing / provider status).
    let mut candidates: Vec<(ModelSpec, &'static str)> = FreeTierRouter::new()
        .route(&req, free_models)
        .into_iter()
        .filter(|c| accept(&c.model))
        .map(|c| (c.model, c.rationale))
        .collect();

    if res.free_tier_latency_critical {
        // Decisively prefer Fast-tier models for latency-critical work. The
        // router only adds a small additive bonus, which the base scorer can
        // swamp (e.g. a 1M-context Pro model out-scoring a Fast model). A
        // *stable* partition keeps the router's relative ranking as the
        // within-tier tiebreaker, so the best Fast candidate wins, falling
        // back to the router's best non-Fast pick when no Fast model qualifies.
        candidates.sort_by_key(|(m, _)| u8::from(m.capabilities.tier != ModelTier::Fast));
    }

    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::models::{
        ModelCapabilities, ModelSpec, ModelTier, PricingSource, ProviderType, StrengthTag,
    };
    use vox_orchestrator::types::TaskCategory;

    fn free_spec(
        id: &str,
        provider_type: ProviderType,
        tier: ModelTier,
        max_context: u64,
        vision: bool,
        json: bool,
    ) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: String::new(),
            provider: "test".to_string(),
            provider_type,
            max_tokens: max_context,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::OpenRouter,
            is_free: true,
            strengths: vec![StrengthTag::Codegen, StrengthTag::Generalist],
            capabilities: ModelCapabilities {
                supports_vision: vision,
                supports_json: json,
                max_context,
                tier,
                ..Default::default()
            },
            supported_parameters: vec![],
        }
    }

    fn latency_res() -> McpChatModelResolution {
        McpChatModelResolution {
            free_tier_latency_critical: true,
            task_category: TaskCategory::CodeGen,
            ..Default::default()
        }
    }

    #[test]
    fn latency_critical_prefers_fast_tier_not_largest_context() {
        // The legacy `best_free_for_with_filter` ranks by max_tokens and would
        // pick `big`; the FreeTierRouter's +5.0 Fast-tier latency bonus must win.
        let big = free_spec(
            "p/big",
            ProviderType::OpenRouter,
            ModelTier::Pro,
            1_000_000,
            false,
            false,
        );
        let fast = free_spec(
            "p/fast",
            ProviderType::Cerebras,
            ModelTier::Fast,
            8_000,
            false,
            false,
        );
        let models = vec![big, fast];

        let (model, rationale) = route_free_tier_latency(&models, &latency_res(), &[], |_| true)
            .expect("a free model should be routed");
        assert_eq!(model.id, "p/fast");
        assert_eq!(model.capabilities.tier, ModelTier::Fast);
        assert!(
            !rationale.is_empty(),
            "a human-readable rationale must accompany the pick"
        );
    }

    #[test]
    fn fim_hint_excludes_non_fim_providers() {
        let fast_nonfim = free_spec(
            "x/fast",
            ProviderType::Cerebras,
            ModelTier::Fast,
            32_000,
            false,
            false,
        );
        let mistral = free_spec(
            "mistral/codestral",
            ProviderType::Mistral,
            ModelTier::Pro,
            32_000,
            false,
            false,
        );
        let models = vec![fast_nonfim, mistral];

        let mut res = latency_res();
        res.free_tier_fill_in_middle = true;

        let (model, _rationale) = route_free_tier_latency(&models, &res, &[], |_| true)
            .expect("a FIM-capable free model should be routed");
        assert_eq!(model.provider_type, ProviderType::Mistral);
    }

    #[test]
    fn required_vision_filters_non_vision_and_keeps_vision() {
        let caps = vec![Capability::SupportsVision];
        let no_vision = free_spec(
            "a/novis",
            ProviderType::OpenRouter,
            ModelTier::Pro,
            64_000,
            false,
            true,
        );
        assert!(
            route_free_tier_latency(&[no_vision], &latency_res(), &caps, |_| true).is_none(),
            "a non-vision model must not satisfy a vision requirement"
        );

        let vision = free_spec(
            "a/vis",
            ProviderType::OpenRouter,
            ModelTier::Pro,
            64_000,
            true,
            false,
        );
        assert!(
            route_free_tier_latency(&[vision], &latency_res(), &caps, |_| true).is_some(),
            "a vision-capable free model must be routable under a vision requirement"
        );
    }

    #[test]
    fn accept_filter_excludes_blocked_local() {
        // Preserve the caller's local-Ollama / routing-profile gating: the
        // router may surface an Ollama free model, but `accept` rejects it.
        let ollama = free_spec(
            "ollama/x",
            ProviderType::Ollama,
            ModelTier::Local,
            8_000,
            false,
            false,
        );
        let got = route_free_tier_latency(&[ollama], &latency_res(), &[], |m| {
            m.provider_type != ProviderType::Ollama
        });
        assert!(got.is_none(), "accept gate must exclude blocked providers");
    }

    #[test]
    fn no_free_models_returns_none() {
        let got = route_free_tier_latency(&[], &latency_res(), &[], |_| true);
        assert!(got.is_none());
    }
}
