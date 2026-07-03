//! Cold-tier compaction (T1.6): emit a durable `OperationKind::Checkpoint` op
//! encoding projection state and prune warm `convergence_op_log` rows below
//! the checkpoint's `op_id_lo`.
//!
//! ## Storage
//!
//! The checkpoint payload (every registered [`Projection`](crate::projection::Projection)'s
//! `snapshot()` output, framed via [`ProjectionRegistry::snapshot_bytes`]) is
//! stored in vox-db's `checkpoint_blobs` table — a small dedicated table
//! rather than the generic `objects` CAS store, because `OperationKind::Checkpoint.
//! payload_blob_id` is a `u64` row id (cheap to reference from the durable op
//! itself) rather than a content-hash string. See
//! `crates/vox-db/src/schema/domains/sql/coordination.sql`.
//!
//! ## No-op path
//!
//! If [`PersistContext::projections`] is `None` (no registry attached via
//! [`super::OpLog::bind_projections`]/[`PersistContext::with_projections`]),
//! this still records a `Checkpoint` marker (so `op_id_lo`/`op_id_hi` bookkeeping
//! stays consistent) but with an empty payload and a zero hash — there is
//! nothing to restore from it. Real restorability requires a registry.

use std::sync::Arc;

use super::persist::{PersistContext, PersistError};
use super::{OperationId, OperationKind};

/// Snapshot all projections, write a Checkpoint op, and prune warm rows below `up_to`.
pub async fn compact_now(ctx: Arc<PersistContext>, up_to: OperationId) -> Result<(), PersistError> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Find the previous checkpoint's op_id_hi (if any) so this checkpoint's
    // op_id_lo picks up exactly where the last one left off, keeping the
    // ranges contiguous and non-overlapping.
    let op_id_lo = match ctx
        .db
        .latest_checkpoint_blob(&ctx.repository_id)
        .await
        .map_err(|e| PersistError::Db(e.to_string()))?
    {
        Some((_, _, prev_hi, ..)) => prev_hi,
        None => 0,
    };

    if up_to.0 <= op_id_lo {
        // Nothing new to compact (can happen if compact_now is called
        // explicitly with a stale `up_to`).
        return Ok(());
    }

    let (payload, projection_blake3) = match ctx.projections.as_ref() {
        Some(registry) => {
            let bytes = registry.snapshot_bytes();
            let hash = *blake3::hash(&bytes).as_bytes();
            (bytes, hash)
        }
        None => (Vec::new(), [0u8; 32]),
    };

    let blob_id = ctx
        .db
        .insert_checkpoint_blob(
            &ctx.repository_id,
            op_id_lo,
            up_to.0,
            &hex::encode(projection_blake3),
            payload,
            now_ms,
        )
        .await
        .map_err(|e| PersistError::Db(e.to_string()))?;

    let kind = OperationKind::Checkpoint {
        op_id_lo,
        op_id_hi: up_to.0,
        projection_blake3,
        payload_blob_id: blob_id,
    };
    let kind_json = serde_json::to_string(&kind).map_err(PersistError::Serde)?;
    let set_id_hex = hex::encode(ctx.set_id);
    let daemon_id_hex = hex::encode(ctx.daemon_id);
    let payload_blake3_hex = hex::encode(blake3::hash(kind_json.as_bytes()).as_bytes());

    // The Checkpoint marker itself gets the next id minted through the
    // *same* `OperationIdGenerator` every other durable write on this OpLog
    // uses (`ctx.id_gen`, shared via `Arc` from `OpLog::with_db`) — never a
    // freestanding `up_to.0 + 1`. Minting out-of-band never advanced the
    // generator's atomic counter, so the very next `record_persisted` call
    // minted the *same* id; `insert_convergence_op_log`'s `INSERT OR IGNORE`
    // then silently swallowed the collision, dropping that op with no error
    // (T1.6 follow-up, Bug 1 — deterministic on every compaction, not a rare
    // race, mirroring the fix already applied to the sibling production path
    // in `vox-orchestrator/src/orchestrator/core/checkpoint.rs`).
    let checkpoint_op_id = ctx.id_gen.next().0;
    debug_assert!(
        checkpoint_op_id > up_to.0,
        "checkpoint marker id must not collide with the range it summarizes"
    );
    ctx.db
        .insert_convergence_op_log(
            checkpoint_op_id as i64,
            &set_id_hex,
            "[]",
            &kind_json,
            &payload_blake3_hex,
            None,
            None,
            None,
            0,
            &daemon_id_hex,
            now_ms,
            &format!("checkpoint covering OP-{op_id_lo:06}..=OP-{:06}", up_to.0),
            None,
            None,
        )
        .await
        .map_err(|e| PersistError::Db(e.to_string()))?;

    // Prune warm-tier convergence_op_log rows now covered by the checkpoint.
    ctx.db
        .prune_convergence_op_log_up_to(op_id_lo, up_to.0)
        .await
        .map_err(|e| PersistError::Db(e.to_string()))?;

    Ok(())
}

/// Startup hydration (T1.6): if a checkpoint exists for `ctx.repository_id`,
/// restore `registry` from its blob and return `Some(op_id_hi)` so the caller
/// can replay only ops with `op_id > op_id_hi` instead of full history.
/// Returns `Ok(None)` if no checkpoint has ever been recorded (caller should
/// replay from the beginning, matching pre-T1.6 behavior).
pub async fn hydrate_from_checkpoint(
    ctx: &PersistContext,
    registry: &crate::projection::ProjectionRegistry,
) -> Result<Option<u64>, PersistError> {
    let Some((_, _op_id_lo, op_id_hi, _blake3_hex, payload)) = ctx
        .db
        .latest_checkpoint_blob(&ctx.repository_id)
        .await
        .map_err(|e| PersistError::Db(e.to_string()))?
    else {
        return Ok(None);
    };

    if !payload.is_empty() {
        registry
            .restore_bytes(&payload)
            .map_err(|e| PersistError::Db(format!("checkpoint restore failed: {e}")))?;
    }

    Ok(Some(op_id_hi))
}
