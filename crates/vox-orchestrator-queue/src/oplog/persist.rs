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

use crate::projection::ProjectionRegistry;

use super::{OpLog, OperationEntry, OperationId, OperationIdGenerator, OperationKind};

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
    /// Logical namespace `compact_now`/checkpoint hydration use to scope
    /// `checkpoint_blobs` rows (T1.6). Defaults to `"default"` — callers with
    /// multiple independent op-log streams sharing one `VoxDb` should set a
    /// distinct value per stream so their checkpoints don't collide.
    pub repository_id: String,
    /// Registered projections to snapshot on compaction (T1.6). `None` means
    /// `compact_now` records a `Checkpoint` marker with an empty payload and
    /// prunes warm rows, but has nothing to restore state from — set this via
    /// [`Self::with_projections`] to get a real, restorable checkpoint.
    pub projections: Option<Arc<ProjectionRegistry>>,
    /// The *same* [`OperationIdGenerator`] instance backing the owning
    /// [`OpLog`] (shared via `Arc`, wired in [`OpLog::with_db`]/
    /// [`OpLog::reseed_id_gen_from_highest`]). `checkpoint::compact_now` mints
    /// its `Checkpoint` marker's id through this — never via freestanding
    /// arithmetic like `up_to.0 + 1` — so the marker's id always advances the
    /// same atomic counter every other `record`/`record_persisted` call uses.
    /// Minting out-of-band let the very next `record_persisted` call reuse the
    /// same id, and `insert_convergence_op_log`'s `INSERT OR IGNORE` silently
    /// swallowed the resulting collision (T1.6 follow-up, Bug 1).
    pub id_gen: Arc<OperationIdGenerator>,
}

impl std::fmt::Debug for PersistContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistContext")
            .field("daemon_id", &hex::encode(self.daemon_id))
            .field("set_id", &hex::encode(self.set_id))
            .field("compaction_interval", &self.compaction_interval)
            .field("repository_id", &self.repository_id)
            .field("has_projections", &self.projections.is_some())
            .finish()
    }
}

impl PersistContext {
    pub fn new(db: VoxDb, daemon_id: [u8; 16], set_id: [u8; 16]) -> Self {
        Self::with_id_gen(db, daemon_id, set_id, Arc::new(OperationIdGenerator::new()))
    }

    /// Like [`Self::new`], but shares an existing [`OperationIdGenerator`]
    /// (e.g. the owning [`OpLog`]'s) instead of minting a fresh one. Callers
    /// that construct a `PersistContext` outside of [`OpLog::with_db`] should
    /// use this to keep the checkpoint marker's id allocation on the same
    /// counter as every other durable write.
    pub fn with_id_gen(
        db: VoxDb,
        daemon_id: [u8; 16],
        set_id: [u8; 16],
        id_gen: Arc<OperationIdGenerator>,
    ) -> Self {
        Self {
            db,
            daemon_id,
            set_id,
            compaction_interval: DEFAULT_COMPACTION_INTERVAL,
            repository_id: "default".to_string(),
            projections: None,
            id_gen,
        }
    }

    /// Attach a [`ProjectionRegistry`] so `compact_now` produces real,
    /// restorable checkpoint blobs instead of an empty-payload marker.
    pub fn with_projections(mut self, registry: Arc<ProjectionRegistry>) -> Self {
        self.projections = Some(registry);
        self
    }
}

impl OpLog {
    /// Create a log bound to `vox-db` for write-through persistence. Does
    /// **not** query the DB — the [`OperationId`] generator still starts at 1.
    /// Prefer [`Self::with_db_seeded`] in any path that must survive a
    /// process restart with a monotonic id sequence (T1.3); this sync
    /// constructor is kept for callers that cannot await (rare) and for
    /// call sites that immediately follow up with
    /// [`Self::reseed_id_gen_from_highest`] themselves.
    pub fn with_db(db: VoxDb, hot_capacity: usize) -> Self {
        let mut log = OpLog::new(hot_capacity);
        log.persist = Some(Arc::new(PersistContext::with_id_gen(
            db,
            [0u8; 16],
            [0u8; 16],
            log.id_gen.clone(),
        )));
        log
    }

    /// [`Self::with_db`], then seed the [`OperationId`] generator from the
    /// highest `op_id` already persisted in `convergence_op_log` (T1.3
    /// restart-durability). A fresh DB with no rows yet leaves the generator
    /// starting at 1, matching today's behavior; a DB carrying prior history
    /// (the process restarted) resumes strictly after the highest existing id
    /// so a client using `OperationId` as a replay offset never sees the
    /// sequence go backwards or collide across a restart.
    pub async fn with_db_seeded(db: VoxDb, hot_capacity: usize) -> Result<Self, PersistError> {
        let highest = db
            .max_convergence_op_id()
            .await
            .map_err(|e| PersistError::Db(e.to_string()))?;
        let mut log = Self::with_db(db, hot_capacity);
        if let Some(highest) = highest {
            log.reseed_id_gen_from_highest(highest);
        }
        Ok(log)
    }

    /// Bind daemon + set identity (must be called before first `record_persisted`).
    pub fn bind_identity(&mut self, daemon_id: [u8; 16], set_id: [u8; 16]) {
        if let Some(ctx) = self.persist.as_ref() {
            let updated = PersistContext {
                db: ctx.db.clone(),
                daemon_id,
                set_id,
                compaction_interval: ctx.compaction_interval,
                repository_id: ctx.repository_id.clone(),
                projections: ctx.projections.clone(),
                id_gen: ctx.id_gen.clone(),
            };
            self.persist = Some(Arc::new(updated));
        }
    }

    /// Return a clone of the bound [`PersistContext`], if any — used by
    /// `checkpoint::compact_now`/`hydrate_from_checkpoint` callers (including
    /// the T1.6 test suite) that need to call those functions directly with a
    /// deterministic `up_to` rather than waiting for the compaction-interval
    /// trigger inside `record_persisted`.
    pub fn persist_context(&self) -> Option<Arc<PersistContext>> {
        self.persist.clone()
    }

    /// Attach a [`ProjectionRegistry`] to this log's bound [`PersistContext`]
    /// (T1.6). Must be called after [`Self::with_db`]/[`Self::with_db_seeded`];
    /// a no-op if no persist context is bound yet. Every subsequent
    /// `record_persisted` call applies the entry to `registry`, and
    /// `compact_now` snapshots it into the checkpoint blob.
    pub fn bind_projections(&mut self, registry: Arc<ProjectionRegistry>) {
        if let Some(ctx) = self.persist.as_ref() {
            let updated = PersistContext {
                db: ctx.db.clone(),
                daemon_id: ctx.daemon_id,
                set_id: ctx.set_id,
                compaction_interval: ctx.compaction_interval,
                repository_id: ctx.repository_id.clone(),
                projections: Some(registry),
                id_gen: ctx.id_gen.clone(),
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

        if let Some(registry) = ctx.projections.as_ref() {
            registry.apply(&entry).await;
        }

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
