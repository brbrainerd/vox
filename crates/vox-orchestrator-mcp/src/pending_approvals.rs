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
//! Scope: in-memory only (lost on restart — a pending call then errors out).
//! Cross-process resolve for autonomous daemon agents (registry on
//! `Orchestrator` + an `orch.resolve_approval` RPC) and DB persistence are
//! deliberate follow-ups.

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

    /// Resolve a pending approval, waking its awaiter with `outcome`. Returns
    /// `false` if no pending approval has that id.
    pub fn resolve(&self, id: &str, outcome: ApprovalOutcome) -> bool {
        let mut g = self.inner.lock().expect("pending-approvals lock");
        g.pending.retain(|p| p.approval_id != id);
        match g.waiters.remove(id) {
            Some(tx) => {
                // Err only if the awaiter already gave up (e.g. timed out); the
                // approval is resolved regardless.
                let _ = tx.send(outcome);
                true
            }
            None => false,
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
