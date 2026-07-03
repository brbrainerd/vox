//! Append-only operation log for durable orchestration history.

pub mod backfill;
pub mod checkpoint;
pub mod persist;
mod query;
pub mod sign;
mod store;

pub use persist::PersistError;
pub use query::{list_from_db, list_from_db_since};
pub use store::{append_to_db, append_to_db_with_breaker, mark_undone_in_db};

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use vox_orchestrator_types::AgentId;
use vox_orchestrator_types::ChangeId;
use vox_orchestrator_types::SnapshotId;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// Unique operation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub u64);

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OP-{:06}", self.0)
    }
}

/// Thread-safe generator for [`OperationId`]s.
#[derive(Debug)]
pub struct OperationIdGenerator(AtomicU64);

impl OperationIdGenerator {
    /// Create a new generator starting at 1.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Create a generator whose *next* produced id is `highest_existing + 1`
    /// (T1.3 restart-durability). Pass the highest `op_id` already persisted
    /// in `convergence_op_log` (queried via
    /// `VoxDb::max_convergence_op_id`) so the sequence stays monotonic across
    /// a daemon restart instead of resetting to `OP-000001` and colliding
    /// with — or shadowing — history a client may have already replayed.
    pub fn resuming_after(highest_existing: u64) -> Self {
        Self(AtomicU64::new(highest_existing + 1))
    }

    /// Produce the next unique [`OperationId`].
    pub fn next(&self) -> OperationId {
        OperationId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for OperationIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Operation types
// ---------------------------------------------------------------------------

/// The kind of operation that was performed.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationKind {
    /// An agent edited one or more files.
    FileEdit { paths: Vec<String> },
    /// A task was submitted to the queue.
    TaskSubmit { task_id: u64 },
    /// A task was completed.
    TaskComplete { task_id: u64 },
    /// A task failed.
    TaskFail { task_id: u64, reason: String },
    /// A file lock was acquired.
    LockAcquire { path: String, agent_id: u64 },
    /// A file lock was released.
    LockRelease { path: String, agent_id: u64 },
    /// Tasks were rebalanced across agents.
    Rebalance,
    /// A workspace was created for an agent.
    WorkspaceCreate { agent_id: u64 },
    /// A workspace was merged back.
    WorkspaceMerge { agent_id: u64 },
    /// A conflict was resolved.
    ConflictResolved { path: String, strategy: String },
    /// A file conflict was first detected.
    ConflictDetected {
        path: String,
        severity: String,
        sides: Vec<u64>,
    },
    /// Conflict resolution was deferred to another agent.
    ConflictDeferred { path: String, defer_to_agent: u64 },
    /// A logical change was created.
    ChangeCreated { change_id: u64, description: String },
    /// A logical change's status was updated.
    ChangeUpdated { change_id: u64, new_status: String },
    /// An AI model call was made (cost is stored as microdollars to keep Eq derivable).
    AiCall {
        provider: String,
        model: String,
        input_tokens: u32,
        output_tokens: u32,
        /// Cost in microdollars (multiply by 1e-6 to get USD).
        cost_usd_micro: u64,
    },
    /// An artifact was created.
    ArtifactCreate { artifact_id: String },
    /// An artifact was modified.
    ArtifactModify { artifact_id: String },
    /// A skill was installed.
    SkillInstall { skill_name: String, version: String },
    /// A skill was uninstalled.
    SkillUninstall { skill_name: String },
    /// Generic/custom operation.
    Custom { label: String },
    /// Tier-3 cold compaction: encodes projection state for ops in (op_id_lo..=op_id_hi].
    /// Allows replay to start from the most recent checkpoint instead of replaying from zero.
    Checkpoint {
        op_id_lo: u64,
        op_id_hi: u64,
        /// blake3 over the deterministically encoded projection snapshot.
        projection_blake3: [u8; 32],
        /// Reference into vox-db blob storage with the actual snapshot bytes.
        payload_blob_id: u64,
    },
    /// MENS: references a signed SafeTensors checkpoint bundle in CAS (Mn-T6).
    TrainingCheckpoint {
        session_id: String,
        /// Lowercase SHA3-512 hex (128 chars).
        bundle_hash: String,
        /// Lowercase SHA3-512 hex (128 chars).
        optimizer_state_hash: String,
        step: u64,
    },
    // ── T1.1: dispatch lifecycle events (harness reliability spec, Phase 1) ──
    // These make approval / feedback / doubt / hopper lifecycle transitions
    // durable via the existing op-log rather than a new ledger. See
    // docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md
    // section 3, T1.1. Cost fields are deliberately absent — vox-telemetry
    // stays the cost SSOT; correlate via `trace_id`/`run_id` elsewhere.
    /// A dangerous-tool call parked on a human-in-the-loop approval decision.
    ApprovalRequested {
        approval_id: String,
        tool: String,
        run_id: Option<String>,
    },
    /// A parked approval was resolved (approved/rejected/timed out/modified).
    ApprovalResolved {
        approval_id: String,
        /// Stringified via the same convention as the MCP gate's
        /// `format!("{outcome:?}").to_lowercase()`.
        outcome: String,
        resolver: Option<String>,
    },
    /// A soft-HITL feedback item (clarification / doubt / skill proposal) was registered.
    FeedbackRequested {
        request_id: String,
        task_id: Option<u64>,
        kind: String,
    },
    /// A feedback item was resolved (answer/skip/overrule/let_verify/accept_skill).
    FeedbackResolved { request_id: String, action: String },
    /// A task was flagged as suspect by a human, forcing a verification pass.
    TaskDoubted {
        task_id: u64,
        reason: Option<String>,
    },
    /// A hopper intake item was admitted to the inbox.
    HopperAdmit { item_id: String },
    /// A hopper intake item was assigned to a dispatched task.
    HopperAssign { item_id: String, task_id: u64 },
    /// A hopper intake item's backing task completed; the item is Done.
    HopperComplete { item_id: String },
}

/// A single entry in the operation log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    /// Unique operation ID.
    pub id: OperationId,
    /// Agent that performed the operation.
    pub agent_id: AgentId,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// What kind of operation.
    pub kind: OperationKind,
    /// Human-readable description.
    pub description: String,
    /// Snapshot before the operation (if available).
    pub snapshot_before: Option<SnapshotId>,
    /// Snapshot after the operation (if available).
    pub snapshot_after: Option<SnapshotId>,
    /// Database snapshot before the operation (if available).
    pub db_snapshot_before: Option<u64>,
    /// Database snapshot after the operation (if available).
    pub db_snapshot_after: Option<u64>,
    /// Context snapshot before the operation (if available).
    pub context_snapshot_before: Option<u64>,
    /// Context snapshot after the operation (if available).
    pub context_snapshot_after: Option<u64>,
    /// Whether this operation has been undone.
    pub undone: bool,
    /// Stable logical change ID — survives rebases and amendments.
    /// Links this operation to a logical unit of work across multiple snapshots.
    pub change_id: Option<ChangeId>,
    /// Model that produced this operation (e.g. "gemini-2.5-pro", "claude-3-7-sonnet").
    /// Enables model-level provenance queries.
    pub model_id: Option<String>,
    /// SHA-3-256 hash of the previous operation's ID + timestamp, forming a
    /// cryptographic chain. Allows tamper detection in the audit trail.
    pub predecessor_hash: Option<String>,
    /// Ed25519 signature over the canonical payload (P3-T2). `None` for legacy entries.
    pub signature: Option<Vec<u8>>,
    /// 32-byte id (blake3 of pubkey) of the daemon key used to sign. `None` for legacy.
    pub signing_key_id: Option<Vec<u8>>,
    /// Daemon UUID that produced this entry (16 bytes, all-zero for local-only entries).
    pub daemon_id: [u8; 16],
    /// Parent op-ids (DAG). Usually a single predecessor; multi-parent for merge ops.
    pub parent_op_ids: Vec<u64>,
}

// ---------------------------------------------------------------------------
// OpLog
// ---------------------------------------------------------------------------

/// Append-only operation log with undo/redo support.
#[derive(Debug)]
pub struct OpLog {
    pub(crate) id_gen: OperationIdGenerator,
    pub(crate) db_snap_id_gen: AtomicU64,
    pub(crate) entries: VecDeque<OperationEntry>,
    pub(crate) max_entries: usize,
    /// Optional vox-db persistence context (set by [`OpLog::with_db`]).
    pub(crate) persist: Option<std::sync::Arc<persist::PersistContext>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1.1 dispatch-lifecycle variant fixture/schema validation lives in the
    // `dispatch_events_contract` integration test (crates/vox-orchestrator-queue/tests/
    // dispatch_events_contract.rs), which validates the per-fixture-file corpus under
    // contracts/orchestration/fixtures/dispatch-events/ against
    // contracts/orchestration/dispatch-events.v1.schema.json using the workspace
    // `jsonschema` crate.

    fn agent() -> AgentId {
        AgentId(1)
    }

    #[test]
    fn operation_id_display() {
        assert_eq!(OperationId(7).to_string(), "OP-000007");
    }

    #[test]
    fn record_and_list() {
        let mut log = OpLog::new(100);
        let id = log.record(
            agent(),
            OperationKind::FileEdit {
                paths: vec!["foo.rs".into()],
            },
            "edited foo.rs",
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(log.count(), 1);
        let entries = log.list(None, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
    }

    #[test]
    fn undo_and_redo() {
        let mut log = OpLog::new(100);
        let snap_before = SnapshotId(10);
        let snap_after = SnapshotId(11);

        let id = log.record(
            agent(),
            OperationKind::TaskSubmit { task_id: 1 },
            "submit task",
            Some(snap_before),
            Some(snap_after),
            None,
            None,
            None,
            None,
        );

        // Undo returns snapshot_before and db_snapshot_before
        let result = log.undo(id);
        assert_eq!(result, Some((Some(snap_before), None)));
        assert!(log.get(id).expect("entry exists").undone);

        // Double-undo returns None
        assert_eq!(log.undo(id), None);

        // Redo returns snapshot_after and db_snapshot_after
        let result = log.redo(id);
        assert_eq!(result, Some((Some(snap_after), None)));
        assert!(!log.get(id).expect("entry exists").undone);
    }

    #[test]
    fn eviction() {
        let mut log = OpLog::new(3);
        for i in 0..5 {
            log.record(
                agent(),
                OperationKind::Custom {
                    label: format!("op-{i}"),
                },
                format!("operation {i}"),
                None,
                None,
                None,
                None,
                None,
                None,
            );
        }
        assert_eq!(log.count(), 3);
    }

    #[test]
    fn filter_by_agent() {
        let mut log = OpLog::new(100);
        log.record(
            AgentId(1),
            OperationKind::Rebalance,
            "rebalance",
            None,
            None,
            None,
            None,
            None,
            None,
        );
        log.record(
            AgentId(2),
            OperationKind::Rebalance,
            "rebalance",
            None,
            None,
            None,
            None,
            None,
            None,
        );

        assert_eq!(log.list(Some(AgentId(1)), 10).len(), 1);
        assert_eq!(log.list(Some(AgentId(2)), 10).len(), 1);
        assert_eq!(log.list(None, 10).len(), 2);
    }

    #[test]
    fn last_for_agent_skips_undone() {
        let mut log = OpLog::new(100);
        let id1 = log.record(
            agent(),
            OperationKind::Custom {
                label: "first".into(),
            },
            "first op",
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let _id2 = log.record(
            agent(),
            OperationKind::Custom {
                label: "second".into(),
            },
            "second op",
            None,
            None,
            None,
            None,
            None,
            None,
        );

        log.undo(_id2);
        let last = log.last_for_agent(agent()).expect("should find one");
        assert_eq!(last.id, id1);
    }
}
