//! Orchestrator CRUD for [`crate::VoxDb`] (Arca / Turso).
//! Contains methods for distributed locks and heartbeats.

use turso::params;

use crate::store::types::StoreError;

impl crate::VoxDb {
    // ── Distributed Locks (distributed_locks) ────────────────────────────────

    /// Acquire or refresh a distributed lease lock.
    ///
    /// Uses a single upsert with a `WHERE` guard on the conflict branch so an active foreign holder
    /// cannot be overwritten. After the upsert, ownership is verified by reading the live row.
    /// `fence_token` is monotonic per `(lock_key, repository_id)` (max + 1), including refreshes
    /// by the same node.
    ///
    /// Returns `Ok(Ok(fence_token))` when this node holds a non-expired lease, `Ok(Err(holder))`
    /// when another node holds it, or `Err` on database failure.
    ///
    /// When [`crate::DbCircuitBreaker`] is enabled and open, returns [`StoreError::CircuitBreaker`].
    pub async fn acquire_distributed_lock(
        &self,
        lock_key: &str,
        node_id: &str,
        agent_id: &str,
        ttl_secs: i64,
        repository_id: &str,
    ) -> Result<Result<i64, String>, StoreError> {
        let lock_key = lock_key.to_string();
        let node_id = node_id.to_string();
        let agent_id = agent_id.to_string();
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| {
                let lock_key = lock_key.clone();
                let node_id = node_id.clone();
                let agent_id = agent_id.clone();
                let repository_id = repository_id.clone();
                async move {
                    let mut rows = conn
                        .query(
                            "SELECT COALESCE(MAX(fence_token), 0) + 1 AS n
                 FROM distributed_locks
                 WHERE lock_key = ?1 AND repository_id = ?2",
                            params![lock_key.as_str(), repository_id.as_str()],
                        )
                        .await?;

                    let next_fence: i64 = if let Some(row) = rows.next().await? {
                        row.get(0)?
                    } else {
                        1
                    };

                    conn
                        .execute(
                            "INSERT INTO distributed_locks (lock_key, holder_node, holder_agent, fence_token, expires_at, repository_id)
                 VALUES (?1, ?2, ?3, ?4, datetime('now', '+' || ?5 || ' seconds'), ?6)
                 ON CONFLICT(lock_key, repository_id) DO UPDATE SET
                    holder_node = excluded.holder_node,
                    holder_agent = excluded.holder_agent,
                    fence_token = excluded.fence_token,
                    acquired_at = datetime('now'),
                    expires_at = excluded.expires_at
                 WHERE distributed_locks.expires_at <= datetime('now')
                    OR distributed_locks.holder_node = excluded.holder_node",
                            params![
                                lock_key.as_str(),
                                node_id.as_str(),
                                agent_id.as_str(),
                                next_fence,
                                ttl_secs,
                                repository_id.as_str()
                            ],
                        )
                        .await?;

                    let mut rows = conn
                        .query(
                            "SELECT holder_node, fence_token FROM distributed_locks
                 WHERE lock_key = ?1 AND repository_id = ?2 AND expires_at > datetime('now')",
                            params![lock_key.as_str(), repository_id.as_str()],
                        )
                        .await?;

                    let Some(row) = rows.next().await? else {
                        return Ok(Err("_missing_lock_row".to_string()));
                    };
                    let holder: String = row.get(0)?;
                    let fence: i64 = row.get(1)?;
                    if holder != node_id {
                        return Ok(Err(holder));
                    }
                    if fence != next_fence {
                        return Ok(Err("_contended".to_string()));
                    }
                    Ok(Ok(fence))
                }
            })
            .await
    }

    /// Release a distributed lock.
    pub async fn release_distributed_lock(
        &self,
        lock_key: &str,
        node_id: &str,
        repository_id: &str,
    ) -> Result<(), StoreError> {
        let lock_key = lock_key.to_string();
        let node_id = node_id.to_string();
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| {
                let lock_key = lock_key.clone();
                let node_id = node_id.clone();
                let repository_id = repository_id.clone();
                async move {
                    conn.execute(
                        "DELETE FROM distributed_locks WHERE lock_key = ?1 AND holder_node = ?2 AND repository_id = ?3",
                        params![lock_key.as_str(), node_id.as_str(), repository_id.as_str()],
                    )
                    .await?;
                    Ok(())
                }
            })
            .await
    }

    /// Prune all expired distributed locks.
    pub async fn prune_stale_distributed_locks(&self) -> Result<u64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let rows_affected = conn
                    .execute(
                        "DELETE FROM distributed_locks WHERE expires_at <= datetime('now')",
                        (),
                    )
                    .await?;
                Ok(rows_affected)
            })
            .await
    }

    // ── Heartbeats (mesh_heartbeats) ─────────────────────────────────────────

    /// Upsert a node heartbeat.
    pub async fn upsert_mesh_heartbeat(
        &self,
        node_id: &str,
        agent_id: &str,
        activity: &str,
        now_ms: i64,
        repository_id: &str,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let node_id = node_id.to_string();
        let agent_id = agent_id.to_string();
        let activity = activity.to_string();
        let repository_id = repository_id.to_string();
        breaker
            .call(|| {
                let node_id = node_id.clone();
                let agent_id = agent_id.clone();
                let activity = activity.clone();
                let repository_id = repository_id.clone();
                async move {
                    conn.execute(
                        "INSERT INTO mesh_heartbeats (node_id, agent_id, last_seen_ms, activity, repository_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(node_id, repository_id) DO UPDATE SET
                agent_id = excluded.agent_id,
                last_seen_ms = excluded.last_seen_ms,
                activity = excluded.activity",
                        params![
                            node_id.as_str(),
                            agent_id.as_str(),
                            now_ms,
                            activity.as_str(),
                            repository_id.as_str()
                        ],
                    )
                    .await?;
                    Ok(())
                }
            })
            .await
    }

    /// Get live nodes (heartbeats after min_seen).
    pub async fn list_live_nodes(
        &self,
        min_seen_ms: i64,
        repository_id: &str,
    ) -> Result<Vec<Vec<String>>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT node_id, agent_id, activity, CAST(last_seen_ms AS TEXT)
             FROM mesh_heartbeats
             WHERE last_seen_ms >= ?1 AND repository_id = ?2",
                params![min_seen_ms, repository_id],
            )
            .await?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let cols = vec![
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
                row.get::<String>(3)?,
            ];
            out.push(cols);
        }
        Ok(out)
    }

    /// Remove heartbeats older than threshold.
    pub async fn evict_dead_heartbeats(&self, min_seen_ms: i64) -> Result<u64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let affected = conn
                    .execute(
                        "DELETE FROM mesh_heartbeats WHERE last_seen_ms < ?1",
                        params![min_seen_ms],
                    )
                    .await?;
                Ok(affected)
            })
            .await
    }
}
