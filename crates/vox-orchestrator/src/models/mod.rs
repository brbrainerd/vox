pub mod generated;
pub mod key_guard;
mod registry;
pub mod routing_table;
pub mod scoring;
pub mod spec;
pub mod admission;
pub mod autonomic;
pub mod select;
#[cfg(test)]
mod tests;

pub use generated::{
    Capability, CapabilityFlags, ModelTier, PromptIntent, StrengthTag, TaskCategory,
    infer_capabilities, infer_prompt_intents, intent_required_capabilities,
};
pub use registry::{ModelRegistry, ModelScore};
pub use select::{
    SelectionAxes, SelectionIntent, SelectionOutcome, SelectionReason, select,
    select_with_default_registry,
};
pub use spec::{
    ModelCapabilities, ModelConfig, ModelRouteBackend, ModelSpec, PricingSource, ProviderType,
    route_backend_for_model, task_category_premium_key, task_category_strength,
};
