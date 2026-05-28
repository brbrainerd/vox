//! Resolve research pipeline stage models from the canonical [`vox_orchestrator::models::ModelRegistry`].
//!
//! All research stages flow through [`vox_orchestrator::models::select_with_default_registry`]
//! so routing honors premium aliases, scoreboard feedback, and user axes.

use vox_orchestrator::models::{
    SelectionIntent, select_with_default_registry,
};
use vox_orchestrator::models::ModelRegistry;

/// Sentinel NLI model id used before registry resolution; see [`verifier_config_for_research_run`].
pub const FALLBACK_NLI_MODEL_ID: &str = vox_config::NLI_FALLBACK;

/// Opaque inference config passed to model resolution.
#[derive(Debug, Clone, Default)]
pub struct InferenceConfig {
    pub quality: QualityLevel,
}

/// Model quality level hint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QualityLevel {
    Flash,
    #[default]
    Balanced,
    Premium,
}

/// Registry-selected model IDs for each research LLM stage.
#[derive(Debug, Clone)]
pub struct ResolvedResearchModels {
    /// Query decomposition / subquery planning.
    pub planner_model: String,
    /// Claim extraction from text.
    pub claim_model: String,
    /// Final cited answer synthesis.
    pub synthesis_model: String,
    /// LLM-as-judge quality score.
    pub judge_model: String,
}

fn resolve_stage(
    registry: &ModelRegistry,
    intent: SelectionIntent,
    fallback: &str,
) -> String {
    if let Some(outcome) = select_with_default_registry(&intent) {
        return outcome.model_id;
    }
    registry
        .get(fallback)
        .map(|m| m.id.clone())
        .unwrap_or_else(|| fallback.to_string())
}

/// Select models for planner, claim extraction, synthesis, and judge stages.
#[must_use]
pub fn resolve_research_models(
    registry: &ModelRegistry,
    _base_inference: &InferenceConfig,
) -> ResolvedResearchModels {
    let planner = resolve_stage(
        registry,
        SelectionIntent::research(),
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let claim = resolve_stage(
        registry,
        SelectionIntent::nli_classifier(),
        vox_config::NLI_FALLBACK,
    );
    let synthesis = resolve_stage(
        registry,
        SelectionIntent::research(),
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let judge = resolve_stage(
        registry,
        SelectionIntent::review(),
        vox_config::REVIEW_PREMIUM_FALLBACK,
    );

    ResolvedResearchModels {
        planner_model: planner,
        claim_model: claim,
        synthesis_model: synthesis,
        judge_model: judge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_research_models_returns_non_empty_ids() {
        let registry = ModelRegistry::from_cache();
        let models = resolve_research_models(&registry, &InferenceConfig::default());
        assert!(!models.planner_model.is_empty());
        assert!(!models.claim_model.is_empty());
        assert!(!models.synthesis_model.is_empty());
        assert!(!models.judge_model.is_empty());
    }
}
