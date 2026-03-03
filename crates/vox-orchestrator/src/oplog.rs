//! Operation log — inspired by Jujutsu's operation log and `UnpublishedOperation` model.
//!
//! Records every agent action as an immutable entry with before/after snapshots,
//! enabling universal undo/redo.  This is the safety net that lets agents
//! experiment fearlessly.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

use crate::snapshot::SnapshotId;
use crate::types::AgentId;
use crate::workspace::ChangeId;

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
}

// ---------------------------------------------------------------------------
// OpLog
// ---------------------------------------------------------------------------

/// Append-only operation log with undo/redo support.
#[derive(Debug)]
pub struct OpLog {
    id_gen: OperationIdGenerator,
    db_snap_id_gen: AtomicU64,
    entries: VecDeque<OperationEntry>,
    max_entries: usize,
}

impl OpLog {
    /// Create a new log with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            id_gen: OperationIdGenerator::new(),
            db_snap_id_gen: AtomicU64::new(1),
            entries: VecDeque::new(),
            max_entries,
        }
    }

    /// Produce the next unique DB snapshot ID.
    pub fn next_db_snapshot_id(&self) -> u64 {
        self.db_snap_id_gen.fetch_add(1, Ordering::Relaxed)
    }

    /// Access the full history of operations (oldest first).
    pub fn history(&self) -> Vec<&OperationEntry> {
        self.entries.iter().collect()
    }

    /// Access the stack of operations that can be redone (most recently undone last).
    pub fn redo_stack(&self) -> Vec<&OperationEntry> {
        self.entries.iter().filter(|e| e.undone).collect()
    }

    /// Record a new operation.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
    ) -> OperationId {
        self.record_with_provenance(
            agent_id, kind, description,
            snapshot_before, snapshot_after,
            db_snapshot_before, db_snapshot_after,
            context_snapshot_before, context_snapshot_after,
            None, None,
        )
    }

    /// Record an operation with full provenance (change ID + model ID).
    #[allow(clippy::too_many_arguments)]
    pub fn record_with_provenance(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
        change_id: Option<ChangeId>,
        model_id: Option<String>,
    ) -> OperationId {
        let id = self.id_gen.next();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Compute cryptographic chain: hash(prev_id || prev_timestamp)
        let predecessor_hash = self.entries.back().map(|prev| {
            let mut hasher = Sha3_256::new();
            hasher.update(prev.id.0.to_le_bytes());
            hasher.update(prev.timestamp_ms.to_le_bytes());
            hasher.update(&prev.predecessor_hash.as_deref().unwrap_or("").as_bytes());
            format!("{:x}", hasher.finalize())
        });

        let entry = OperationEntry {
            id,
            agent_id,
            timestamp_ms,
            kind,
            description: description.into(),
            snapshot_before,
            snapshot_after,
            db_snapshot_before,
            db_snapshot_after,
            context_snapshot_before,
            context_snapshot_after,
            undone: false,
            change_id,
            model_id,
            predecessor_hash,
        };

        self.entries.push_back(entry);

        // Evict oldest if over limit.
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        id
    }

    /// List recent operations (newest first), optionally filtered by agent.
    pub fn list(&self, agent_id: Option<AgentId>, limit: usize) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| agent_id.is_none_or(|a| e.agent_id == a))
            .take(limit)
            .collect()
    }

    /// Mark an operation as undone.
    ///
    /// Returns the entry's `snapshot_before` and `db_snapshot_before` so the caller can restore state.
    pub fn undo(&mut self, op_id: OperationId) -> Option<(Option<SnapshotId>, Option<u64>)> {
        for entry in self.entries.iter_mut().rev() {
            if entry.id == op_id && !entry.undone {
                entry.undone = true;
                return Some((entry.snapshot_before, entry.db_snapshot_before));
            }
        }
        None
    }

    /// Mark a previously undone operation as active again.
    ///
    /// Returns the entry's `snapshot_after` and `db_snapshot_after` so the caller can re-apply state.
    pub fn redo(&mut self, op_id: OperationId) -> Option<(Option<SnapshotId>, Option<u64>)> {
        for entry in self.entries.iter_mut().rev() {
            if entry.id == op_id && entry.undone {
                entry.undone = false;
                return Some((entry.snapshot_after, entry.db_snapshot_after));
            }
        }
        None
    }

    /// Find the most recent non-undone operation for an agent.
    pub fn last_for_agent(&self, agent_id: AgentId) -> Option<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.agent_id == agent_id && !e.undone)
    }

    /// Get a specific operation by ID.
    pub fn get(&self, op_id: OperationId) -> Option<&OperationEntry> {
        self.entries.iter().find(|e| e.id == op_id)
    }

    /// Total number of entries.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Find the snapshots associated with a task's submission.
    pub fn find_task_snapshots(&self, task_id: u64) -> (Option<SnapshotId>, Option<u64>) {
        for entry in self.entries.iter().rev() {
            if let OperationKind::TaskSubmit { task_id: id } = entry.kind {
                if id == task_id {
                    return (entry.snapshot_before, entry.db_snapshot_before);
                }
            }
        }
        (None, None)
    }

    /// Find all operations belonging to a logical change.
    /// Returns entries in chronological order (oldest first).
    pub fn find_by_change_id(&self, change_id: ChangeId) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .filter(|e| e.change_id == Some(change_id))
            .collect()
    }

    /// Find all operations produced by a specific model (e.g. "gemini-2.5-pro").
    pub fn find_by_model(&self, model: &str) -> Vec<&OperationEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.model_id.as_deref() == Some(model))
            .collect()
    }

    /// Verify the cryptographic chain integrity of the log.
    /// Returns the index of the first broken link, or `Ok(())` if intact.
    pub fn verify_chain(&self) -> Result<(), usize> {
        for (i, entry) in self.entries.iter().enumerate().skip(1) {
            let prev = &self.entries[i - 1];
            let mut hasher = sha3::Sha3_256::new();
            sha3::Digest::update(&mut hasher, prev.id.0.to_le_bytes());
            sha3::Digest::update(&mut hasher, prev.timestamp_ms.to_le_bytes());
            sha3::Digest::update(
                &mut hasher,
                prev.predecessor_hash.as_deref().unwrap_or("").as_bytes(),
            );
            let expected = format!("{:x}", sha3::Digest::finalize(hasher));
            if entry.predecessor_hash.as_deref() != Some(&expected) {
                return Err(i);
            }
        }
        Ok(())
    }

    /// Record an AI model call in the log (convenience wrapper).
    pub fn record_ai_call(
        &mut self,
        agent_id: AgentId,
        provider: impl Into<String>,
        model: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    ) -> OperationId {
        let cost_usd_micro = (cost_usd * 1_000_000.0).round() as u64;
        let provider = provider.into();
        let model = model.into();
        let desc = format!(
            "AI call: {}/{} ({} in / {} out tokens, ${:.6})",
            provider, model, input_tokens, output_tokens, cost_usd
        );
        self.record(
            agent_id,
            OperationKind::AiCall {
                provider,
                model,
                input_tokens,
                output_tokens,
                cost_usd_micro,
            },
            desc,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Total cost across all recorded AI calls (in USD).
    pub fn total_cost_usd(&self) -> f64 {
        self.entries
            .iter()
            .filter_map(|e| {
                if let OperationKind::AiCall { cost_usd_micro, .. } = &e.kind {
                    Some(*cost_usd_micro as f64 * 1e-6)
                } else {
                    None
                }
            })
            .sum()
    }

    /// Total tokens consumed across all AI calls.
    pub fn total_tokens(&self) -> (u64, u64) {
        self.entries
            .iter()
            .filter_map(|e| {
                if let OperationKind::AiCall { input_tokens, output_tokens, .. } = &e.kind {
                    Some((*input_tokens as u64, *output_tokens as u64))
                } else {
                    None
                }
            })
            .fold((0u64, 0u64), |(ai, ao), (i, o)| (ai + i, ao + o))
    }
}

impl Default for OpLog {
    fn default() -> Self {
        Self::new(1000)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        );
        log.record(
            AgentId(2),
            OperationKind::Rebalance,
            "rebalance",
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
        );

        log.undo(_id2);
        let last = log.last_for_agent(agent()).expect("should find one");
        assert_eq!(last.id, id1);
    }
}
