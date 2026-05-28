//! Typed orchestrator settings: scaling, queues, compaction, sessions, and env overlay.
//!
//! [`OrchestratorConfig`] loads from `Vox.toml` / environment and is validated before use.

mod defaults;
mod enums;
mod errors;
mod impl_default;
mod impl_env;
mod impl_load;
mod impl_validate;
mod merge_populi;
mod news;
mod orchestrator_fields;
mod scientia_research_mesh;

pub use enums::{CostPreference, OverflowStrategy, ScalingProfile};
pub use errors::{ConfigError, ConfigValidationError};
pub use news::NewsConfig;
pub use orchestrator_fields::OrchestratorConfig;
pub use scientia_research_mesh::ScientiaResearchMeshConfig;

#[cfg(test)]
mod tests;
