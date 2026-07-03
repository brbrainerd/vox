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

use std::collections::{HashMap, HashSet};

use crate::oplog::OperationKind;
use crate::types::{AgentTask, TaskId, TaskPriority};

/// Reconstruct and re-enqueue direct-submit tasks that were submitted but
/// never reached a terminal state (`TaskComplete`/`TaskFail`) as of the last
/// durable op-log record. Returns the number of tasks re-enqueued.
///
/// Must run after the hopper-inbox rehydration loop in `init_db` so the
/// `HopperAssign` exclusion set below reflects hopper state as of boot.
pub(crate) async fn rehydrate_direct_submit_tasks(
    orch: &crate::orchestrator::Orchestrator,
    db: &vox_db::VoxDb,
) -> usize {
    let repo = crate::lineage::repository_id();
    let entries = match crate::oplog::list_from_db(db, None, repo.as_str(), 100_000).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "T1.4: failed to list durable oplog for direct-submit task rehydration; \
                 skipping (in-flight tasks submitted before this crash may be lost)"
            );
            return 0;
        }
    };

    // task_id -> submit entry's description
    let mut submitted: HashMap<u64, String> = HashMap::new();
    let mut terminal: HashSet<u64> = HashSet::new();
    let mut hopper_assigned: HashSet<u64> = HashSet::new();

    for entry in &entries {
        match &entry.kind {
            OperationKind::TaskSubmit { task_id } => {
                submitted.insert(*task_id, entry.description.clone());
            }
            OperationKind::TaskComplete { task_id } => {
                terminal.insert(*task_id);
            }
            OperationKind::TaskFail { task_id, .. } => {
                terminal.insert(*task_id);
            }
            OperationKind::HopperAssign { task_id, .. } => {
                hopper_assigned.insert(*task_id);
            }
            _ => {}
        }
    }

    let mut rehydrated = 0usize;
    for (task_id, description) in submitted {
        if terminal.contains(&task_id) {
            continue; // reached TaskComplete/TaskFail — nothing to restore
        }
        if hopper_assigned.contains(&task_id) {
            // Hopper-sourced; the hopper-inbox loop owns rehydration for this
            // item (or it's legitimately not re-enqueued because it's no
            // longer in ItemState::Inbox — see module docs).
            continue;
        }

        let recovered_desc = format!("[recovered-on-restart] {description}");
        let task = AgentTask::new(TaskId(task_id), recovered_desc, TaskPriority::Normal, vec![]);

        let agents_guard = orch.agents.read().unwrap();
        let Some(queue_arc) = agents_guard.values().min_by_key(|q| q.read().unwrap().len()) else {
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

    if rehydrated > 0 {
        tracing::info!(
            rehydrated_task_count = rehydrated,
            "T1.4: re-enqueued in-flight direct-submit tasks from durable oplog on boot"
        );
    }

    rehydrated
}
