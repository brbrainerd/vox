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

/// T1.6 follow-up regression (Bug 1, HIGH — deterministic silent data loss):
/// `compact_now` used to mint the Checkpoint marker's operation_id via
/// freestanding arithmetic (`up_to.0 + 1`) instead of the shared
/// `OperationIdGenerator`. That never advanced the generator's atomic
/// counter, so the very next `record_persisted` call minted the *same* id as
/// the checkpoint marker; `insert_convergence_op_log`'s `INSERT OR IGNORE`
/// silently swallowed the resulting collision — `record_persisted` returned
/// `Ok`, but the write never actually landed in the DB. This is deterministic
/// on every single compaction, not a rare race.
///
/// This test records N ops, calls `compact_now`, records one more op, and
/// then queries the database directly (not just `record_persisted`'s return
/// value) to confirm the write actually landed with a genuinely unique,
/// non-colliding operation_id.
#[tokio::test]
async fn record_after_compact_now_does_not_collide_with_checkpoint_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let db = VoxDb::open(tmp.path().join("vox_collision.sqlite").to_str().unwrap())
        .await
        .unwrap();

    let projections = Arc::new(registry());
    let mut log = OpLog::with_db(db.clone(), 10_000);
    log.bind_projections(projections.clone());

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

    let persist_ctx = log.persist_context().expect("persist context bound");
    compact_now(persist_ctx.clone(), last_id)
        .await
        .expect("compact_now");

    // The Checkpoint marker itself was just recorded — read it back to know
    // its op_id_hex string, so we can assert the *next* op doesn't collide.
    let checkpoint = db
        .latest_checkpoint_blob("default")
        .await
        .expect("latest_checkpoint_blob")
        .expect("checkpoint should exist");
    let (_blob_id, _op_id_lo, op_id_hi, _blake3_hex, _payload) = checkpoint;
    assert_eq!(op_id_hi, last_id.0);

    // Record one more op immediately after compact_now.
    let post_compact_id = log
        .record_persisted(
            AgentId(1),
            OperationKind::LockAcquire {
                path: "src/post_compact.rs".to_string(),
                agent_id: 1,
            },
            "lock after compaction",
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("record_persisted after compact_now");

    // The bug: this id used to collide with the checkpoint marker's id
    // (up_to.0 + 1), since compact_now never advanced the shared generator.
    assert!(
        post_compact_id.0 > op_id_hi,
        "post-compaction op id ({}) must be strictly greater than the checkpoint marker's id ({})",
        post_compact_id.0,
        op_id_hi
    );

    // Query the database directly — do not trust record_persisted's Ok
    // return value alone, since INSERT OR IGNORE silently swallows a
    // colliding write while still letting the calling code observe Ok(()).
    let recent = db
        .load_recent_convergence_op_log(1000)
        .await
        .expect("load_recent_convergence_op_log");
    let landed = recent
        .iter()
        .find(|r| r.op_id == post_compact_id.0)
        .expect("post-compaction op must have actually landed in convergence_op_log");
    assert!(
        landed.description.contains("lock after compaction"),
        "the landed row must be the real post-compaction op, not a stale/colliding row"
    );

    // And it must be a distinct row from the checkpoint marker's own row
    // (marker id > op_id_hi, since the marker summarizes ..=op_id_hi and is
    // itself outside the pruned range `op_id <= op_id_hi`).
    let marker_rows: Vec<_> = recent
        .iter()
        .filter(|r| r.op_id > op_id_hi && r.op_id < post_compact_id.0)
        .collect();
    assert_eq!(
        marker_rows.len(),
        1,
        "checkpoint marker row must exist exactly once, undisturbed by the later write; found {marker_rows:?}"
    );
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
