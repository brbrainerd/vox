//! SCIENTIA Phase H — dashboard panel JSON builders.
//!
//! The dashboard backend collects rows from the existing tables
//! (`scientia_finding_candidates`, `scientia_claims`,
//! `publication_manifests`, `publication_status_events`,
//! `external_submission_attempts`) and assembles a [`QueueSnapshot`] /
//! [`CostRollup`]. This crate owns the *shapes* and the deterministic
//! build logic; the routes themselves live in the dashboard backend.
//!
//! Conventions per the
//! [Mesh § 5.6 route convention](../../../docs/src/architecture/mesh-and-language-distribution-ssot-2026.md):
//!
//! - REST: `GET /api/v2/scientia/queue` → JSON of [`QueueSnapshot`].
//! - REST: `GET /api/v2/scientia/cost`  → JSON of [`CostRollup`].
//! - WS topic: `scientia.queue.changed` published on any candidate insert,
//!   claim verification, reply-window state change, retraction, or cost
//!   rollup tick.
//!
//! The crate is pure: it accepts a [`DashboardInputs`] (whatever the
//! dashboard backend's adapter assembled from the DB) and returns the
//! response struct. No DB queries here.

pub mod cost;
pub mod queue;
pub mod stalls;

pub use cost::{
    build_cost_rollup, CostByProvider, CostRollup, QuarterlyCostSummary,
};
pub use queue::{
    build_queue_snapshot, CandidateRow, ClaimsPendingSummary, DashboardInputs,
    QueueSnapshot, ReplyWindowEntry,
};
pub use stalls::{detect_stalls, StallEntry, STALE_THRESHOLD_MS};
