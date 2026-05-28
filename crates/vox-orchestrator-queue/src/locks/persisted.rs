//! Persistent write-through extension for `FileLockManager` (Phase 0, P0-T1).
//!
//! Provides async methods that both update the in-memory map and write through
//! to the `vcs_lock` table so a fresh daemon instance can replay the lock map
//! on startup via [`FileLockManager::hydrate_from_db`].

use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use vox_db::mesh_locks::{LockKindRow, VcsLockRow};
use vox_orchestrator_types::AgentId;

use super::{FileLock, FileLockManager, LockConflict, LockEntry, LockKind};
use crate::sync_lock;

/// Default TTL for persisted file locks (30 minutes).
const DEFAULT_LOCK_TTL_MS: i64 = 30 * 60 * 1_000;

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

impl FileLockManager {
    /// Create a lock manager backed by a vox-db instance for persistence.
    ///
    /// All mutations via [`Self::try_acquire_persisted`] and [`Self::release_persisted`]
    /// write through to the `vcs_lock` table. On daemon start, call
    /// [`Self::hydrate_from_db`] to replay the on-disk state into memory.
    pub fn with_db(db: vox_db::VoxDb, node_id: &str, repository_id: &str) -> Self {
        Self {
            locks: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            queue: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            db: Some(db),
            node_id: node_id.to_string(),
            repository_id: repository_id.to_string(),
        }
    }

    /// Replay the `vcs_lock` table into the in-memory map.
    ///
    /// Called once at daemon startup so the manager reflects locks that
    /// survived a restart. No-op when no DB handle is configured.
    pub async fn hydrate_from_db(&self) -> Result<(), vox_db::StoreError> {
        let Some(db) = &self.db else {
            return Ok(());
        };

        let rows = db.mesh_locks_for_repo(&self.repository_id).await?;
        let current_ms = now_ms();
        let mut locks = sync_lock::rw_write(&*self.locks);

        for row in rows {
            if row.expires_at <= current_ms {
                continue; // Skip expired rows; leader prune will clean them.
            }
            let Ok(holder_u64) = row.holder.parse::<u64>() else {
                tracing::warn!(path = %row.path, holder = %row.holder, "non-numeric holder in vcs_lock; skipping");
                continue;
            };
            let agent = AgentId(holder_u64);
            let kind = match row.kind {
                LockKindRow::Exclusive => LockKind::Exclusive,
                LockKindRow::SharedRead => LockKind::SharedRead,
            };
            let path = std::path::PathBuf::from(&row.path);
            let lock = FileLock {
                path: path.clone(),
                kind,
                holder: agent,
                acquired_at: Instant::now(),
            };
            let entry = match kind {
                LockKind::Exclusive => LockEntry::Exclusive(lock),
                LockKind::SharedRead => LockEntry::SharedRead(vec![lock]),
            };
            locks.insert(path, entry);
        }
        Ok(())
    }

    /// Acquire a lock on a file and write through to the DB (if configured).
    ///
    /// Returns `Err(LockConflict)` when the in-memory check fails; the DB
    /// write is fire-and-log (non-fatal if the DB is temporarily unavailable).
    pub async fn try_acquire_persisted(
        &self,
        path: &Path,
        agent: AgentId,
        kind: LockKind,
    ) -> Result<(), LockConflict> {
        // In-memory check first.
        self.try_acquire(path, agent, kind)?;

        // Write through to DB when configured.
        if let Some(db) = &self.db {
            let ts = now_ms();
            let row = VcsLockRow {
                path: path.to_string_lossy().into_owned(),
                kind: match kind {
                    LockKind::Exclusive => LockKindRow::Exclusive,
                    LockKind::SharedRead => LockKindRow::SharedRead,
                },
                holder: agent.0.to_string(),
                holder_node_id: self.node_id.clone(),
                repository_id: self.repository_id.clone(),
                acquired_at: ts,
                expires_at: ts + DEFAULT_LOCK_TTL_MS,
                lease_id: None,
                fence_token: 0,
            };
            if let Err(e) = db.mesh_locks_upsert(&row).await {
                tracing::warn!(path = %path.display(), error = %e, "write-through DB upsert failed");
            }
        }
        Ok(())
    }

    /// Release a lock and write the deletion through to the DB (if configured).
    pub async fn release_persisted(&self, path: &Path, agent: AgentId) {
        self.release(path, agent);

        if let Some(db) = &self.db
            && let Err(e) = db
                .mesh_locks_release(&path.to_string_lossy(), &self.node_id)
                .await
        {
            tracing::warn!(path = %path.display(), error = %e, "write-through DB release failed");
        }
    }
}
