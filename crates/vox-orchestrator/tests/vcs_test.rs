//! End-to-end integration test for JJ-inspired VCS features.
//!
//! Exercises the full lifecycle: workspace → snapshot → oplog → conflict → resolve.

use std::path::PathBuf;
use vox_orchestrator::{
    AgentId, CompletionAttestation, FileAffinity, Orchestrator, OrchestratorConfig, TaskId,
    conflicts::ConflictResolution, snapshot::SnapshotId, workspace::ChangeStatus,
};

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::for_testing()
}

fn completion_attestation_for_tests() -> CompletionAttestation {
    CompletionAttestation {
        checks_passed: vec!["human_review_approved".to_string()],
        ..Default::default()
    }
}

/// `complete_task*` requires the task to be dequeued into `in_progress` (see `orch_smoke::complete_task_flow`).
async fn dequeue_and_complete(orch: &Orchestrator, agent_id: AgentId, task_id: TaskId) {
    orch.agent_queue(agent_id)
        .expect("agent queue")
        .write()
        .unwrap()
        .dequeue();
    orch.complete_task_with_attestation(task_id, Some(completion_attestation_for_tests()))
        .await
        .expect("complete should succeed");
}

#[tokio::test]
async fn vcs_lifecycle_snapshot_oplog_conflict() {
    let orch = Orchestrator::new(test_config());

    // Submit a task to auto-create an agent
    let task_id = orch
        .submit_task(
            "initial task",
            vec![FileAffinity::write("src/lib.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");
    let agent_a = *orch.agent_ids().first().expect("should have an agent");
    dequeue_and_complete(&orch, agent_a, task_id).await;

    // 1. Create workspace for agent A
    let ss_lock = orch.snapshot_store_mut();
    let snap_id = ss_lock.write().unwrap().take_snapshot(
        agent_a,
        &[PathBuf::from("src/lib.rs")],
        "initial".to_string(),
    );
    let wm_lock = orch.workspace_manager_mut();
    wm_lock.write().unwrap().create_workspace(agent_a, snap_id);
    assert!(wm_lock.read().unwrap().has_workspace(agent_a));

    // 2. Submit a second task — should capture pre-task snapshot and record in oplog
    let task_id = orch
        .submit_task(
            "test task",
            vec![FileAffinity::write("src/lib.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    // Oplog should have at least one TaskSubmit entry
    {
        let oplog = orch.oplog_mut();
        let guard = oplog.read().unwrap();
        let ops = guard.list(Some(agent_a), 10);
        assert!(
            ops.iter().any(|op| matches!(
                &op.kind,
                vox_orchestrator::oplog::OperationKind::TaskSubmit { .. }
            )),
            "oplog should contain TaskSubmit"
        );
    }

    // Snapshot store should have at least 2 snapshots (initial + pre-task)
    assert!(ss_lock.read().unwrap().count() >= 2);

    // 3. Complete the task — should capture post-task snapshot
    dequeue_and_complete(&orch, agent_a, task_id).await;

    {
        let oplog = orch.oplog_mut();
        let guard = oplog.read().unwrap();
        let ops_after = guard.list(Some(agent_a), 10);
        assert!(
            ops_after.iter().any(|op| matches!(
                &op.kind,
                vox_orchestrator::oplog::OperationKind::TaskComplete { .. }
            )),
            "oplog should contain TaskComplete"
        );
    }

    // 4. Create a change and verify lifecycle
    let wm_lock = orch.workspace_manager_mut();
    let change_id = wm_lock
        .write()
        .unwrap()
        .create_change(agent_a, "Fix parser bug");
    wm_lock
        .write()
        .unwrap()
        .add_snapshot_to_change(change_id, snap_id);
    {
        let manager = wm_lock.read().unwrap();
        let change = manager.get_change(change_id).expect("change exists");
        assert_eq!(change.status, ChangeStatus::InProgress);
        assert_eq!(change.snapshots.len(), 1);
    }

    wm_lock
        .write()
        .unwrap()
        .update_change_status(change_id, ChangeStatus::Merged);
    {
        let manager = wm_lock.read().unwrap();
        let change = manager.get_change(change_id).expect("change exists");
        assert_eq!(change.status, ChangeStatus::Merged);
    }

    // 5. Test conflict detection manually
    let cm_lock = orch.conflict_manager_mut();
    let conflict_id = cm_lock.write().unwrap().record_conflict(
        "shared.rs",
        Some(snap_id),
        vec![(AgentId(1), snap_id), (AgentId(2), snap_id)],
    );
    assert_eq!(cm_lock.read().unwrap().active_count(), 1);

    // Resolve it
    cm_lock
        .write()
        .unwrap()
        .resolve(conflict_id, ConflictResolution::TakeLeft);
    assert_eq!(cm_lock.read().unwrap().active_count(), 0);

    // 6. Verify oplog undo/redo
    let oplog_lock = orch.oplog_mut();
    let first_op = {
        let guard = oplog_lock.read().unwrap();
        let ops = guard.list(None, 100);
        ops[ops.len() - 1].id
    };
    let snap_before = oplog_lock.write().unwrap().undo(first_op);
    assert!(snap_before.is_some(), "undo should return snapshot_before");

    let snap_after = oplog_lock.write().unwrap().redo(first_op);
    assert!(snap_after.is_some(), "redo should return snapshot_after");
}

#[tokio::test]
async fn vcs_rebalance_records_oplog() {
    let config = OrchestratorConfig::for_testing();
    let orch = Orchestrator::new(config);

    let oplog_lock = orch.oplog_mut();
    let initial_oplog = oplog_lock.read().unwrap().count();
    let moved = orch.rebalance();

    // If nothing was moved, oplog shouldn't change
    if moved == 0 {
        assert_eq!(oplog_lock.read().unwrap().count(), initial_oplog);
    }
    // If tasks were moved, a Rebalance entry should appear
    if moved > 0 {
        let guard = oplog_lock.read().unwrap();
        let ops = guard.list(None, 10);
        assert!(
            ops.iter()
                .any(|op| matches!(&op.kind, vox_orchestrator::oplog::OperationKind::Rebalance)),
            "oplog should contain Rebalance"
        );
    }
}

#[tokio::test]
async fn vcs_workspace_overlap_detection() {
    let orch = Orchestrator::new(test_config());

    let agent_a = AgentId(1);
    let agent_b = AgentId(2);
    let base = SnapshotId(1);

    // Create workspaces for both agents
    let wm_lock = orch.workspace_manager_mut();
    wm_lock.write().unwrap().create_workspace(agent_a, base);
    wm_lock.write().unwrap().create_workspace(agent_b, base);

    // Both modify the same file
    {
        let mut manager = wm_lock.write().unwrap();
        manager
            .get_workspace_mut(agent_a)
            .unwrap()
            .record_modification("shared.rs", "hash_a".into());
        manager
            .get_workspace_mut(agent_b)
            .unwrap()
            .record_modification("shared.rs", "hash_b".into());
    }

    // Overlaps should be detected
    let overlaps = wm_lock.read().unwrap().overlapping_paths(agent_a, agent_b);
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0], PathBuf::from("shared.rs"));

    // Non-overlapping files
    wm_lock
        .write()
        .unwrap()
        .get_workspace_mut(agent_a)
        .unwrap()
        .record_modification("unique_a.rs", "hash_c".into());
    let overlaps = wm_lock.read().unwrap().overlapping_paths(agent_a, agent_b);
    assert_eq!(overlaps.len(), 1); // Still only shared.rs
}
