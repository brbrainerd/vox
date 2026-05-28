//! # vox-db-types — Pure data types for [`vox-db`](../vox_db/index.html)
//!
//! Lightweight crate holding parameter and row structs used by the `vox-db` facade.
//! Consumers that only need types (not connection-bearing operations) can depend on
//! this crate to avoid pulling in `turso` and other heavy operational deps.
//!
//! `vox-db` re-exports everything here for back-compat: `vox_db::SaveSnippetParams`
//! resolves the same as `vox_db_types::SaveSnippetParams`.

#![allow(clippy::collapsible_if)]
#![allow(missing_docs)] // many types here have crate-level docs in vox-db

/// Circuit-breaker state enum (`CircuitState`) for `vox_db::circuit_breaker`.
pub mod circuit;
pub use circuit::CircuitState;

/// Workspace-journey store mode enum (`WorkspaceJourneyStoreMode`).
pub mod workspace_journey;
pub use workspace_journey::WorkspaceJourneyStoreMode;

/// Execution time telemetry types (`ExecOutcome`, `ExecTimeRecord`, `ToolLatencyProfile`).
pub mod exec_time;
pub use exec_time::{ExecOutcome, ExecTimeRecord, ToolLatencyProfile};

/// Parameters for [`crate::EvalRunParams`] (RLHF / dogfood eval-run recording).
pub mod eval_params;
pub use eval_params::EvalRunParams;

/// Request parameters, row shapes, and MENS observation/training types
/// (formerly `vox_db::store::types::*`).
pub mod store_types;
pub use store_types::*;

/// SCIENTIA / research-session row DTOs.
pub mod research;
pub use research::{ResearchArtifactRecord, ResearchSessionRecord, ResearchSessionSummary};

/// Alias kept for back-compat (`vox_db::MemoryParams`).
pub type MemoryParams<'a> = store_types::SaveMemoryParams<'a>;

/// Typed string-ID newtypes for DB row fields (UUIDs, hashes, human-readable IDs).
pub mod ids;
pub use ids::{DbAgentId, DbCorrelationId, DbPlanSessionId, DbSessionId, DbTaskId, DbUserId};
