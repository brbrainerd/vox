//! B3 HITL: in-memory pending-approval registry.
//!
//! A dangerous MCP tool call that lacks pre-supplied `user_approval: true` no
//! longer rejects outright (see `dispatch.rs`) — it `register`s a pending
//! approval and `.await`s the returned receiver. A human resolves it via the
//! `vox_resolve_approval` tool (which calls [`PendingApprovals::resolve`]),
//! waking the parked tool call with an [`ApprovalOutcome`]. The registry lives
//! on `ServerState`, so the awaiting tool call and the resolve/list tools all
//! share it (in-process; the GUI drives both through B5's `invoke_mcp_tool`).
//!
//! Scope: in-memory only (lost on restart — a pending call then errors out;
//! T1.4's [`reregister_after_restart`](PendingApprovals::reregister_after_restart)
//! restores *visibility* of pre-restart entries but not their live waiter).
//! Cross-process access for autonomous daemon agents and the GUI is served by
//! the daemon's `orch.list_pending_approvals` / `orch.resolve_approval` RPCs
//! (see `daemon_extra.rs`'s `ExtraDispatch` impl, which reads/writes this same
//! registry off `ServerState`) — implemented, not a follow-up. DB persistence
//! of resolved approvals (for audit/history beyond the in-memory list) remains
//! a deliberate follow-up.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;
use vox_orchestrator::ApprovalOutcome;

/// Opaque id for one pending approval (e.g. `"AP-000001"`).
pub type ApprovalId = String;

/// Operator-facing description of a pending approval (returned by `list`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingApprovalInfo {
    /// Stable id used to resolve this approval.
    pub approval_id: ApprovalId,
    /// Canonical MCP tool name awaiting approval.
    pub tool: String,
    /// Short human-readable summary of what will run.
    pub summary: String,
    /// When the approval was requested (unix-ms).
    pub requested_at_ms: u64,
}

#[derive(Default)]
struct Inner {
    next: u64,
    waiters: HashMap<ApprovalId, oneshot::Sender<ApprovalOutcome>>,
    pending: Vec<PendingApprovalInfo>,
}

/// Shared registry of approvals awaiting a human decision.
#[derive(Default)]
pub struct PendingApprovals {
    inner: Mutex<Inner>,
}

impl PendingApprovals {
    /// Register a new pending approval. Returns its id and a receiver the tool
    /// call awaits; the receiver resolves when [`resolve`](Self::resolve) is
    /// called for this id (or errors if the entry is cancelled/dropped).
    pub fn register(
        &self,
        tool: String,
        summary: String,
        requested_at_ms: u64,
    ) -> (ApprovalId, oneshot::Receiver<ApprovalOutcome>) {
        let (tx, rx) = oneshot::channel();
        let mut g = self.inner.lock().expect("pending-approvals lock");
        g.next += 1;
        let id = format!("AP-{:06}", g.next);
        g.waiters.insert(id.clone(), tx);
        g.pending.push(PendingApprovalInfo {
            approval_id: id.clone(),
            tool,
            summary,
            requested_at_ms,
        });
        (id, rx)
    }

    /// T1.4: re-park an approval that was requested (per the durable op-log's
    /// `ApprovalRequested`) but never resolved before a daemon restart. Unlike
    /// [`register`](Self::register), the id is **preserved** from the original
    /// request (so `vox_resolve_approval` still targets the same
    /// `approval_id` a human may already be looking at in `hitl_approvals`)
    /// and there is deliberately **no oneshot waiter** — the original tool
    /// call that parked on it died with the process and cannot be woken. This
    /// restores *visibility* (the entry reappears in `list()` and can be
    /// `resolve()`d for audit-consistency / human record-keeping) without
    /// pretending the original in-flight call can be resumed. A no-op if an
    /// entry with this id is already pending (idempotent under repeated
    /// rehydration calls).
    pub fn reregister_after_restart(
        &self,
        approval_id: ApprovalId,
        tool: String,
        summary: String,
        requested_at_ms: u64,
    ) {
        let mut g = self.inner.lock().expect("pending-approvals lock");
        if g.pending.iter().any(|p| p.approval_id == approval_id) {
            return;
        }
        g.pending.push(PendingApprovalInfo {
            approval_id,
            tool,
            summary,
            requested_at_ms,
        });
    }

    /// Resolve a pending approval, waking its awaiter with `outcome` if one is
    /// still parked. Returns `true` whenever a pending entry with this id
    /// existed and was removed — including a restart-recovered entry with no
    /// live waiter (see
    /// [`reregister_after_restart`](Self::reregister_after_restart)), since
    /// the approval decision is recorded either way; only `false` when no
    /// pending approval has that id at all.
    pub fn resolve(&self, id: &str, outcome: ApprovalOutcome) -> bool {
        let mut g = self.inner.lock().expect("pending-approvals lock");
        let before = g.pending.len();
        g.pending.retain(|p| p.approval_id != id);
        let had_pending = g.pending.len() != before;
        match g.waiters.remove(id) {
            Some(tx) => {
                // Err only if the awaiter already gave up (e.g. timed out); the
                // approval is resolved regardless.
                let _ = tx.send(outcome);
                true
            }
            // No live waiter (e.g. a restart-recovered entry) — still counts
            // as resolved as long as a pending entry actually existed.
            None => had_pending,
        }
    }

    /// Drop a pending approval without an outcome (e.g. the awaiter timed out).
    pub fn cancel(&self, id: &str) {
        let mut g = self.inner.lock().expect("pending-approvals lock");
        g.pending.retain(|p| p.approval_id != id);
        g.waiters.remove(id);
    }

    /// Snapshot of approvals currently awaiting a decision.
    pub fn list(&self) -> Vec<PendingApprovalInfo> {
        self.inner
            .lock()
            .expect("pending-approvals lock")
            .pending
            .clone()
    }
}

/// Map a human's `vox_resolve_approval` decision string to an [`ApprovalOutcome`].
///
/// `approve`/`approved` → `Approved`, `modify`/`modified` → `Modified`,
/// anything else (including the empty string) → `Rejected`. Lives here rather
/// than inline in the dispatch match arm so the `command-compliance` handler
/// scraper never mistakes these string-literal match arms for tool names.
pub fn outcome_from_decision(decision: &str) -> ApprovalOutcome {
    match decision {
        "approve" | "approved" => ApprovalOutcome::Approved,
        "modify" | "modified" => ApprovalOutcome::Modified,
        _ => ApprovalOutcome::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_from_decision_maps_known_and_unknown() {
        assert_eq!(outcome_from_decision("approve"), ApprovalOutcome::Approved);
        assert_eq!(outcome_from_decision("approved"), ApprovalOutcome::Approved);
        assert_eq!(outcome_from_decision("modify"), ApprovalOutcome::Modified);
        assert_eq!(outcome_from_decision("modified"), ApprovalOutcome::Modified);
        assert_eq!(outcome_from_decision("reject"), ApprovalOutcome::Rejected);
        assert_eq!(outcome_from_decision(""), ApprovalOutcome::Rejected);
        assert_eq!(outcome_from_decision("garbage"), ApprovalOutcome::Rejected);
    }
}
