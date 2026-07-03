//! T1.6: op-log retention / checkpoint compaction for the real production
//! durable path — `agent_oplog` (written by `Orchestrator::record_operation`,
//! read by `rehydrate.rs`'s boot-time scan and `orch.subscribe`'s
//! replay-from-offset) — as opposed to `vox-orchestrator-queue`'s
//! `OpLog::record_persisted`/`convergence_op_log`/`ProjectionRegistry`
//! machinery, which today is exercised only by its own test suite and by
//! nothing in the live `vox-orchestrator`/`vox-orchestrator-mcp` call graph
//! (verified: no production call site references `record_persisted` or
//! `PersistContext`). See `oplog/checkpoint.rs` in `vox-orchestrator-queue`
//! for that crate's independent (also-real, also-tested) implementation of
//! the same P3-T9 checkpoint design against its own table.
//!
//! ## Design: checkpoint the derived fact, not the raw projection framework
//!
//! `agent_oplog` has no `ProjectionRegistry` wired to it — the durable state
//! consumers actually need on restart (open direct-submit tasks; see
//! `rehydrate.rs`) is produced by a small, already-deterministic scan-based
//! reducer, not a `Projection` impl. Building formal `Projection`s for
//! task/approval/feedback/hopper lifecycle state, wiring them into
//! `record_operation`, and threading a `ProjectionRegistry` through
//! `Orchestrator` would be considerably more surface area for the same
//! outcome this module achieves directly: serialize `rehydrate::OpenTaskState`
//! (the exact fold `rehydrate_direct_submit_tasks` needs) as the checkpoint
//! payload, store it via the same `checkpoint_blobs` table
//! `vox-orchestrator-queue` uses, and record an `OperationKind::Checkpoint`
//! marker in `agent_oplog` referencing it. See T1.6's task brief: "Prefer the
//! smaller change that still achieves 'startup rehydrate time bounded by
//! live-state size' unless the pure-projection approach is clearly not much
//! larger" — it is clearly larger here, so this module takes the direct path.
//!
//! Approval/feedback/doubt/hopper lifecycle state (T1.1's other new durable
//! event kinds) has no equivalent "reconstruct into live in-memory state on
//! boot" consumer today — nothing in `vox-orchestrator` folds
//! `ApprovalRequested`/`FeedbackRequested`/etc. into an in-memory structure
//! the way `rehydrate.rs` does for tasks, so there is no analogous state to
//! checkpoint for them yet; when a rehydration consumer for that state is
//! built, extend [`OpenTaskState`](super::rehydrate::OpenTaskState) (or fold
//! a sibling struct into the same checkpoint payload) rather than
//! introducing a second checkpoint mechanism.

use super::rehydrate::OpenTaskState;

const CHECKPOINT_INTERVAL: u64 = 1_000_000;

/// Called from `Orchestrator::record_operation` after every durable write.
/// Triggers a checkpoint every `CHECKPOINT_INTERVAL` ops, mirroring
/// `vox-orchestrator-queue`'s `record_persisted` trigger.
pub(crate) async fn maybe_compact(orch: &crate::orchestrator::Orchestrator, op_id: u64) {
    if !op_id.is_multiple_of(CHECKPOINT_INTERVAL) {
        return;
    }
    if let Err(e) = compact_now(orch, op_id).await {
        tracing::warn!(
            error = %e,
            op_id,
            "T1.6: agent_oplog checkpoint compaction failed"
        );
    }
}

/// Snapshot open-task state up to `up_to` (inclusive), write a durable
/// `Checkpoint` marker into `agent_oplog`, and prune `agent_oplog` rows with
/// `operation_id <= up_to`. Exposed (not just `pub(crate)`-triggered) so
/// tests can call it deterministically instead of waiting for the 1,000,000-op
/// trigger.
///
/// Takes `&Orchestrator` (not a bare `VoxDb` handle) so the Checkpoint
/// marker's own `operation_id` is minted via
/// `orch.record_operation_no_compact_trigger` — i.e. through the *same*
/// `OperationIdGenerator` every other durable write goes through. Minting the
/// marker's id out-of-band (e.g. a freestanding `max(existing) + 1` query)
/// races the in-memory generator: `record` hands out the next id
/// synchronously, before its durable write is even awaited, so a
/// concurrently-running compaction querying "current max" can under-count and
/// pick an id a live `record_operation` call has already claimed —
/// `agent_oplog.operation_id` is `UNIQUE`, so that collision silently drops
/// one of the two writes (whichever loses the race), losing either the
/// checkpoint marker or a real op. Going through the same generator closes
/// this by construction. (The `_no_compact_trigger` variant is used instead
/// of plain `record_operation` purely to avoid re-arming `maybe_compact` and
/// recursing — see that method's doc comment in `vcs_ops.rs`.)
pub async fn compact_now(
    orch: &crate::orchestrator::Orchestrator,
    up_to: u64,
) -> Result<(), String> {
    let db_opt = { crate::sync_lock::rw_read(&*orch.db).clone() };
    let Some(db) = db_opt else {
        return Err("no db attached".to_string());
    };
    let repository_id = crate::lineage::repository_id();

    let op_id_lo = match db
        .latest_checkpoint_blob(&repository_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some((_, _, prev_hi, ..)) => prev_hi,
        None => 0,
    };

    if up_to <= op_id_lo {
        return Ok(()); // nothing new to compact
    }

    // Fold the range being compacted into open-task state, seeded from the
    // previous checkpoint (if any) so the new checkpoint is a full picture,
    // not just a delta.
    let mut state = match db
        .latest_checkpoint_blob(&repository_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some((_, _, _, _, payload)) if !payload.is_empty() => {
            serde_json::from_slice::<OpenTaskState>(&payload).map_err(|e| e.to_string())?
        }
        _ => OpenTaskState::default(),
    };

    let entries = vox_orchestrator_queue::oplog::list_from_db_since(&db, &repository_id, op_id_lo)
        .await?
        .into_iter()
        .filter(|e| e.id.0 <= up_to)
        .collect::<Vec<_>>();

    state = fold_into(state, &entries);

    let payload = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
    let projection_blake3 = blake3::hash(&payload);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let blob_id = db
        .insert_checkpoint_blob(
            &repository_id,
            op_id_lo,
            up_to,
            &hex::encode(projection_blake3.as_bytes()),
            payload,
            now_ms,
        )
        .await
        .map_err(|e| e.to_string())?;

    let kind = crate::oplog::OperationKind::Checkpoint {
        op_id_lo,
        op_id_hi: up_to,
        projection_blake3: *projection_blake3.as_bytes(),
        payload_blob_id: blob_id,
    };

    // Mint the marker's operation_id through the same generator (and the
    // same durable write path) every other op uses — see doc comment above
    // for why this matters. Uses the `_no_compact_trigger` variant so this
    // write doesn't re-arm `maybe_compact` and recurse.
    orch.record_operation_no_compact_trigger(
        vox_orchestrator_types::AgentId(0),
        kind,
        format!("checkpoint covering OP-{op_id_lo:06}..=OP-{up_to:06}"),
    )
    .await;

    db.prune_agent_oplog_up_to(&repository_id, up_to)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn fold_into(state: OpenTaskState, entries: &[crate::oplog::OperationEntry]) -> OpenTaskState {
    // Reuse the exact reducer rehydrate.rs uses, via its crate-visible fold.
    super::rehydrate::fold_open_task_state(state, entries)
}

/// Load the most recent `agent_oplog` checkpoint for `repository_id`, if any,
/// restoring `OpenTaskState` from its blob. Returns `(state, op_id_hi)` so
/// the caller can scan only the tail (`op_id > op_id_hi`).
pub(crate) async fn latest_agent_oplog_checkpoint(
    db: &vox_db::VoxDb,
    repository_id: &str,
) -> Result<Option<(OpenTaskState, u64)>, String> {
    let Some((_, _op_id_lo, op_id_hi, _blake3, payload)) = db
        .latest_checkpoint_blob(repository_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };

    let state = if payload.is_empty() {
        OpenTaskState::default()
    } else {
        serde_json::from_slice(&payload).map_err(|e| e.to_string())?
    };

    Ok(Some((state, op_id_hi)))
}
