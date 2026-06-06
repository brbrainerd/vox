//! Research pipeline subsystem for `vox-orchestrator`.
//!
//! See `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`
//! for the strategic context. This module is currently in **Phase 0a stub**
//! state: types are real, behavior returns empty/default values. Phase 1
//! replaces the stub bodies with the `vox-claim-extractor` crate.
//!
//! All stubs are marked `// PHASE_0a_STUB` for grep-based discovery.

pub mod claims;
pub(super) mod config;
pub mod emitter;
pub mod gate;
pub(super) mod json_parse;
mod mesh_subscriber;
pub mod model_select;
pub mod orchestrator;
pub mod persistence;
pub mod planner;
pub mod provider;
mod research_event_metrics_bridge;
mod search_policy_feedback;
pub mod types;
pub mod verifier;

pub use emitter::BroadcastEmitter;
pub use mesh_subscriber::{
    ScientiaMeshSubscriberOptions, spawn_scientia_mesh_research_event_subscriber,
};
pub use orchestrator::{
    ResearchConfig, run_research, run_research_with_context, run_research_with_context_and_session,
};
pub use research_event_metrics_bridge::TELEMETRY_CATALOG_ID_RESEARCH_EVENT_BRIDGE;
pub(crate) use research_event_metrics_bridge::spawn_persist_research_event_for_metrics;
pub use search_policy_feedback::load_rolling_search_policy_feedback;
pub use types::{
    Citation, CompetenceSignal, ResearchHit, ResearchMetadata, ResearchPlan, ResearchQuery,
    ResearchResult, ResearchScope, RetrievalDiagnostics, RoutingTier, SelfVerificationResult,
};
