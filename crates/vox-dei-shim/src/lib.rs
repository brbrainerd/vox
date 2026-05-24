//! DEI research pipeline and model-selection sub-systems.
//!
//! Extracted from `vox-orchestrator/src/dei_shim/` as a wedge crate (A-12).
//! The public surface is intentionally identical to the old `vox_orchestrator::dei_shim::*`
//! path — consumers need only update their import root.

pub mod agent_frontmatter;
pub mod research;
pub mod route_telemetry;
// selection/ (WIP model-scoring code) was not wired into the original dei_shim module tree.
// It requires ModelTier::Fast/Free, RoutingProfile, and other types not yet promoted to
// vox-orchestrator-types. Wire it back in during Tier-D Phase 1.

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
