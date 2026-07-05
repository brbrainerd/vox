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
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
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
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
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
        assert!(
            tokio::time::Instant::now() < deadline,
            "ApprovalResolved never landed durably"
        );
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

/// T1.6 follow-up regression (Bug 2, HIGH — HITL integrity regression):
/// `compact_now` used to prune `agent_oplog` rows purely by `operation_id <=
/// up_to`, with no awareness of whether an `ApprovalRequested` entry in that
/// range had a matching `ApprovalResolved`. Since checkpoint state only
/// captures task lifecycle (`OpenTaskState`), an unresolved approval whose
/// `Requested` row fell at or before `up_to` at compaction time was
/// permanently deleted with no trace — `hitl_rehydrate_on_restart`'s
/// full-history scan has no checkpoint awareness and simply wouldn't see
/// what had been pruned.
///
/// This records an `ApprovalRequested` with no matching `ApprovalResolved`,
/// pads with enough subsequent ops that `up_to` naturally includes the
/// unresolved approval's row, calls `compact_now`, then verifies the
/// unresolved approval is STILL findable via the normal
/// `with_db_initialized` -> `rehydrate_open_hitl_from_oplog` restart path
/// (not silently gone).
#[tokio::test]
async fn unresolved_approval_survives_compaction_and_is_rehydrated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t16-bug2-unresolved-approval.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let approval_id = "t16-bug2-approval-unresolved".to_string();
    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalRequested {
                approval_id: approval_id.clone(),
                tool: "vox_run_shell".to_string(),
                run_id: None,
            },
            "approval requested, never resolved (simulated crash)",
            None,
            None,
            None,
            None,
        )
        .await;

    // Pad with enough subsequent durable ops that `up_to` naturally covers
    // the unresolved approval's row (not just happens to sit right at the
    // boundary).
    for i in 0..20 {
        state
            .orchestrator
            .record_operation(
                vox_orchestrator_types::AgentId(0),
                vox_orchestrator::oplog::OperationKind::Custom {
                    label: format!("t16-bug2-pad-{i}"),
                },
                format!("padding op {i}"),
                None,
                None,
                None,
                None,
            )
            .await;
    }

    let repo = vox_orchestrator::lineage::repository_id();
    let up_to = db
        .max_agent_oplog_id(&repo)
        .await
        .expect("max_agent_oplog_id")
        .expect("should have durable rows by now");

    vox_orchestrator::orchestrator::core::checkpoint::compact_now(&state.orchestrator, up_to)
        .await
        .expect("compact_now should succeed");

    // The unresolved approval's row must still be present in agent_oplog —
    // not silently deleted by the compaction that just ran.
    let rows_after = db
        .list_oplog_entries(None, &repo, 10_000)
        .await
        .expect("list_oplog_entries");
    let approval_row_still_present = rows_after.iter().any(|r| {
        r.get(2)
            .and_then(|v| v.as_deref())
            .map(|k| k.contains(&approval_id))
            .unwrap_or(false)
    });
    assert!(
        approval_row_still_present,
        "unresolved ApprovalRequested row for {approval_id} must survive compaction \
         even though its operation_id <= up_to"
    );

    // "Restart": a fresh ServerState re-attached to the same durable DB must
    // still be able to rehydrate the unresolved approval via the normal
    // full-history scan path.
    let restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;
    let after_restart = restarted.pending_approvals.list();
    assert!(
        after_restart.iter().any(|p| p.approval_id == approval_id),
        "approval {approval_id} was unresolved at compaction time; it must still be \
         visible after a restart following compaction, got: {after_restart:?}"
    );
    drop(dir);
}

/// T1.6 follow-up positive test (Bug 2 overcorrection guard): a RESOLVED
/// approval whose row falls within the pruned range must still be pruned
/// normally — proving the Bug 2 fix excludes only genuinely open HITL rows,
/// not disabling pruning entirely.
#[tokio::test]
async fn resolved_approval_is_still_pruned_by_compaction() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t16-bug2-resolved-approval-pruned.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let approval_id = "t16-bug2-approval-resolved".to_string();
    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalRequested {
                approval_id: approval_id.clone(),
                tool: "vox_run_shell".to_string(),
                run_id: None,
            },
            "approval requested",
            None,
            None,
            None,
            None,
        )
        .await;
    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalResolved {
                approval_id: approval_id.clone(),
                outcome: "approved".to_string(),
                resolver: Some("test".to_string()),
            },
            "approval resolved",
            None,
            None,
            None,
            None,
        )
        .await;

    for i in 0..20 {
        state
            .orchestrator
            .record_operation(
                vox_orchestrator_types::AgentId(0),
                vox_orchestrator::oplog::OperationKind::Custom {
                    label: format!("t16-bug2-pad-resolved-{i}"),
                },
                format!("padding op {i}"),
                None,
                None,
                None,
                None,
            )
            .await;
    }

    let repo = vox_orchestrator::lineage::repository_id();
    let up_to = db
        .max_agent_oplog_id(&repo)
        .await
        .expect("max_agent_oplog_id")
        .expect("should have durable rows by now");

    vox_orchestrator::orchestrator::core::checkpoint::compact_now(&state.orchestrator, up_to)
        .await
        .expect("compact_now should succeed");

    let rows_after = db
        .list_oplog_entries(None, &repo, 10_000)
        .await
        .expect("list_oplog_entries");
    let resolved_rows_still_present = rows_after
        .iter()
        .filter(|r| {
            r.get(2)
                .and_then(|v| v.as_deref())
                .map(|k| k.contains(&approval_id))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        resolved_rows_still_present, 0,
        "a fully-resolved approval's rows must be pruned normally, not preserved \
         as if it were still open (overcorrection guard)"
    );
}

/// RED test: a soft-HITL clarification request (`ask_clarification`, which
/// durably records `FeedbackRequested` per T1.1/T1.2) that is never resolved
/// before a simulated crash is visible again in `FeedbackStore::open_needs_you()`
/// after restart, with the SAME request id.
#[tokio::test]
async fn open_feedback_request_survives_restart_as_visible() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-feedback-restart.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let raw = vox_orchestrator_mcp::feedback_tools::ask_clarification(
        &state,
        vox_orchestrator_mcp::params::AskClarificationParams {
            prompt: "which schema should this use?".to_string(),
            options: vec!["a".to_string(), "b".to_string()],
            gates: vec![],
            session_id: None,
        },
    )
    .await;
    // ask_clarification returns a JSON envelope; just confirm it registered
    // something rather than parsing its exact shape (not this test's concern).
    assert!(!raw.is_empty());

    // Never resolved — simulated crash right here.
    let before_restart = state.feedback().open_needs_you();
    assert_eq!(
        before_restart.len(),
        1,
        "expected exactly one open clarification before the simulated crash \
         (attention policy may route to Withheld under some configs — if this \
         assertion trips in CI, check open_needs_you() vs withheld())"
    );
    let request_id = before_restart[0].id.clone();

    // "Restart": fresh ServerState re-attached to the same durable DB.
    let restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let after_restart = restarted.feedback().open_needs_you();
    assert!(
        after_restart.iter().any(|f| f.id == request_id),
        "clarification {request_id:?} was requested but never resolved before \
         the simulated crash; it must be visible again after restart, got: {after_restart:?}"
    );
    drop(dir);
}

/// T1.4 follow-up: a `hitl_approvals` (DB audit table) row stuck at
/// `status = 'pending'` with NO matching oplog `ApprovalRequested` at all
/// (simulating a row that predates recorded oplog history, or whose durable
/// oplog write never landed) has no discoverable resolution and no
/// oplog-derived "still open" signal either — it must be reconciled to
/// `orphaned` on the next boot via the real `with_db_initialized` ->
/// `rehydrate_open_hitl_from_oplog` path, not left stuck `pending` forever.
#[tokio::test]
async fn stale_pending_audit_row_with_no_oplog_trace_is_marked_orphaned_on_boot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-followup-orphaned.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    // Write directly to the audit table only — no oplog ApprovalRequested/
    // ApprovalResolved at all — simulating a row the oplog-derived scan will
    // never consider "open" and can never backfill a resolution for.
    db.hitl_approval_record(
        "t14-followup-orphan-1",
        "vox_run_shell",
        "echo orphan",
        1_000,
    )
    .await
    .expect("record pending row");

    let before = db
        .hitl_approval_get("t14-followup-orphan-1")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(before.status, "pending");

    // Boot: this is the real rehydration hook, not a unit-tested helper in
    // isolation — the same hook `vox mcp` stdio and `vox-orchestrator-d` run.
    let _restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let after = db
        .hitl_approval_get("t14-followup-orphan-1")
        .await
        .expect("get")
        .expect("still present");
    assert_eq!(
        after.status, "orphaned",
        "a hitl_approvals row stuck 'pending' with no discoverable oplog \
         resolution and no oplog-derived open entry must be reconciled to \
         'orphaned' on boot, got status={:?}",
        after.status
    );
    assert!(
        after.resolved_at_ms.is_some(),
        "an orphaned row must record when it was reconciled"
    );
    drop(dir);
}

/// T1.4 follow-up: a `hitl_approvals` row stuck at `status = 'pending'`
/// whose approval WAS actually resolved via the durable oplog (the
/// `hitl_approval_resolve` audit-table write raced or failed independently
/// of the oplog `ApprovalResolved` write) must be backfilled from the oplog
/// entry on the next boot — not guessed at, and not left `pending`.
#[tokio::test]
async fn stale_pending_audit_row_with_oplog_resolution_is_backfilled_on_boot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-followup-backfill.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let approval_id = "t14-followup-backfill-1".to_string();

    // Durable oplog: both Requested and Resolved land normally.
    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalRequested {
                approval_id: approval_id.clone(),
                tool: "vox_run_shell".to_string(),
                run_id: None,
            },
            "approval requested",
            None,
            None,
            None,
            None,
        )
        .await;
    // Audit table: record the request (mirrors dispatch.rs's `hitl_approval_record`
    // call) but DO NOT call `hitl_approval_resolve` — simulating that
    // best-effort audit-table write failing/racing independently of the
    // durable oplog write below.
    db.hitl_approval_record(&approval_id, "vox_run_shell", "echo backfill", 1_000)
        .await
        .expect("record pending row");

    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalResolved {
                approval_id: approval_id.clone(),
                outcome: "approved".to_string(),
                resolver: Some("test".to_string()),
            },
            "approval resolved",
            None,
            None,
            None,
            None,
        )
        .await;

    // Confirm the audit row is still stuck 'pending' pre-boot (the gap this
    // test targets).
    let before = db
        .hitl_approval_get(&approval_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(before.status, "pending");

    // "Restart": fresh ServerState re-attached to the same durable DB — the
    // real rehydration path.
    let _restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let after = db
        .hitl_approval_get(&approval_id)
        .await
        .expect("get")
        .expect("still present");
    assert_eq!(
        after.status, "approved",
        "a hitl_approvals row stuck 'pending' whose approval WAS resolved per \
         the durable oplog must be backfilled to that real outcome on boot, \
         got status={:?}",
        after.status
    );
    assert!(
        after.resolved_at_ms.is_some(),
        "a backfilled row must carry the oplog entry's resolved_at_ms"
    );
    drop(dir);
}

/// T1.4 follow-up overcorrection guard: a `hitl_approvals` row that is
/// genuinely still open (oplog `ApprovalRequested` with no matching
/// `ApprovalResolved`) must NOT be touched by the reconciliation pass — it
/// stays `pending` and is also visible again via `PendingApprovals`, same as
/// the existing T1.4 restart-visibility behavior.
#[tokio::test]
async fn genuinely_open_pending_audit_row_is_not_reconciled_on_boot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t14-followup-still-open.db");
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Local {
            path: db_path.to_string_lossy().to_string(),
        })
        .await
        .expect("open db"),
    );

    let state = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let approval_id = "t14-followup-still-open-1".to_string();
    state
        .orchestrator
        .record_operation(
            vox_orchestrator_types::AgentId(0),
            vox_orchestrator::oplog::OperationKind::ApprovalRequested {
                approval_id: approval_id.clone(),
                tool: "vox_run_shell".to_string(),
                run_id: None,
            },
            "approval requested, never resolved",
            None,
            None,
            None,
            None,
        )
        .await;
    db.hitl_approval_record(&approval_id, "vox_run_shell", "echo still-open", 1_000)
        .await
        .expect("record pending row");

    let _restarted = ServerState::new_full(vox_orchestrator_mcp::load_config())
        .with_db_initialized(db.clone())
        .await;

    let after = db
        .hitl_approval_get(&approval_id)
        .await
        .expect("get")
        .expect("still present");
    assert_eq!(
        after.status, "pending",
        "a genuinely still-open approval's audit row must remain 'pending', \
         not be reconciled away, got status={:?}",
        after.status
    );
    assert!(after.resolved_at_ms.is_none());
    drop(dir);
}
