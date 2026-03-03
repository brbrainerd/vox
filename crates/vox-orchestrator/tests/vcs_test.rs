//! End-to-end integration test for JJ-inspired VCS features.
//!
//! Exercises the full lifecycle: workspace → snapshot → oplog → conflict → resolve.

use std::path::PathBuf;
use vox_orchestrator::{
    conflicts::ConflictResolution, snapshot::SnapshotId, workspace::ChangeStatus, AgentId,
    FileAffinity, Orchestrator, OrchestratorConfig,
};

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::for_testing()
}

#[tokio::test]
async fn vcs_lifecycle_snapshot_oplog_conflict() {
    let mut orch = Orchestrator::new(test_config());

    // Submit a task to auto-create an agent
    let task_id = orch
        .submit_task(
            "initial task",
            vec![FileAffinity::write("src/lib.rs")],
            None,
        )
        .await
        .expect("submit should succeed");
    let agent_a = *orch.agent_ids().first().expect("should have an agent");
    orch.complete_task(task_id).await
        .expect("complete should succeed");

    // 1. Create workspace for agent A
    let snap_id = orch.snapshot_store_mut().take_snapshot(
        agent_a,
        &[PathBuf::from("src/lib.rs")],
        "initial".to_string(),
    );
    orch.workspace_manager_mut()
        .create_workspace(agent_a, snap_id);
    assert!(orch.workspace_manager().has_workspace(agent_a));

    // 2. Submit a second task — should capture pre-task snapshot and record in oplog
    let task_id = orch
        .submit_task("test task", vec![FileAffinity::write("src/lib.rs")], None)
        .await
        .expect("submit should succeed");

    // Oplog should have at least one TaskSubmit entry
    {
        let ops = orch.oplog().list(Some(agent_a), 10);
        assert!(
            ops.iter().any(|op| matches!(
                &op.kind,
                vox_orchestrator::oplog::OperationKind::TaskSubmit { .. }
            )),
            "oplog should contain TaskSubmit"
        );
    }

    // Snapshot store should have at least 2 snapshots (initial + pre-task)
    assert!(orch.snapshot_store().count() >= 2);

    // 3. Complete the task — should capture post-task snapshot
    orch.complete_task(task_id).await
        .expect("complete should succeed");

    {
        let ops_after = orch.oplog().list(Some(agent_a), 10);
        assert!(
            ops_after.iter().any(|op| matches!(
                &op.kind,
                vox_orchestrator::oplog::OperationKind::TaskComplete { .. }
            )),
            "oplog should contain TaskComplete"
        );
    }

    // 4. Create a change and verify lifecycle
    let change_id = orch
        .workspace_manager_mut()
        .create_change(agent_a, "Fix parser bug");
    orch.workspace_manager_mut()
        .add_snapshot_to_change(change_id, snap_id);
    {
        let change = orch
            .workspace_manager()
            .get_change(change_id)
            .expect("change exists");
        assert_eq!(change.status, ChangeStatus::InProgress);
        assert_eq!(change.snapshots.len(), 1);
    }

    orch.workspace_manager_mut()
        .update_change_status(change_id, ChangeStatus::Merged);
    {
        let change = orch
            .workspace_manager()
            .get_change(change_id)
            .expect("change exists");
        assert_eq!(change.status, ChangeStatus::Merged);
    }

    // 5. Test conflict detection manually
    let conflict_id = orch.conflict_manager_mut().record_conflict(
        "shared.rs",
        Some(snap_id),
        vec![(AgentId(1), snap_id), (AgentId(2), snap_id)],
    );
    assert_eq!(orch.conflict_manager().active_count(), 1);

    // Resolve it
    orch.conflict_manager_mut()
        .resolve(conflict_id, ConflictResolution::TakeLeft);
    assert_eq!(orch.conflict_manager().active_count(), 0);

    // 6. Verify oplog undo/redo
    let first_op = {
        let ops = orch.oplog().list(None, 100);
        ops[ops.len() - 1].id
    };
    let snap_before = orch.oplog_mut().undo(first_op);
    assert!(snap_before.is_some(), "undo should return snapshot_before");

    let snap_after = orch.oplog_mut().redo(first_op);
    assert!(snap_after.is_some(), "redo should return snapshot_after");
}

#[test]
fn vcs_rebalance_records_oplog() {
    let config = OrchestratorConfig::for_testing();
    let mut orch = Orchestrator::new(config);

    let initial_oplog = orch.oplog().count();
    let moved = orch.rebalance();

    // If nothing was moved, oplog shouldn't change
    if moved == 0 {
        assert_eq!(orch.oplog().count(), initial_oplog);
    }
    // If tasks were moved, a Rebalance entry should appear
    if moved > 0 {
        let ops = orch.oplog().list(None, 10);
        assert!(
            ops.iter()
                .any(|op| matches!(&op.kind, vox_orchestrator::oplog::OperationKind::Rebalance)),
            "oplog should contain Rebalance"
        );
    }
}

#[test]
fn vcs_workspace_overlap_detection() {
    let mut orch = Orchestrator::new(test_config());

    let agent_a = AgentId(1);
    let agent_b = AgentId(2);
    let base = SnapshotId(1);

    // Create workspaces for both agents
    orch.workspace_manager_mut().create_workspace(agent_a, base);
    orch.workspace_manager_mut().create_workspace(agent_b, base);

    // Both modify the same file
    orch.workspace_manager_mut()
        .get_workspace_mut(agent_a)
        .unwrap()
        .record_modification("shared.rs", "hash_a".into());
    orch.workspace_manager_mut()
        .get_workspace_mut(agent_b)
        .unwrap()
        .record_modification("shared.rs", "hash_b".into());

    // Overlaps should be detected
    let overlaps = orch.workspace_manager().overlapping_paths(agent_a, agent_b);
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0], PathBuf::from("shared.rs"));

    // Non-overlapping files
    orch.workspace_manager_mut()
        .get_workspace_mut(agent_a)
        .unwrap()
        .record_modification("unique_a.rs", "hash_c".into());
    let overlaps = orch.workspace_manager().overlapping_paths(agent_a, agent_b);
    assert_eq!(overlaps.len(), 1); // Still only shared.rs
}
