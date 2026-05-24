//! Resolve research pipeline stage models from the canonical [`vox_orchestrator::models::ModelRegistry`].
//!
//! Phase 0a: returns static fallback model IDs. Phase 1+ wires this to the
//! full `InferenceConfig`/`selection` machinery once those modules are activated.
//!
//! PHASE_0a_STUB: all picks return config-based fallbacks; no dynamic registry resolution.

// Re-export from the single source of truth in vox-config.
pub(crate) use vox_config::NLI_FALLBACK as FALLBACK_NLI_MODEL_ID;
pub(crate) use vox_config::RESEARCH_FLASH_FALLBACK as FALLBACK_RESEARCH_FLASH_MODEL_ID;
pub(crate) use vox_config::REVIEW_PREMIUM_FALLBACK as FALLBACK_REVIEW_PREMIUM_MODEL_ID;

/// Opaque inference config passed to model resolution.
///
/// Phase 0a STUB: carries only the model override strings used by `pipeline.rs`.
/// Phase 1 replaces this with the full `vox_orchestrator::mode::InferenceConfig`.
#[derive(Debug, Clone, Default)]
pub struct InferenceConfig {
    // PHASE_0a_STUB: placeholder struct. Phase 1 merges with vox_orchestrator::mode::InferenceConfig.
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

/// Select models for planner, claim extraction, synthesis, and judge stages.
///
/// Phase 0a STUB: returns static fallback model IDs from vox-config constants.
/// Phase 1 replaces with live registry resolution via `crate::selection`.
///
/// `_base_inference` is accepted for API compatibility with future live resolution.
#[must_use]
pub fn resolve_research_models(
    _registry: &vox_orchestrator::models::ModelRegistry,
    _base_inference: &InferenceConfig,
) -> ResolvedResearchModels {
    // PHASE_0a_STUB: static fallbacks. Phase 1 wires to resolve_model_with_registry_fallbacks.
    ResolvedResearchModels {
        planner_model: FALLBACK_RESEARCH_FLASH_MODEL_ID.to_string(),
        claim_model: FALLBACK_NLI_MODEL_ID.to_string(),
        synthesis_model: FALLBACK_RESEARCH_FLASH_MODEL_ID.to_string(),
        judge_model: FALLBACK_REVIEW_PREMIUM_MODEL_ID.to_string(),
    }
}
