//! T1.4 RED tests: pending approvals and open feedback requests that were
//! requested but never resolved before a restart are visible again after
//! `ServerState::with_db_initialized` re-runs against the same durable DB.
//!
//! Semantics under test are deliberately narrow: these tests prove
//! *visibility* (the item reappears and is resolvable) survives a restart,
//! NOT that the original in-flight tool call is resumed — there is no live
//! caller waiting on the other end after a real process restart, and these
//! tests do not pretend otherwise (see `hitl_rehydrate` module docs).

use std::sync::Arc;

use vox_orchestrator_mcp::ServerState;

/// RED test: a dangerous-tool call parks on an approval (durable
/// `ApprovalRequested` written, per T1.1); we never resolve it — simulating a
/// crash while it's still pending — then build a **fresh** `ServerState`
/// against the same durable DB (`with_db_initialized`, exactly the hook both
/// `vox mcp` stdio and `vox-orchestrator-d` run on boot) and assert the
/// approval is visible again in the new state's `pending_approvals.list()`
/// and can be resolved through it.
#[tokio::test]
async fn pending_approval_survives_restart_as_visible_and_resolvable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-approvals-restart.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local { path: db_path.to_string_lossy().to_string() })
            .await
            .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let s2 = Arc::new(state);
    let s3 = s2.clone();
    // Park a dangerous-tool call; never resolve it — simulated crash.
    let call = tokio::spawn(async move {
        vox_orchestrator_mcp::handle_tool_call(
            &s3,
            "vox_run_shell",
            serde_json::json!({ "command": "echo t14" }),
        )
        .await
    });

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if !s2.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "dangerous tool never registered a pending approval"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let before_restart = s2.pending_approvals.list();
    assert_eq!(before_restart.len(), 1);
    let approval_id = before_restart[0].approval_id.clone();

    // "Restart": build a brand-new ServerState (fresh PendingApprovals with
    // no waiter for approval_id, exactly like a real process restart) and
    // reattach the SAME durable DB via with_db_initialized.
    let restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let after_restart = restarted.pending_approvals.list();
    assert!(
        after_restart.iter().any(|p| p.approval_id == approval_id),
        "approval {approval_id} was requested but never resolved before the \
         simulated crash; it must be visible again after restart, got: {after_restart:?}"
    );

    // Resolvable: a human can still record a decision against the restored id.
    assert!(
        restarted
            .pending_approvals
            .resolve(&approval_id, vox_orchestrator::ApprovalOutcome::Rejected),
        "a restart-recovered approval must still be resolve()-able (audit \
         consistency), even though its original waiter is gone"
    );
    assert!(restarted.pending_approvals.list().is_empty());

    // Clean up the still-parked original call so the test process doesn't
    // leak a task awaiting a timeout; it has no live path to resolution
    // anymore (that's exactly the documented limitation), so just detach.
    call.abort();
    drop(dir);
}

/// RED test: an approval requested AND resolved before the crash must NOT
/// reappear after restart.
#[tokio::test]
async fn resolved_approval_does_not_reappear_after_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-approvals-resolved-restart.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local { path: db_path.to_string_lossy().to_string() })
            .await
            .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;
    let s2 = Arc::new(state);
    let s3 = s2.clone();
    let call = tokio::spawn(async move {
        vox_orchestrator_mcp::handle_tool_call(
            &s3,
            "vox_run_shell",
            serde_json::json!({ "command": "echo t14-resolved" }),
        )
        .await
    });

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if !s2.pending_approvals.list().is_empty() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "never parked");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pending = s2.pending_approvals.list();
    let approval_id = pending[0].approval_id.clone();
    assert!(
        s2.pending_approvals
            .resolve(&approval_id, vox_orchestrator::ApprovalOutcome::Rejected)
    );
    let _ = call.await;

    // Give the durable ApprovalResolved write a moment to land (dispatch
    // awaits it directly, but poll briefly for robustness against scheduling).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let entries = s2.orchestrator.list_recent_operations(None, 256).await;
        if entries.iter().any(|e| {
            matches!(
                &e.kind,
                vox_orchestrator::oplog::OperationKind::ApprovalResolved { approval_id: aid, .. }
                    if aid == &approval_id
            )
        }) {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "ApprovalResolved never landed durably");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;
    assert!(
        !restarted
            .pending_approvals
            .list()
            .iter()
            .any(|p| p.approval_id == approval_id),
        "a resolved approval must not reappear after restart"
    );
    drop(dir);
}
