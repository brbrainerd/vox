//! DEI research pipeline and model-selection sub-systems.
//!
//! Extracted from `vox-orchestrator/src/dei_shim/` as a wedge crate (A-12).
//! The public surface is intentionally identical to the old `vox_orchestrator::dei_shim::*`
//! path — consumers need only update their import root.

pub mod agent_frontmatter;
pub mod research;
pub mod route_telemetry;
/// Model-selection sub-system: task→strength mapping, pluggable scoring, free-tier routing.
///
/// Previously deferred (WIP) because it required `ModelTier::Fast/Free` and `RoutingProfile`
/// which are now available (added 2026-05-24, F-F track).
pub mod selection;

pub mod research_policy {
    pub use vox_orchestrator_types::socrates_policy::ConfidencePolicy;

    #[must_use]
    pub const fn persist_min_confidence() -> f64 {
        ConfidencePolicy::DEFAULT_MIN_PERSIST_CONFIDENCE
    }

    #[must_use]
    pub const fn training_pair_min_confidence() -> f64 {
        ConfidencePolicy::DEFAULT_MIN_TRAINING_PAIR_CONFIDENCE
    }
}
