//! Persistence glue for [`OpLog`] against `vox-db`.
//!
//! Tiered retention model:
//! * **Hot tier** — last `hot_capacity` (default 10_000) entries in `OpLog::entries`
//!   `VecDeque`. Reads from here are O(1) lookups.
//! * **Warm tier** — every `record_persisted` call also inserts into the
//!   `convergence_op_log` table. Eviction from the hot tier never deletes warm rows.
//! * **Cold tier** — every 1_000_000 ops (or via explicit `compact_now`),
//!   the [`checkpoint`](super::checkpoint) module emits a `OperationKind::Checkpoint`
//!   op encoding projection state and prunes warm rows below `op_id_lo`.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator_types::{AgentId, ChangeId, SnapshotId};

use super::{OpLog, OperationEntry, OperationId, OperationKind};

const DEFAULT_COMPACTION_INTERVAL: u64 = 1_000_000;

/// Vox-db context bound to an [`OpLog`] for write-through persistence.
#[derive(Clone)]
pub struct PersistContext {
    pub db: VoxDb,
    /// 16-byte daemon UUID (hex-encoded for storage).
    pub daemon_id: [u8; 16],
    /// 16-byte convergence-set ULID (hex-encoded for storage).
    pub set_id: [u8; 16],
    pub compaction_interval: u64,
}

impl std::fmt::Debug for PersistContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistContext")
            .field("daemon_id", &hex::encode(self.daemon_id))
            .field("set_id", &hex::encode(self.set_id))
            .field("compaction_interval", &self.compaction_interval)
            .finish()
    }
}

impl PersistContext {
    pub fn new(db: VoxDb, daemon_id: [u8; 16], set_id: [u8; 16]) -> Self {
        Self {
            db,
            daemon_id,
            set_id,
            compaction_interval: DEFAULT_COMPACTION_INTERVAL,
        }
    }
}

impl OpLog {
    /// Create a log bound to `vox-db` for write-through persistence.
    pub fn with_db(db: VoxDb, hot_capacity: usize) -> Self {
        let mut log = OpLog::new(hot_capacity);
        log.persist = Some(Arc::new(PersistContext::new(db, [0u8; 16], [0u8; 16])));
        log
    }

    /// Bind daemon + set identity (must be called before first `record_persisted`).
    pub fn bind_identity(&mut self, daemon_id: [u8; 16], set_id: [u8; 16]) {
        if let Some(ctx) = self.persist.as_ref() {
            let updated = PersistContext {
                db: ctx.db.clone(),
                daemon_id,
                set_id,
                compaction_interval: ctx.compaction_interval,
            };
            self.persist = Some(Arc::new(updated));
        }
    }

    /// Record an op and write it through to vox-db.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_persisted(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
    ) -> Result<OperationId, PersistError> {
        let id = self.record(
            agent_id,
            kind,
            description,
            snapshot_before,
            snapshot_after,
            db_snapshot_before,
            db_snapshot_after,
            context_snapshot_before,
            context_snapshot_after,
        );

        let entry = self
            .entries
            .back()
            .cloned()
            .ok_or(PersistError::EntryMissing)?;

        let ctx = self
            .persist
            .as_ref()
            .ok_or(PersistError::NoPersistContext)?
            .clone();

        write_entry(&ctx, &entry).await?;

        if id.0.is_multiple_of(ctx.compaction_interval) {
            super::checkpoint::compact_now(ctx, id).await?;
        }

        Ok(id)
    }

    /// Warm-load the most recent `n` entries from vox-db into the hot tier on startup.
    pub async fn warm_load_recent(&mut self, n: usize) -> Result<(), PersistError> {
        let ctx = self
            .persist
            .as_ref()
            .ok_or(PersistError::NoPersistContext)?
            .clone();

        let rows = ctx
            .db
            .load_recent_convergence_op_log(n as i64)
            .await
            .map_err(|e| PersistError::Db(e.to_string()))?;

        // Rows come newest-first; insert oldest-first into the hot tier.
        for row in rows.into_iter().rev() {
            let kind: OperationKind =
                serde_json::from_str(&row.kind_json).map_err(PersistError::Serde)?;
            let parent_op_ids: Vec<u64> =
                serde_json::from_str(&row.parent_op_ids_json).unwrap_or_default();

            let entry = OperationEntry {
                id: OperationId(row.op_id),
                agent_id: AgentId(row.agent_id),
                timestamp_ms: row.produced_at,
                kind,
                description: row.description,
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: row.undone,
                change_id: row.change_id.map(ChangeId),
                model_id: row.model_id,
                predecessor_hash: row.predecessor_hash,
                signature: None,
                signing_key_id: None,
                daemon_id: [0u8; 16],
                parent_op_ids,
            };

            self.entries.push_back(entry);
            while self.entries.len() > self.max_entries {
                self.entries.pop_front();
            }
        }

        Ok(())
    }
}

async fn write_entry(ctx: &PersistContext, entry: &OperationEntry) -> Result<(), PersistError> {
    let kind_json = serde_json::to_string(&entry.kind).map_err(PersistError::Serde)?;
    let parents_json = serde_json::to_string(&entry.parent_op_ids).map_err(PersistError::Serde)?;
    let set_id_hex = hex::encode(ctx.set_id);
    let daemon_id_hex = hex::encode(ctx.daemon_id);
    let payload_blake3_hex = hex::encode(blake3::hash(kind_json.as_bytes()).as_bytes());

    ctx.db
        .insert_convergence_op_log(
            entry.id.0 as i64,
            &set_id_hex,
            &parents_json,
            &kind_json,
            &payload_blake3_hex,
            entry.predecessor_hash.as_deref(),
            entry.signature.as_ref().map(hex::encode).as_deref(),
            entry.signing_key_id.as_ref().map(hex::encode).as_deref(),
            entry.agent_id.0 as i64,
            &daemon_id_hex,
            entry.timestamp_ms as i64,
            &entry.description,
            entry.change_id.map(|c| c.0 as i64),
            entry.model_id.as_deref(),
        )
        .await
        .map_err(|e| PersistError::Db(e.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("no persist context bound; call OpLog::with_db")]
    NoPersistContext,
    #[error("entry missing after record")]
    EntryMissing,
    #[error("db error: {0}")]
    Db(String),
    #[error("serde_json: {0}")]
    Serde(#[from] serde_json::Error),
}
