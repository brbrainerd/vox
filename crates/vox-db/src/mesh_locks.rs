//! Typed accessors over the `vcs_lock` and `lock_leader` tables (Phase 0, P0-T1/T2).
//!
//! The orchestrator queue treats this module as the single source of truth for
//! cross-process file locks. The in-memory `FileLockManager` is a write-through
//! cache; reconciliation on daemon start replays from these tables.

use crate::{StoreError, VoxDb};
use serde::{Deserialize, Serialize};

/// Lock kind discriminator persisted as TEXT in `vcs_lock.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockKindRow {
    Exclusive,
    SharedRead,
}

impl LockKindRow {
    pub fn as_sql(&self) -> &'static str {
        match self {
            LockKindRow::Exclusive => "exclusive",
            LockKindRow::SharedRead => "shared_read",
        }
    }

    pub fn from_sql(s: &str) -> Option<Self> {
        match s {
            "exclusive" => Some(LockKindRow::Exclusive),
            "shared_read" => Some(LockKindRow::SharedRead),
            _ => None,
        }
    }
}

/// One row of the `vcs_lock` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcsLockRow {
    pub path: String,
    pub kind: LockKindRow,
    /// `AgentId.0.to_string()` of the lock holder.
    pub holder: String,
    pub holder_node_id: String,
    pub repository_id: String,
    pub acquired_at: i64,
    pub expires_at: i64,
    pub lease_id: Option<String>,
    pub fence_token: i64,
}

/// One row of the `lock_leader` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockLeaderRow {
    pub repository_id: String,
    pub leader_node_id: String,
    pub elected_at: i64,
    pub heartbeat_at: i64,
    pub expires_at: i64,
    pub epoch: i64,
}

impl VoxDb {
    /// Upsert a `vcs_lock` row. The primary key is `path`; re-acquiring by the
    /// same holder refreshes `expires_at` and atomically increments `fence_token`.
    pub async fn mesh_locks_upsert(&self, row: &VcsLockRow) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO vcs_lock(path, kind, holder, holder_node_id, repository_id, \
                                      acquired_at, expires_at, lease_id, fence_token) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(path) DO UPDATE SET \
                     kind            = excluded.kind, \
                     holder          = excluded.holder, \
                     holder_node_id  = excluded.holder_node_id, \
                     acquired_at     = excluded.acquired_at, \
                     expires_at      = excluded.expires_at, \
                     lease_id        = excluded.lease_id, \
                     fence_token     = vcs_lock.fence_token + 1",
                turso::params![
                    row.path.clone(),
                    row.kind.as_sql().to_string(),
                    row.holder.clone(),
                    row.holder_node_id.clone(),
                    row.repository_id.clone(),
                    row.acquired_at,
                    row.expires_at,
                    row.lease_id.clone(),
                    row.fence_token,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Release a `vcs_lock` only when `holder_node_id` matches.
    /// Returns the number of rows deleted (0 or 1).
    ///
    /// Uses a SELECT-before-DELETE approach because Turso reports `changes()`
    /// as total connection changes, not per-statement changes.
    pub async fn mesh_locks_release(
        &self,
        path: &str,
        holder_node_id: &str,
    ) -> Result<u64, StoreError> {
        let mut check = self
            .conn
            .query(
                "SELECT 1 FROM vcs_lock WHERE path = ?1 AND holder_node_id = ?2 LIMIT 1",
                turso::params![path.to_string(), holder_node_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;

        if check.next().await.map_err(StoreError::Turso)?.is_none() {
            return Ok(0);
        }

        self.conn
            .execute(
                "DELETE FROM vcs_lock WHERE path = ?1 AND holder_node_id = ?2",
                turso::params![path.to_string(), holder_node_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(1)
    }

    /// Load all `vcs_lock` rows for a repository (WAL replay on daemon start).
    pub async fn mesh_locks_for_repo(
        &self,
        repository_id: &str,
    ) -> Result<Vec<VcsLockRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT path, kind, holder, holder_node_id, repository_id, \
                        acquired_at, expires_at, lease_id, fence_token \
                 FROM vcs_lock WHERE repository_id = ?1",
                turso::params![repository_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let kind_str: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let kind = LockKindRow::from_sql(&kind_str)
                .ok_or_else(|| StoreError::Db(format!("unknown lock kind: {kind_str}")))?;
            out.push(VcsLockRow {
                path: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                kind,
                holder: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                holder_node_id: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                repository_id: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                acquired_at: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                expires_at: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                lease_id: row.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                fence_token: row.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// Prune `vcs_lock` rows whose `expires_at` is older than `now_ms`.
    /// Returns the number of rows pruned (approximate — Turso `changes()` is
    /// cumulative per connection so we count matching rows before deleting).
    pub async fn mesh_locks_prune_expired(&self, now_ms: i64) -> Result<u64, StoreError> {
        let mut count_rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM vcs_lock WHERE expires_at < ?1",
                turso::params![now_ms],
            )
            .await
            .map_err(StoreError::Turso)?;
        let count = if let Some(row) = count_rows.next().await.map_err(StoreError::Turso)? {
            row.get::<i64>(0).unwrap_or(0).max(0) as u64
        } else {
            0
        };

        self.conn
            .execute(
                "DELETE FROM vcs_lock WHERE expires_at < ?1",
                turso::params![now_ms],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(count)
    }

    /// Compare-and-swap insert into `lock_leader`. Returns `true` if this node
    /// is now the leader, `false` if another node holds an unexpired claim.
    ///
    /// Uses a read-before-write pattern because Turso's `changes()` is
    /// cumulative per connection, not per-statement.
    pub async fn lock_leader_try_claim(
        &self,
        repository_id: &str,
        candidate_node_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<bool, StoreError> {
        // Check if an unexpired row exists for a different node.
        let mut blocker = self
            .conn
            .query(
                "SELECT leader_node_id FROM lock_leader \
                 WHERE repository_id = ?1 AND expires_at >= ?2 AND leader_node_id != ?3 \
                 LIMIT 1",
                turso::params![
                    repository_id.to_string(),
                    now_ms,
                    candidate_node_id.to_string(),
                ],
            )
            .await
            .map_err(StoreError::Turso)?;

        if blocker.next().await.map_err(StoreError::Turso)?.is_some() {
            return Ok(false);
        }

        let expires_at = now_ms + ttl_ms;
        self.conn
            .execute(
                "INSERT INTO lock_leader(repository_id, leader_node_id, elected_at, \
                                         heartbeat_at, expires_at, epoch) \
                 VALUES (?1, ?2, ?3, ?3, ?4, 0) \
                 ON CONFLICT(repository_id) DO UPDATE SET \
                     leader_node_id = excluded.leader_node_id, \
                     elected_at     = excluded.elected_at, \
                     heartbeat_at   = excluded.heartbeat_at, \
                     expires_at     = excluded.expires_at, \
                     epoch          = lock_leader.epoch + 1",
                turso::params![
                    repository_id.to_string(),
                    candidate_node_id.to_string(),
                    now_ms,
                    expires_at,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(true)
    }

    /// Refresh the leader's heartbeat. Returns `true` if still leader, `false`
    /// if the row's `leader_node_id` no longer matches (preempted).
    pub async fn lock_leader_heartbeat(
        &self,
        repository_id: &str,
        leader_node_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<bool, StoreError> {
        // Check before updating to avoid relying on Turso's cumulative changes().
        let mut check = self
            .conn
            .query(
                "SELECT 1 FROM lock_leader \
                 WHERE repository_id = ?1 AND leader_node_id = ?2 LIMIT 1",
                turso::params![repository_id.to_string(), leader_node_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;

        if check.next().await.map_err(StoreError::Turso)?.is_none() {
            return Ok(false);
        }

        let expires_at = now_ms + ttl_ms;
        self.conn
            .execute(
                "UPDATE lock_leader \
                 SET heartbeat_at = ?3, expires_at = ?4 \
                 WHERE repository_id = ?1 AND leader_node_id = ?2",
                turso::params![
                    repository_id.to_string(),
                    leader_node_id.to_string(),
                    now_ms,
                    expires_at,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(true)
    }

    /// Read the current leader row for a repository, if any.
    pub async fn lock_leader_get(
        &self,
        repository_id: &str,
    ) -> Result<Option<LockLeaderRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT repository_id, leader_node_id, elected_at, heartbeat_at, \
                        expires_at, epoch \
                 FROM lock_leader WHERE repository_id = ?1",
                turso::params![repository_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;

        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            Ok(Some(LockLeaderRow {
                repository_id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                leader_node_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                elected_at: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                heartbeat_at: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                expires_at: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                epoch: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }
}
