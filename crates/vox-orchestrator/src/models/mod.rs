pub mod admission;
pub mod autonomic;
pub mod cost_tier;
pub mod discovery_pipeline;
pub mod generated;
pub mod key_guard;
mod pareto;
pub mod policy;
pub mod prompt_profiles;
mod registry;
pub mod routing_table;
pub mod scoring;
pub mod select;
pub mod spec;
#[cfg(test)]
mod tests;
pub mod vram;

pub use cost_tier::{
    CHEAP_COST_PER_1K_USD, CostTier, blended_cost_per_1k, cost_tier_for, cost_tier_for_blended,
};
pub use generated::{
    Capability, CapabilityFlags, ModelTier, PromptIntent, StrengthTag, TaskCategory,
    infer_capabilities, infer_prompt_intents, intent_required_capabilities,
};
pub use pareto::{ParetoPoint, is_observed, pareto_frontier, pareto_point_for};
pub use policy::{
    FallbackCondition, PolicyContext, SelectionAxisKind, SelectionPolicy, SelectionStep,
    active_policy, install_active_policy, policy_for_profile, resolve_policy,
};
#[cfg(feature = "runtime")]
pub use registry::llm_config_for_spec;
pub use registry::{
    MIN_CALLS_FOR_CONFIDENT_RANK, ModelRegistry, ModelScore, wilson_score_interval,
};
pub use scoring::install_base_routing_priority;
pub use select::{
    CandidateScope, ModelSelectionDecision, ModelSelectionRequest, ScoreBreakdown, SelectionAxes,
    SelectionIntent, SelectionOutcome, SelectionReason, decide, select,
    select_with_default_registry, select_with_policy,
};
pub use spec::{
    ModelCapabilities, ModelConfig, ModelRouteBackend, ModelSpec, PricingSource, ProviderType,
    route_backend_for_model, task_category_premium_key, task_category_strength,
};
pub use vram::{VramFit, estimate_vram_fit, free_vram_mb_hint, refresh_free_vram_hint_from_nvml};
