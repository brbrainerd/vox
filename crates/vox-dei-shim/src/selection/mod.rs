//! Model selection: canonical task→strength mapping, pluggable scoring, and virtual models.
//!
//! Single source of truth for task-strength mapping and scoring weights.
//! Virtual models (e.g. openrouter/auto) are defined here and merged when applicable.

mod free_tier;
mod scorer;
mod task_routing;
#[cfg(test)]
mod tests;
mod virtual_models;
mod weights;

pub use free_tier::{FreeTierRouteRequest, FreeTierRouter, RouteCandidate};
pub use scorer::{ModelScorer, ScoreParams, select_best_model};
pub use task_routing::{
    config_to_routing_profile, model_matches_task, primary_strength, task_and_flags_to_profile,
    task_strengths,
};
pub use virtual_models::{openrouter_auto_model, openrouter_free_model, virtual_models};
pub use vox_orchestrator::registry_model_resolve::{
    RegistryModelResolutionParams, infer_prompt_capability_hints,
    resolve_model_with_registry_fallbacks,
};
pub use weights::ScoringWeights;
