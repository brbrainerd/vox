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
//! ## T1.4 follow-up: `hitl_approvals` audit-table reconciliation
//!
//! The in-memory `PendingApprovals` restore above is the live-functional-state
//! path and was already correct. Separately, `hitl_approvals` (the DB-persisted
//! audit table, `crates/vox-db/src/facade/hitl_approvals.rs`) can accumulate
//! rows stuck at `status = 'pending'` forever: `hitl_approval_resolve` is the
//! only thing that ever transitions a row out of `pending`, and it is called
//! from exactly one call site (`dispatch.rs`'s dangerous-tool gate) — a
//! best-effort write that can fail independently of the durable oplog write,
//! or simply never run again for an approval whose live process died before
//! resolving. On its own such a row is an accurate historical record ("this
//! approval was requested and never resolved by the process that requested
//! it") but becomes *misleading* once this module's oplog-derived open set
//! says otherwise for the current restart. `reconcile_hitl_approvals_table`
//! below closes that gap for two cases:
//! - The audit table missed an `ApprovalResolved` that the oplog *does* have
//!   (the resolve write raced/failed) — backfill the row from the oplog entry
//!   rather than guessing.
//! - No such oplog entry exists (row predates recorded history, or the
//!   approval was cancelled/dropped without ever recording a resolution) —
//!   mark the row `orphaned` so "what's pending right now" queries against
//!   the audit table stay accurate.
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
    let entries = match state
        .orchestrator
        .list_recent_operations(None, 100_000)
        .await
    {
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
    // approval_id -> (outcome, resolved_at_ms) for every ApprovalResolved seen
    // in the scanned oplog window, open or not — used below to backfill
    // `hitl_approvals` rows the audit-table write missed (T1.4 follow-up).
    let mut approvals_resolved: HashMap<String, (String, u64)> = HashMap::new();

    // Both `list_recent_operations` and `list_from_db` return newest-first;
    // fold oldest-first so a Requested/Resolved pair is applied in the order
    // it actually happened (insert-then-remove), not backwards.
    for entry in entries.iter().rev() {
        match &entry.kind {
            OperationKind::ApprovalRequested {
                approval_id, tool, ..
            } => {
                approvals_open.insert(approval_id.clone(), (tool.clone(), entry.timestamp_ms));
            }
            OperationKind::ApprovalResolved {
                approval_id,
                outcome,
                ..
            } => {
                approvals_open.remove(approval_id);
                approvals_resolved
                    .insert(approval_id.clone(), (outcome.clone(), entry.timestamp_ms));
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
    let approvals_open_ids: std::collections::HashSet<String> =
        approvals_open.keys().cloned().collect();
    for (approval_id, (tool, requested_at_ms)) in approvals_open {
        let summary = format!("[recovered-on-restart] {tool}");
        state.pending_approvals.reregister_after_restart(
            approval_id,
            tool,
            summary,
            requested_at_ms,
        );
    }

    // T1.4 follow-up: reconcile `hitl_approvals` (DB audit table) rows stuck
    // at `status = 'pending'` that this restart's oplog-derived open set does
    // NOT consider open — either backfill their real resolution from the
    // oplog, or mark them `orphaned` if no resolution is discoverable.
    if let Some(db) = state.db.as_ref() {
        reconcile_hitl_approvals_table(db, &approvals_open_ids, &approvals_resolved).await;
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

/// T1.4 follow-up: reconcile `hitl_approvals` (DB audit table) rows still
/// `status = 'pending'` against `approvals_open_ids` (this restart's
/// oplog-derived open set, already re-registered into `PendingApprovals`
/// above) and `approvals_resolved` (every `ApprovalResolved` seen in the same
/// scanned oplog window, keyed by `approval_id`).
///
/// For each `pending` row NOT in `approvals_open_ids`:
/// - if `approvals_resolved` has an entry for it, the audit-table write at
///   resolve time evidently missed or raced — backfill `status`/
///   `resolved_at_ms` from the oplog entry (the durable source of truth)
///   rather than guessing.
/// - otherwise there is no discoverable resolution — mark the row
///   `orphaned` so it stops appearing as falsely "still pending".
///
/// Best-effort: a listing or write failure logs a warning and leaves the
/// affected row(s) as-is rather than failing MCP/daemon startup, matching
/// this module's existing best-effort posture for the in-memory restore.
async fn reconcile_hitl_approvals_table(
    db: &vox_db::VoxDb,
    approvals_open_ids: &std::collections::HashSet<String>,
    approvals_resolved: &HashMap<String, (String, u64)>,
) {
    let pending_rows = match db.hitl_approvals_pending().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("T1.4 follow-up: failed to list pending hitl_approvals rows: {e}");
            return;
        }
    };

    let mut backfilled = 0u64;
    let mut orphaned = 0u64;
    for row in pending_rows {
        if approvals_open_ids.contains(&row.approval_id) {
            // Still genuinely open per this restart's oplog scan — leave it
            // `pending`; it was just re-registered into PendingApprovals above.
            continue;
        }
        if let Some((outcome, resolved_at_ms)) = approvals_resolved.get(&row.approval_id) {
            match db
                .hitl_approval_resolve(&row.approval_id, outcome, *resolved_at_ms as i64)
                .await
            {
                Ok(()) => backfilled += 1,
                Err(e) => tracing::warn!(
                    approval_id = %row.approval_id,
                    "T1.4 follow-up: failed to backfill hitl_approvals resolution from oplog: {e}"
                ),
            }
        } else {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            match db
                .hitl_approval_mark_orphaned(&row.approval_id, now_ms)
                .await
            {
                Ok(()) => orphaned += 1,
                Err(e) => tracing::warn!(
                    approval_id = %row.approval_id,
                    "T1.4 follow-up: failed to mark orphaned hitl_approvals row: {e}"
                ),
            }
        }
    }

    if backfilled > 0 || orphaned > 0 {
        tracing::info!(
            backfilled,
            orphaned,
            "T1.4 follow-up: reconciled stale-pending hitl_approvals rows on boot"
        );
    }
}
