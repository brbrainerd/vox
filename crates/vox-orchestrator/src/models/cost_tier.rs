//! Cost-tier classification for model-selection tracking (chat harness continuous eval design,
//! 2026-08-02). Used by `vox harness eval --live` to record whether a given selection was
//! Free/Cheap/Premium, so cost-appropriateness drift can be tracked over time in
//! `model_selection_event` (see `crates/vox-db/src/harness_eval.rs`).

use super::ModelSpec;

/// The threshold (USD per 1k tokens, blended) below which a non-free model counts as "Cheap"
/// rather than "Premium". Chosen as a round number comfortably below typical premium-tier
/// pricing (e.g. Claude Opus/GPT-4-class models are commonly $5-15/1k) and comfortably above
/// typical budget cloud-model pricing (commonly $0.0001-0.001/1k) — not derived from an existing
/// constant elsewhere in this codebase (none exists as of this plan; grep confirmed no
/// `CHEAP`/cost-tier threshold precedent in `models/scoring.rs`).
pub const CHEAP_COST_PER_1K_USD: f64 = 0.002;

/// Cost tier of a selected model, for cost-appropriateness drift tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostTier {
    Free,
    Cheap,
    Premium,
}

impl CostTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            CostTier::Free => "free",
            CostTier::Cheap => "cheap",
            CostTier::Premium => "premium",
        }
    }
}

/// Classify a model's cost tier from its spec. Uses the blended average of input/output
/// cost_per_1k (falling back to `cost_per_1k` if input/output aren't both set) since a model's
/// nominal `is_free`/`cost_per_1k` fields are the same ones the rest of the selection pipeline
/// already treats as authoritative (see `models::scoring::auto_score_model`).
pub fn cost_tier_for(model: &ModelSpec) -> CostTier {
    if model.is_free {
        return CostTier::Free;
    }
    let blended = if model.cost_per_1k_input > 0.0 || model.cost_per_1k_output > 0.0 {
        (model.cost_per_1k_input + model.cost_per_1k_output) / 2.0
    } else {
        model.cost_per_1k
    };
    if blended <= CHEAP_COST_PER_1K_USD {
        CostTier::Cheap
    } else {
        CostTier::Premium
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProviderType;

    fn spec(is_free: bool, cost_per_1k: f64) -> ModelSpec {
        ModelSpec {
            id: "test-model".into(),
            canonical_slug: "test-model".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 8192,
            cost_per_1k,
            cost_per_1k_input: cost_per_1k,
            cost_per_1k_output: cost_per_1k,
            is_free,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn free_model_is_always_free_tier_regardless_of_cost_fields() {
        assert_eq!(cost_tier_for(&spec(true, 0.19)), CostTier::Free);
    }

    #[test]
    fn cheap_model_below_threshold_is_cheap_tier() {
        assert_eq!(cost_tier_for(&spec(false, 0.001)), CostTier::Cheap);
    }

    #[test]
    fn expensive_model_above_threshold_is_premium_tier() {
        assert_eq!(cost_tier_for(&spec(false, 0.19)), CostTier::Premium);
    }

    #[test]
    fn boundary_at_exact_threshold_counts_as_cheap() {
        assert_eq!(
            cost_tier_for(&spec(false, CHEAP_COST_PER_1K_USD)),
            CostTier::Cheap
        );
    }
}
