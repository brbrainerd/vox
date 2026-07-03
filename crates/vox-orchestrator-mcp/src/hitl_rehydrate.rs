//! T1.4 — restore visibility for pending approvals and open feedback requests
//! that were requested but never resolved before a daemon/MCP-server
//! restart.
//!
//! `PendingApprovals` (this crate) and `FeedbackStore` (`vox-orchestrator`)
//! are both in-memory-only registries keyed by a `oneshot`/plain in-memory
//! item respectively — a parked approval or open feedback request dies with
//! the process. The durable op-log (T1.1: `ApprovalRequested`/
//! `ApprovalResolved`, `FeedbackRequested`/`FeedbackResolved`) plus
//! `hitl_approvals` (the DB-audit table) are the sources of truth for "which
//! approvals/feedback items are still open" — this module reconciles them
//! into the live in-memory registries on boot, called from
//! `ServerState::with_db_initialized` (the one hook both the MCP stdio server
//! and the `vox-orchestrator-d` daemon binary run after attaching a DB).
//!
//! ## What "restored" means here (read before assuming more than this does)
//!
//! The ORIGINAL tool call that parked on an approval, or the original
//! interactive prompt behind a feedback request, died with the process that
//! held it — there is no live Rust task/thread waiting on the other end
//! anymore, and nothing here resumes it. What this module restores is:
//! - **Visibility**: the item reappears in `vox_pending_approvals` /
//!   `FeedbackStore::open_needs_you()` so a human can see it's still open.
//! - **Resolvability**: `vox_resolve_approval` / the feedback resolve tool
//!   still work against the same id, so the decision gets recorded (audit
//!   trail stays consistent) even though nothing wakes up as a result.
//! This is a deliberate, honest partial resume — not a claim that the
//! original in-flight tool call can be un-crashed.

use std::collections::HashMap;

use vox_orchestrator::oplog::OperationKind;

/// Scan the durable op-log for `ApprovalRequested`/`FeedbackRequested`
/// entries with no matching `*Resolved` counterpart as of the last durable
/// record, and re-park/re-register each into the live `PendingApprovals` /
/// `FeedbackStore` registries on `state`. Best-effort: a failure to list the
/// op-log logs a warning and leaves both registries empty rather than
/// failing MCP/daemon startup.
pub async fn rehydrate_open_hitl_from_oplog(state: &crate::server_state::ServerState) {
    let repo = vox_orchestrator::lineage::repository_id();
    let entries = match state.orchestrator.list_recent_operations(None, 100_000).await {
        entries if !entries.is_empty() => entries,
        _ => Vec::new(),
    };
    // `list_recent_operations` already merges in-memory + durable DB rows for
    // the current process's oplog; since this runs at boot before any new
    // durable writes happen, "recent" here effectively means "everything
    // durable for this repo" for a freshly-attached DB. Fall back to a direct
    // DB query if the in-memory merge came back empty but a DB is attached
    // (e.g. a from-scratch ServerState that hasn't touched the oplog yet).
    let entries = if entries.is_empty() {
        if let Some(db) = state.db.as_ref() {
            vox_orchestrator::oplog::list_from_db(db, None, repo.as_str(), 100_000)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        entries
    };

    // approval_id -> (tool, requested_at_ms)
    let mut approvals_open: HashMap<String, (String, u64)> = HashMap::new();
    // request_id -> (kind_string, task_id, created_at_ms)
    let mut feedback_open: HashMap<String, (String, Option<u64>, u64)> = HashMap::new();

    for entry in &entries {
        match &entry.kind {
            OperationKind::ApprovalRequested {
                approval_id, tool, ..
            } => {
                approvals_open.insert(
                    approval_id.clone(),
                    (tool.clone(), entry.timestamp_ms),
                );
            }
            OperationKind::ApprovalResolved { approval_id, .. } => {
                approvals_open.remove(approval_id);
            }
            OperationKind::FeedbackRequested {
                request_id,
                task_id,
                kind,
            } => {
                feedback_open.insert(
                    request_id.clone(),
                    (kind.clone(), *task_id, entry.timestamp_ms),
                );
            }
            OperationKind::FeedbackResolved { request_id, .. } => {
                feedback_open.remove(request_id);
            }
            _ => {}
        }
    }

    let approval_count = approvals_open.len();
    for (approval_id, (tool, requested_at_ms)) in approvals_open {
        let summary = format!("[recovered-on-restart] {tool}");
        state
            .pending_approvals
            .reregister_after_restart(approval_id, tool, summary, requested_at_ms);
    }

    let feedback_count = feedback_open.len();
    for (request_id, (kind_str, task_id, created_at_ms)) in feedback_open {
        let kind = match kind_str.as_str() {
            "doubt" => vox_orchestrator::feedback::FeedbackKind::Doubt,
            "skill_proposal" => vox_orchestrator::feedback::FeedbackKind::SkillProposal,
            _ => vox_orchestrator::feedback::FeedbackKind::Clarification,
        };
        state.feedback.rehydrate_open(
            vox_orchestrator::feedback::FeedbackId(request_id),
            kind,
            task_id.map(vox_orchestrator::types::TaskId),
            created_at_ms,
        );
    }

    if approval_count > 0 || feedback_count > 0 {
        tracing::info!(
            approval_count,
            feedback_count,
            "T1.4: restored visibility for open approvals/feedback from durable oplog on boot"
        );
    }
}
