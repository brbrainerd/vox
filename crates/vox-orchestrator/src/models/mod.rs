pub mod admission;
pub mod autonomic;
pub mod generated;
pub mod key_guard;
pub mod policy;
mod registry;
pub mod routing_table;
pub mod scoring;
pub mod select;
pub mod spec;
#[cfg(test)]
mod tests;

pub use generated::{
    Capability, CapabilityFlags, ModelTier, PromptIntent, StrengthTag, TaskCategory,
    infer_capabilities, infer_prompt_intents, intent_required_capabilities,
};
pub use policy::{
    FallbackCondition, PolicyContext, SelectionAxisKind, SelectionPolicy, SelectionStep,
    active_policy, install_active_policy, policy_for_profile, resolve_policy,
};
pub use registry::{ModelRegistry, ModelScore};
pub use select::{
    CandidateScope, ModelSelectionDecision, ModelSelectionRequest, ScoreBreakdown, SelectionAxes,
    SelectionIntent, SelectionOutcome, SelectionReason, decide, select, select_with_default_registry,
    select_with_policy,
};
pub use spec::{
    ModelCapabilities, ModelConfig, ModelRouteBackend, ModelSpec, PricingSource, ProviderType,
    route_backend_for_model, task_category_premium_key, task_category_strength,
};
