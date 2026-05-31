use std::sync::Arc;

use vox_orchestrator::ApprovalOutcome;
use vox_orchestrator_mcp::pending_approvals::PendingApprovals;
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, load_config};

#[tokio::test]
async fn register_then_resolve_wakes_the_awaiter() {
    let reg = PendingApprovals::default();
    let (id, rx) = reg.register("vox_write_file".to_string(), "write src/x.rs".to_string(), 1000);

    // Visible while pending.
    let listed = reg.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].approval_id, id);
    assert_eq!(listed[0].tool, "vox_write_file");

    // A separate task parks on the decision; resolving from here wakes it.
    let waiter = tokio::spawn(async move { rx.await });
    assert!(reg.resolve(&id, ApprovalOutcome::Approved));
    let outcome = waiter.await.expect("join").expect("sender not dropped");
    assert_eq!(outcome, ApprovalOutcome::Approved);

    // Resolved approvals leave the pending list.
    assert!(reg.list().is_empty());
}

#[tokio::test]
async fn resolve_unknown_id_returns_false() {
    let reg = PendingApprovals::default();
    assert!(!reg.resolve("AP-does-not-exist", ApprovalOutcome::Approved));
}

#[tokio::test]
async fn cancel_drops_the_pending_entry() {
    let reg = PendingApprovals::default();
    let (id, _rx) = reg.register("vox_deploy".to_string(), "deploy prod".to_string(), 1);
    assert_eq!(reg.list().len(), 1);
    reg.cancel(&id);
    assert!(reg.list().is_empty());
}

/// End-to-end gate: a dangerous tool without `user_approval` parks on a pending
/// approval; resolving it Rejected wakes the call with a non-approved error
/// envelope (and the action never executes).
#[tokio::test]
async fn dangerous_tool_parks_until_resolved() {
    let state = Arc::new(ServerState::new_full(load_config()));

    let s2 = state.clone();
    let call = tokio::spawn(async move {
        handle_tool_call(
            &s2,
            "vox_run_shell",
            serde_json::json!({ "command": "echo hi" }),
        )
        .await
    });

    // Wait until the gate registers the pending approval.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "dangerous tool never registered a pending approval"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "vox_run_shell");

    // Reject it; the parked call should wake and return an error envelope.
    assert!(state
        .pending_approvals
        .resolve(&pending[0].approval_id, ApprovalOutcome::Rejected));

    let raw = call.await.expect("join").expect("dispatch ok");
    assert!(
        tool_json_envelope_is_error(&raw),
        "rejected approval must yield an error envelope, got: {raw}"
    );
    assert!(state.pending_approvals.list().is_empty());
}
