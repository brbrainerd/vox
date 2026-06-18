use turso::params;
use crate::store::types::{MemoryEntry, SaveMemoryParams, StoreError};
use vox_db_types::{DbAgentId, DbSessionId};

impl crate::VoxDb {
    /// Append a row to `memories`. Returns the inserted `rowid`.
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::store_memory`.
    pub async fn save_memory(&self, p: SaveMemoryParams<'_>) -> Result<i64, StoreError> {
        let agent_id = p.agent_id.to_string();
        let session_id = p.session_id.to_string();
        let memory_type = p.memory_type.to_string();
        let content = p.content.to_string();
        let metadata = p.metadata.map(str::to_string);
        let importance = p.importance;
        let vcs_snapshot_id = p.vcs_snapshot_id.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO memories
                         (agent_id, session_id, memory_type, content, metadata, importance, vcs_snapshot_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        agent_id.as_str(),
                        session_id.as_str(),
                        memory_type.as_str(),
                        content.as_str(),
                        metadata.as_deref(),
                        importance,
                        vcs_snapshot_id.as_deref(),
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Fetch recent `memories` for `agent_id`, newest first.
    ///
    /// Pass `memory_type = Some("…")` to filter; `_session_id` is accepted for API compatibility
    /// but not yet applied to avoid over-restricting results.
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::recall_memory`.
    pub async fn recall_memory(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: i64,
        _session_id: Option<&str>,
    ) -> Result<Vec<MemoryEntry>, StoreError> {
        let lim = limit.clamp(1, 10_000);
        let mut rows = match memory_type {
            Some(t) => {
                self.conn
                    .query(
                        "SELECT id, agent_id, session_id, memory_type, content, metadata,
                                importance, created_at
                         FROM memories
                         WHERE agent_id = ?1 AND memory_type = ?2
                         ORDER BY created_at DESC LIMIT ?3",
                        params![agent_id, t, lim],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT id, agent_id, session_id, memory_type, content, metadata,
                                importance, created_at
                         FROM memories
                         WHERE agent_id = ?1
                         ORDER BY created_at DESC LIMIT ?2",
                        params![agent_id, lim],
                    )
                    .await?
            }
        };
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(MemoryEntry {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                agent_id: DbAgentId::new(
                    row.get::<String>(1)
                        .map_err(|e| StoreError::Db(e.to_string()))?,
                ),
                session_id: DbSessionId::new(
                    row.get::<String>(2)
                        .map_err(|e| StoreError::Db(e.to_string()))?,
                ),
                memory_type: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                content: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                metadata: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                importance: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at: row.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    pub async fn get_memory_status_counts(&self) -> Result<(usize, usize), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT
                            SUM(CASE WHEN status IN ('pending','queued','in_progress') THEN 1 ELSE 0 END) AS active,
                            SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed
                        FROM memories",
                        (),
                    )
                    .await?;
                let mut active = 0_usize;
                let mut completed = 0_usize;
                if let Some(row) = rows.next().await? {
                    active = row.get::<i64>(0).unwrap_or(0) as usize;
                    completed = row.get::<i64>(1).unwrap_or(0) as usize;
                }
                Ok::<_, StoreError>((active, completed))
            })
            .await
    }
}
