//! P0-T3: VoxDb methods for the `mesh_exec_leases` table.
//!
//! Provides `exec_lease_grant` (upsert) and `mesh_exec_lease_for_scope`
//! (most-recent row lookup by scope) so the lease gate in
//! `vox-orchestrator` can enforce the W1 double-execute invariant without
//! depending on the populi HTTP client.

use crate::{StoreError, VoxDb};

/// One row from `mesh_exec_leases`.
#[derive(Debug, Clone)]
pub struct ExecLeaseRow {
    pub lease_id: String,
    pub task_id: String,
    pub scope_key: String,
    pub holder_node_id: String,
    pub granted_at: i64,
    pub expires_at: i64,
    pub state: String,
}

impl VoxDb {
    /// Insert or replace a lease grant in `mesh_exec_leases`.
    ///
    /// Used by test helpers and by the local worker when taking over a scope.
    /// The `state` column is set to `'granted'`.
    pub async fn exec_lease_grant(
        &self,
        lease_id: &str,
        task_id: &str,
        scope_key: &str,
        holder_node_id: &str,
        granted_at: i64,
        expires_at: i64,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO mesh_exec_leases \
                 (lease_id, task_id, scope_key, holder_node_id, granted_at, expires_at, state) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'granted')",
                turso::params![
                    lease_id,
                    task_id,
                    scope_key,
                    holder_node_id,
                    granted_at,
                    expires_at
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Return the most-recently-granted lease row for `scope_key`, or `None`.
    ///
    /// No server-side TTL filter — callers check `expires_at` to distinguish
    /// expired-but-present vs absent.
    pub async fn mesh_exec_lease_for_scope(
        &self,
        scope_key: &str,
    ) -> Result<Option<ExecLeaseRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT lease_id, task_id, scope_key, holder_node_id, granted_at, expires_at, state \
                 FROM mesh_exec_leases \
                 WHERE scope_key = ?1 \
                 ORDER BY granted_at DESC \
                 LIMIT 1",
                turso::params![scope_key],
            )
            .await
            .map_err(StoreError::Turso)?;

        let Some(row) = rows.next().await.map_err(StoreError::Turso)? else {
            return Ok(None);
        };

        Ok(Some(ExecLeaseRow {
            lease_id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            task_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
            scope_key: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            holder_node_id: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            granted_at: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            expires_at: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            state: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
        }))
    }
}
