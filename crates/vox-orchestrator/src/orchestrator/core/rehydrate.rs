//! T1.4 — rehydrate in-flight direct-submit tasks from the durable op-log on
//! daemon boot.
//!
//! `submit_task`/`submit_task_with_agent` (the non-hopper path) enqueue onto
//! an agent's in-memory `AgentQueue` only — there is no durable copy of the
//! `AgentTask` itself, only a `TaskSubmit { task_id }` entry in the op-log
//! (see `orchestrator/task_dispatch/submit/task_submit.rs`). A task that was
//! queued or `InProgress` at crash time is otherwise gone forever on restart.
//!
//! This module scans the durable op-log for `TaskSubmit` entries with no
//! matching `TaskComplete`/`TaskFail` for the same `task_id`, and re-enqueues
//! a reconstructed placeholder task for each — mirroring the existing
//! hopper-inbox rehydration loop's `enqueue_dedup` pattern in `init.rs`.
//!
//! ## Fidelity: honest, not exact
//!
//! The op-log's `TaskSubmit` entry carries only `task_id` (see
//! `OperationKind::TaskSubmit`) — never the original description or file
//! manifest, which lived solely in the in-memory `AgentTask`. A crashed
//! daemon therefore cannot recover the *original* task content, only the
//! fact that some task was submitted and never reached a terminal state.
//! The reconstructed task:
//! - carries the op-log entry's own human-readable `description` string
//!   (e.g. `"Submitted task <id>"`) prefixed with a recovery marker, since
//!   that's the only description-shaped text durably available;
//! - starts `TaskStatus::Queued` (fresh), never a synthesized `Interrupted`
//!   state — `AgentTask`/`TaskStatus` has no such variant today, and the plan
//!   explicitly allows "re-enqueuing it fresh, to be picked up and re-run" as
//!   the documented interim behavior rather than inventing new task-state
//!   machinery in this task's scope. A task that was actually `InProgress`
//!   when the daemon died is therefore restored as freshly `Queued`, not
//!   resumed mid-work — it will re-run from the top.
//! - is deduplicated against both other reconstructed tasks and the existing
//!   hopper-inbox rehydration loop via `enqueue_dedup`, which rejects a
//!   duplicate case-insensitive description on the same queue.
//!
//! ## Hopper interplay (no double-enqueue)
//!
//! A hopper-admitted item that was already `assign`ed to a task (has a
//! `HopperAssign { task_id, .. }` oplog entry) is *not* eligible for hopper
//! rehydration in the first place — `SqliteHopper::inbox()` only returns
//! `ItemState::Inbox` rows (verified: the DB query filters
//! `WHERE state = '"inbox"'`), so an assigned-but-incomplete hopper item is
//! invisible to both rehydration loops today. Rather than leaving that gap
//! silently unrehydrated, this module explicitly excludes `task_id`s with a
//! `HopperAssign` entry from ITS OWN reconstruction, to avoid ever emitting a
//! second, differently-described copy of the same task if a future change
//! makes assigned hopper items visible again; closing the "assigned hopper
//! item interrupted mid-flight" gap itself is out of scope for T1.4 (tracked
//! as a follow-up in the T1.4 completion report).
//!
//! ## T1.6: bounded rehydration via checkpoints
//!
//! [`rehydrate_direct_submit_tasks`] originally always scanned the *entire*
//! durable op-log (`list_from_db(..., limit: 100_000)`), so boot time grew
//! linearly with lifetime event count even though the actual live state
//! (open tasks) is typically tiny. T1.6 adds a checkpoint of exactly the
//! derived fact this module needs — the still-open `(task_id -> description)`
//! map plus the `hopper_assigned` exclusion set, as of some `op_id_hi` — so a
//! restart only has to (a) restore that small map from the checkpoint blob
//! and (b) scan the tail (`op_id > op_id_hi`) for anything that happened
//! since. This reuses T1.4's existing scan-based reducer (below) rather than
//! introducing a new formal `Projection` impl: the reducer already *is* a
//! deterministic fold over the op-log into exactly the state this module
//! needs to restore, which is what a `Projection` would otherwise do, and
//! building a second representation of the same fold would just be more
//! surface area for the two to drift apart.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::oplog::{OperationEntry, OperationKind};
use crate::types::{AgentTask, TaskId, TaskPriority};

/// Deterministic fold of open-task state derived from a run of op-log
/// entries (T1.4's original reducer, factored out for T1.6 checkpointing).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OpenTaskState {
    // NOTE: `pub(crate)` (not private) — `orchestrator/core/checkpoint.rs`
    // constructs, folds, and serializes this directly (T1.6).
    /// task_id -> submit entry's description, for tasks with no terminal
    /// (TaskComplete/TaskFail) entry yet.
    pub(crate) submitted: HashMap<u64, String>,
    /// task_ids with a HopperAssign entry (excluded from direct-submit
    /// rehydration; the hopper-inbox loop owns them).
    pub(crate) hopper_assigned: HashSet<u64>,
}

/// `pub(crate)` entry point for [`OpenTaskState::fold`], used by
/// `orchestrator/core/checkpoint.rs` (T1.6) to fold checkpoint-tail entries
/// into a restored `OpenTaskState` without duplicating the reducer.
pub(crate) fn fold_open_task_state(
    state: OpenTaskState,
    entries: &[OperationEntry],
) -> OpenTaskState {
    state.fold(entries)
}

impl OpenTaskState {
    /// Fold `entries` (oldest-first) into open-task state, starting from
    /// `self` (empty for a full scan, or a checkpoint's restored state for a
    /// bounded tail replay).
    fn fold(mut self, entries: &[OperationEntry]) -> Self {
        for entry in entries {
            match &entry.kind {
                OperationKind::TaskSubmit { task_id } => {
                    self.submitted.insert(*task_id, entry.description.clone());
                }
                OperationKind::TaskComplete { task_id } => {
                    self.submitted.remove(task_id);
                }
                OperationKind::TaskFail { task_id, .. } => {
                    self.submitted.remove(task_id);
                }
                OperationKind::HopperAssign { task_id, .. } => {
                    self.hopper_assigned.insert(*task_id);
                }
                _ => {}
            }
        }
        self
    }

    /// Re-enqueue every still-open, non-hopper-assigned task onto the least-
    /// loaded agent queue. Returns the number of tasks re-enqueued.
    fn reenqueue(&self, orch: &crate::orchestrator::Orchestrator) -> usize {
        let mut rehydrated = 0usize;
        for (task_id, description) in &self.submitted {
            let task_id = *task_id;
            if self.hopper_assigned.contains(&task_id) {
                // Hopper-sourced; the hopper-inbox loop owns rehydration for this
                // item (or it's legitimately not re-enqueued because it's no
                // longer in ItemState::Inbox — see module docs).
                continue;
            }

            let recovered_desc = format!("[recovered-on-restart] {description}");
            let task = AgentTask::new(
                TaskId(task_id),
                recovered_desc,
                TaskPriority::Normal,
                vec![],
            );

            let agents_guard = orch.agents.read().unwrap();
            let Some(queue_arc) = agents_guard
                .values()
                .min_by_key(|q| q.read().unwrap().len())
            else {
                tracing::warn!(
                    task_id,
                    "T1.4: no active agents available to rehydrate direct-submit task"
                );
                continue;
            };
            let mut queue = queue_arc.write().unwrap();
            if queue.enqueue_dedup(task) {
                rehydrated += 1;
            }
        }
        rehydrated
    }
}

/// Reconstruct and re-enqueue direct-submit tasks that were submitted but
/// never reached a terminal state (`TaskComplete`/`TaskFail`) as of the last
/// durable op-log record. Returns the number of tasks re-enqueued.
///
/// Must run after the hopper-inbox rehydration loop in `init_db` so the
/// `HopperAssign` exclusion set below reflects hopper state as of boot.
///
/// T1.6: if a checkpoint exists (see `orchestrator/core/checkpoint.rs`), this
/// restores `OpenTaskState` from it and only scans the post-checkpoint tail
/// instead of the full history — see the module-level "bounded rehydration"
/// docs above.
pub(crate) async fn rehydrate_direct_submit_tasks(
    orch: &crate::orchestrator::Orchestrator,
    db: &vox_db::VoxDb,
) -> usize {
    let repo = crate::lineage::repository_id();

    let (base_state, entries) =
        match super::checkpoint::latest_agent_oplog_checkpoint(db, &repo).await {
            Ok(Some((state, op_id_hi))) => {
                match vox_orchestrator_queue::oplog::list_from_db_since(db, repo.as_str(), op_id_hi)
                    .await
                {
                    Ok(tail) => {
                        tracing::debug!(
                            checkpoint_op_id_hi = op_id_hi,
                            tail_len = tail.len(),
                            "T1.6: rehydrating direct-submit tasks from checkpoint + bounded tail"
                        );
                        (state, tail)
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "T1.6: failed to list post-checkpoint tail; falling back to full scan"
                        );
                        match full_scan(db, &repo).await {
                            Some(entries) => (OpenTaskState::default(), entries),
                            None => return 0,
                        }
                    }
                }
            }
            Ok(None) => match full_scan(db, &repo).await {
                Some(entries) => (OpenTaskState::default(), entries),
                None => return 0,
            },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "T1.6: failed to load latest checkpoint; falling back to full scan"
                );
                match full_scan(db, &repo).await {
                    Some(entries) => (OpenTaskState::default(), entries),
                    None => return 0,
                }
            }
        };

    let state = base_state.fold(&entries);
    let rehydrated = state.reenqueue(orch);

    if rehydrated > 0 {
        tracing::info!(
            rehydrated_task_count = rehydrated,
            "T1.4: re-enqueued in-flight direct-submit tasks from durable oplog on boot"
        );
    }

    rehydrated
}

async fn full_scan(db: &vox_db::VoxDb, repo: &str) -> Option<Vec<OperationEntry>> {
    match crate::oplog::list_from_db(db, None, repo, 100_000).await {
        Ok(e) => Some(e),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "T1.4: failed to list durable oplog for direct-submit task rehydration; \
                 skipping (in-flight tasks submitted before this crash may be lost)"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator_types::AgentId;

    fn entry(kind: OperationKind) -> OperationEntry {
        OperationEntry {
            id: crate::oplog::OperationId(1),
            agent_id: AgentId(1),
            timestamp_ms: 0,
            kind,
            description: "desc".into(),
            snapshot_before: None,
            snapshot_after: None,
            db_snapshot_before: None,
            db_snapshot_after: None,
            context_snapshot_before: None,
            context_snapshot_after: None,
            undone: false,
            change_id: None,
            model_id: None,
            predecessor_hash: None,
            signature: None,
            signing_key_id: None,
            daemon_id: [0u8; 16],
            parent_op_ids: Vec::new(),
        }
    }

    #[test]
    fn fold_removes_task_on_terminal_entry() {
        let state = OpenTaskState::default().fold(&[
            entry(OperationKind::TaskSubmit { task_id: 1 }),
            entry(OperationKind::TaskComplete { task_id: 1 }),
        ]);
        assert!(state.submitted.is_empty());
    }

    #[test]
    fn fold_keeps_non_terminal_task_open() {
        let state =
            OpenTaskState::default().fold(&[entry(OperationKind::TaskSubmit { task_id: 7 })]);
        assert_eq!(state.submitted.len(), 1);
        assert!(state.submitted.contains_key(&7));
    }

    #[test]
    fn fold_from_checkpoint_base_applies_tail_on_top() {
        let mut base = OpenTaskState::default();
        base.submitted.insert(1, "task one".into());
        base.submitted.insert(2, "task two".into());

        // Tail: task 1 completes after the checkpoint, task 3 is newly submitted.
        let folded = base.fold(&[
            entry(OperationKind::TaskComplete { task_id: 1 }),
            entry(OperationKind::TaskSubmit { task_id: 3 }),
        ]);

        assert!(
            !folded.submitted.contains_key(&1),
            "task 1 completed in tail"
        );
        assert!(
            folded.submitted.contains_key(&2),
            "task 2 untouched by tail"
        );
        assert!(
            folded.submitted.contains_key(&3),
            "task 3 submitted in tail"
        );
    }

    #[test]
    fn state_roundtrips_through_json() {
        let mut state = OpenTaskState::default();
        state.submitted.insert(5, "hello".into());
        state.hopper_assigned.insert(9);

        let json = serde_json::to_vec(&state).unwrap();
        let restored: OpenTaskState = serde_json::from_slice(&json).unwrap();
        assert_eq!(state, restored);
    }
}
