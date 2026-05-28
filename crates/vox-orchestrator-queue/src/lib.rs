//! Queue/lock/oplog primitives for vox-orchestrator.
//!
//! Extracted from `vox-orchestrator` (2026-05-08). Depends only on
//! `vox-orchestrator-types` so it lives below the orchestrator core in the
//! layer model.

pub mod affinity;
pub mod locks;
pub mod oplog;
pub mod projection;
pub mod projections;
pub mod sync_lock;
