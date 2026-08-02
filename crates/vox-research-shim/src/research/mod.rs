//! Research pipeline subsystem for `vox-orchestrator`.
//!
//! See `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`
//! for the strategic context and
//! `docs/src/architecture/deep-research-verification-2026-08-01.md` for a
//! current-state audit. As of 2026-08-01, claim extraction, verification,
//! confidence gating, and synthesis are real LLM-backed implementations
//! (behind the `runtime` feature) — this module is no longer in a blanket
//! stub state. Individual known gaps (not "everything is a stub") are
//! tracked in the verification doc above; grep `PHASE_0a_STUB` only finds
//! genuinely narrow remaining placeholders, not whole-module stubs.

pub mod claims;
pub(super) mod config;
pub mod discovery_bridge;
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
