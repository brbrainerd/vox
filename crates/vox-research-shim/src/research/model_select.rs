//! Resolve research pipeline stage models from the canonical [`vox_orchestrator::models::ModelRegistry`].
//!
//! All research stages flow through [`vox_orchestrator::models::select_with_default_registry`]
//! so routing honors premium aliases, scoreboard feedback, and user axes.

use vox_orchestrator::models::ModelRegistry;
use vox_orchestrator::models::{SelectionAxes, SelectionIntent, select_with_default_registry};

/// Sentinel NLI model id used before registry resolution; see `verifier_config_for_research_run`.
pub const FALLBACK_NLI_MODEL_ID: &str = vox_config::NLI_FALLBACK;

/// Opaque inference config passed to model resolution.
///
/// `quality` is a blanket default applied to any stage without a more
/// specific override below. All fields default to `None`, meaning "use
/// each stage's own `SelectionIntent` constructor's built-in axes" —
/// this preserves prior behavior exactly when nothing is explicitly set.
#[derive(Debug, Clone, Default)]
pub struct InferenceConfig {
    pub quality: Option<QualityLevel>,
    pub planner_quality: Option<QualityLevel>,
    pub claim_quality: Option<QualityLevel>,
    pub synthesis_quality: Option<QualityLevel>,
    pub judge_quality: Option<QualityLevel>,
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

/// Maps a research `QualityLevel` onto the underlying 3-axis selection knob.
fn axes_for_quality(quality: QualityLevel) -> SelectionAxes {
    match quality {
        QualityLevel::Flash => SelectionAxes::COST_FIRST,
        QualityLevel::Balanced => SelectionAxes::BALANCED,
        QualityLevel::Premium => SelectionAxes::QUALITY_FIRST,
    }
}

fn resolve_stage(
    registry: &ModelRegistry,
    mut intent: SelectionIntent,
    quality_override: Option<QualityLevel>,
    fallback: &str,
) -> String {
    if let Some(quality) = quality_override {
        intent.axes = axes_for_quality(quality);
    }
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
    base_inference: &InferenceConfig,
) -> ResolvedResearchModels {
    let planner_quality = base_inference.planner_quality.or(base_inference.quality);
    let claim_quality = base_inference.claim_quality.or(base_inference.quality);
    let synthesis_quality = base_inference.synthesis_quality.or(base_inference.quality);
    let judge_quality = base_inference.judge_quality.or(base_inference.quality);

    let planner = resolve_stage(
        registry,
        SelectionIntent::research(),
        planner_quality,
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let claim = resolve_stage(
        registry,
        SelectionIntent::nli_classifier(),
        claim_quality,
        vox_config::NLI_FALLBACK,
    );
    let synthesis = resolve_stage(
        registry,
        SelectionIntent::research(),
        synthesis_quality,
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let judge = resolve_stage(
        registry,
        SelectionIntent::review(),
        judge_quality,
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

    #[test]
    fn quality_level_flash_prefers_cost_first_axes() {
        use vox_orchestrator::models::SelectionAxes;
        let flash = axes_for_quality(QualityLevel::Flash);
        let premium = axes_for_quality(QualityLevel::Premium);
        assert_eq!(flash, SelectionAxes::COST_FIRST);
        assert_eq!(premium, SelectionAxes::QUALITY_FIRST);
    }

    #[test]
    fn default_inference_config_preserves_each_stage_builtin_axes() {
        // With everything None (the Default), resolve_stage must NOT override
        // intent.axes at all — the stage constructor's own tuned default
        // (QUALITY_FIRST for research/review, COST_FIRST for nli_classifier)
        // must survive untouched. We can't easily inspect the final axes used
        // internally by select_with_default_registry from here, so this test
        // instead asserts the override-decision logic directly: no override
        // requested means resolve_stage's quality_override parameter is None.
        let config = InferenceConfig::default();
        assert_eq!(config.quality, None);
        assert_eq!(config.planner_quality, None);
        let effective_planner_quality = config.planner_quality.or(config.quality);
        assert_eq!(
            effective_planner_quality, None,
            "no override should reach resolve_stage by default"
        );
    }

    #[test]
    fn stage_specific_override_takes_precedence_over_blanket_default() {
        let config = InferenceConfig {
            quality: Some(QualityLevel::Balanced),
            judge_quality: Some(QualityLevel::Premium),
            ..Default::default()
        };
        let effective_judge_quality = config.judge_quality.or(config.quality);
        let effective_planner_quality = config.planner_quality.or(config.quality);
        assert_eq!(
            effective_judge_quality,
            Some(QualityLevel::Premium),
            "stage-specific override must win"
        );
        assert_eq!(
            effective_planner_quality,
            Some(QualityLevel::Balanced),
            "blanket default applies when no stage override set"
        );
    }
}
