//! T1.6 RED tests: `checkpoint::compact_now` produces a real, restorable
//! checkpoint and prunes warm-tier rows below it; a fresh registry hydrated
//! from the checkpoint blob matches the pre-compaction registry's hash.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator_queue::oplog::checkpoint::{compact_now, hydrate_from_checkpoint};
use vox_orchestrator_queue::oplog::{OpLog, OperationId, OperationKind};
use vox_orchestrator_queue::projection::ProjectionRegistry;
use vox_orchestrator_queue::projections::{AffinityProjection, LocksProjection};
use vox_orchestrator_types::AgentId;

fn registry() -> ProjectionRegistry {
    ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default())
}

#[tokio::test]
async fn compact_now_creates_checkpoint_and_prunes_warm_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let db = VoxDb::open(tmp.path().join("vox.sqlite").to_str().unwrap())
        .await
        .unwrap();

    let projections = Arc::new(registry());
    let mut log = OpLog::with_db(db.clone(), 10_000);
    log.bind_projections(projections.clone());

    // Seed a handful of lock ops.
    let mut last_id = OperationId(0);
    for i in 0..10 {
        last_id = log
            .record_persisted(
                AgentId(1),
                OperationKind::LockAcquire {
                    path: format!("src/file_{i}.rs"),
                    agent_id: 1,
                },
                format!("lock {i}"),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("record_persisted");
    }

    let pre_compaction_hash = projections.snapshot_blake3();

    // Deterministic: call compact_now directly with a chosen up_to rather than
    // waiting for the 1,000,000-op trigger.
    let persist_ctx = log.persist_context().expect("persist context bound");
    compact_now(persist_ctx.clone(), last_id)
        .await
        .expect("compact_now");

    // (a) a durable Checkpoint entry now exists with the correct op_id_lo/op_id_hi
    let checkpoint = db
        .latest_checkpoint_blob("default")
        .await
        .expect("latest_checkpoint_blob")
        .expect("checkpoint should exist");
    let (_blob_id, op_id_lo, op_id_hi, _blake3_hex, payload) = checkpoint;
    assert_eq!(op_id_lo, 0, "first checkpoint should start from 0");
    assert_eq!(op_id_hi, last_id.0, "checkpoint op_id_hi must match up_to");
    assert!(!payload.is_empty(), "checkpoint payload must be non-empty");

    // (b) rows below op_id_lo are genuinely pruned from the warm tier
    let recent = db
        .load_recent_convergence_op_log(1000)
        .await
        .expect("load_recent_convergence_op_log");
    let pruned_still_present = recent.iter().any(|r| r.op_id <= op_id_hi && r.op_id != 0);
    // All 10 LockAcquire ops (ids 1..=10, all <= op_id_hi) must be gone; only
    // the Checkpoint marker itself (op_id_hi + 1) may remain.
    assert!(
        !pruned_still_present,
        "convergence_op_log rows covered by the checkpoint must be pruned"
    );

    // (c) the blob is retrievable and its content, restored into a fresh
    // ProjectionRegistry, produces the same snapshot_blake3 as pre-compaction.
    let fresh = registry();
    fresh
        .restore_bytes(&payload)
        .expect("restore_bytes should succeed");
    assert_eq!(
        fresh.snapshot_blake3(),
        pre_compaction_hash,
        "restored registry must match pre-compaction snapshot hash"
    );

    // hydrate_from_checkpoint should report the same op_id_hi and restore identically.
    let hydrate_registry = registry();
    let hydrated_hi = hydrate_from_checkpoint(&persist_ctx, &hydrate_registry)
        .await
        .expect("hydrate_from_checkpoint")
        .expect("checkpoint should be found");
    assert_eq!(hydrated_hi, op_id_hi);
    assert_eq!(hydrate_registry.snapshot_blake3(), pre_compaction_hash);
}

#[tokio::test]
async fn hydrate_from_checkpoint_returns_none_when_no_checkpoint_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let db = VoxDb::open(tmp.path().join("vox_none.sqlite").to_str().unwrap())
        .await
        .unwrap();
    let log = OpLog::with_db(db.clone(), 10_000);
    let ctx = log.persist_context().expect("persist context bound");
    let fresh = registry();
    let result = hydrate_from_checkpoint(&ctx, &fresh)
        .await
        .expect("hydrate_from_checkpoint should not error");
    assert!(result.is_none());
}
