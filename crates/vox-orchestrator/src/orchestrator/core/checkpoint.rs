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
//! ## HITL approval/feedback/doubt state: excluded from pruning, not folded
//! into the checkpoint payload (T1.6 follow-up, Bug 2, 2026-07-03)
//!
//! Earlier revision of this doc claimed approval/feedback/doubt lifecycle
//! state "has no equivalent reconstruct-into-live-state consumer today" and
//! so "there is no analogous state to checkpoint for them yet." That framing
//! was wrong: `hitl_rehydrate_on_restart`
//! (`vox-orchestrator-mcp/src/hitl_rehydrate.rs`) *is* exactly such a
//! consumer — a full-history scan over `agent_oplog` for unresolved
//! `ApprovalRequested`/`FeedbackRequested` entries, run at boot to restore
//! visibility into `PendingApprovals`/`FeedbackStore`. Because that scan has
//! no checkpoint awareness of its own, `compact_now`'s pruning (originally
//! unconditional on `operation_id <= up_to`) could silently, permanently
//! delete an unresolved approval/feedback/doubt row before it was ever
//! resolved — a human approval parked before a restart could vanish with no
//! trace anywhere and no error, a real regression in exactly the area (HITL
//! approval integrity) this codebase treats as security-critical.
//!
//! The fix taken here is approach (a) from the follow-up review, not (b):
//! `compact_now` does **not** fold HITL state into the `OpenTaskState`
//! checkpoint payload the way it does for tasks. Instead, immediately before
//! pruning, [`open_hitl_operation_ids`] rescans everything currently present
//! in `agent_oplog` up to `up_to` for `*Requested` entries with no matching
//! `*Resolved` (or, for `TaskDoubted`, no matching `TaskComplete`/`TaskFail`)
//! counterpart, and those specific rows are excluded from the prune via
//! [`vox_db::VoxDb::prune_agent_oplog_up_to_excluding`]. Approach (a) was
//! chosen over folding HITL state into the checkpoint (approach (b), which
//! would need `hitl_rehydrate_on_restart` to become checkpoint-aware too) because
//! it's the smaller change that fully closes the data-loss hole without
//! touching `hitl_rehydrate_on_restart`'s existing full-scan contract: an
//! unresolved row simply stays a real `agent_oplog` row past the checkpoint
//! boundary (at the cost of `agent_oplog` not shrinking as tightly as it
//! could while approvals sit open) rather than becoming a second serialized
//! representation of the same fact that the two code paths could drift apart
//! on. If open-HITL volume ever grows large enough for this rescan to become
//! a real cost, revisit folding it into the checkpoint payload the way
//! [`OpenTaskState`](super::rehydrate::OpenTaskState) already does for tasks
//! — but do not reintroduce unconditional pruning without either exclusion
//! mechanism in place.

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

    // T1.6 follow-up (Bug 2, HIGH — HITL integrity regression): never delete
    // an `agent_oplog` row that is part of an *unresolved*
    // `ApprovalRequested`/`FeedbackRequested`/`TaskDoubted` entry, even if its
    // `operation_id <= up_to`. `hitl_rehydrate_on_restart`
    // (`vox-orchestrator-mcp/src/hitl_rehydrate.rs`) scans the full durable
    // op-log for exactly these unresolved pairs on boot — pruning them out
    // from under it would silently, permanently destroy a parked human
    // approval or open feedback item with no trace anywhere and no error. See
    // `open_hitl_operation_ids` below for the scan.
    let open_hitl_op_ids = open_hitl_operation_ids(&db, &repository_id, up_to).await?;

    db.prune_agent_oplog_up_to_excluding(&repository_id, up_to, &open_hitl_op_ids)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Scan every `agent_oplog` row currently present with `operation_id <=
/// up_to` and return the `operation_id`s of `ApprovalRequested`/
/// `FeedbackRequested`/`TaskDoubted` entries that have no matching
/// `*Resolved`/resolving entry in that same range — i.e. still "open" as of
/// this compaction. These are the rows `compact_now` must exclude from
/// pruning (T1.6 follow-up, Bug 2).
///
/// A full-history scan bounded by `up_to`, not a tail-only scan since the
/// last checkpoint: an approval requested in an *earlier* checkpoint interval
/// and still unresolved only still exists in `agent_oplog` today because that
/// earlier compaction already excluded it from its own prune — a tail-only
/// view would miss it entirely and let this compaction delete it, which is
/// exactly the bug this exists to prevent.
///
/// `TaskDoubted` has no dedicated `*Resolved` variant in
/// [`crate::oplog::OperationKind`]; per the task brief, a doubt is treated as
/// resolved once a `TaskComplete`/`TaskFail` for the same `task_id` appears
/// (the verification pass the doubt forced has run its course).
async fn open_hitl_operation_ids(
    db: &vox_db::VoxDb,
    repository_id: &str,
    up_to: u64,
) -> Result<Vec<u64>, String> {
    use crate::oplog::OperationKind;

    let entries = vox_orchestrator_queue::oplog::list_from_db_up_to(db, repository_id, up_to)
        .await?
        .into_iter()
        .filter(|e| e.id.0 <= up_to)
        .collect::<Vec<_>>();

    // id -> operation_id of the *Requested row, removed once a matching
    // resolution is observed later in the (oldest-first) scan.
    let mut open_approvals: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut open_feedback: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut open_doubts: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();

    for entry in &entries {
        match &entry.kind {
            OperationKind::ApprovalRequested { approval_id, .. } => {
                open_approvals.insert(approval_id.clone(), entry.id.0);
            }
            OperationKind::ApprovalResolved { approval_id, .. } => {
                open_approvals.remove(approval_id);
            }
            OperationKind::FeedbackRequested { request_id, .. } => {
                open_feedback.insert(request_id.clone(), entry.id.0);
            }
            OperationKind::FeedbackResolved { request_id, .. } => {
                open_feedback.remove(request_id);
            }
            OperationKind::TaskDoubted { task_id, .. } => {
                open_doubts.insert(*task_id, entry.id.0);
            }
            OperationKind::TaskComplete { task_id } | OperationKind::TaskFail { task_id, .. } => {
                open_doubts.remove(task_id);
            }
            _ => {}
        }
    }

    Ok(open_approvals
        .into_values()
        .chain(open_feedback.into_values())
        .chain(open_doubts.into_values())
        .collect())
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
