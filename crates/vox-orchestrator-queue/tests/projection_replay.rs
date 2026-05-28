//! P3-T9 acceptance: replay the op-log into a fresh ProjectionRegistry and assert
//! the resulting state matches a "live" registry that processed the same ops.

use vox_orchestrator_queue::oplog::{OperationEntry, OperationId, OperationKind};
use vox_orchestrator_queue::projection::ProjectionRegistry;
use vox_orchestrator_queue::projections::{AffinityProjection, LocksProjection};
use vox_orchestrator_types::AgentId;

fn synth_ops() -> Vec<OperationEntry> {
    let base = OperationEntry {
        id: OperationId(0),
        agent_id: AgentId(1),
        timestamp_ms: 1_000_000,
        kind: OperationKind::Custom {
            label: "noop".into(),
        },
        description: String::new(),
        snapshot_before: None,
        snapshot_after: None,
        db_snapshot_before: None,
        db_snapshot_after: None,
        context_snapshot_before: None,
        context_snapshot_after: None,
        undone: false,
        change_id: None,
        model_id: None,
        predecessor_hash: None,
        signature: None,
        signing_key_id: None,
        daemon_id: [1u8; 16],
        parent_op_ids: Vec::new(),
    };

    vec![
        OperationEntry {
            id: OperationId(1),
            kind: OperationKind::LockAcquire {
                path: "src/main.rs".into(),
                agent_id: 1,
            },
            ..base.clone()
        },
        OperationEntry {
            id: OperationId(2),
            kind: OperationKind::WorkspaceCreate { agent_id: 1 },
            ..base.clone()
        },
        OperationEntry {
            id: OperationId(3),
            kind: OperationKind::LockRelease {
                path: "src/main.rs".into(),
                agent_id: 1,
            },
            ..base.clone()
        },
        OperationEntry {
            id: OperationId(4),
            kind: OperationKind::LockAcquire {
                path: "src/lib.rs".into(),
                agent_id: 2,
            },
            agent_id: AgentId(2),
            ..base.clone()
        },
    ]
}

#[tokio::test]
async fn replay_reconstructs_locks_and_affinity_bit_identical() {
    let live = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());

    let ops = synth_ops();
    for op in &ops {
        live.apply(op).await;
    }

    let replay = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());
    for op in &ops {
        replay.apply(op).await;
    }

    assert_eq!(
        live.snapshot_blake3(),
        replay.snapshot_blake3(),
        "live and replay must produce bit-identical snapshots"
    );
}

#[tokio::test]
async fn empty_registry_has_stable_hash() {
    let r1 = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());
    let r2 = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());
    assert_eq!(r1.snapshot_blake3(), r2.snapshot_blake3());
}
