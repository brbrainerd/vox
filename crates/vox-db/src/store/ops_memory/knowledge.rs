use crate::store::types::StoreError;
use turso::params;

impl crate::VoxDb {
    /// Upsert a knowledge node manually
    pub async fn upsert_knowledge_node(
        &self,
        id: &str,
        label: &str,
        content: &str,
        node_type: Option<&str>,
        metadata: Option<&str>,
        _vcs_snapshot_id: Option<&str>,
    ) -> Result<(), StoreError> {
        let id = id.to_string();
        let label = label.to_string();
        let content = content.to_string();
        let node_type = node_type.map(str::to_string);
        let metadata = metadata.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO knowledge_nodes (id, label, content, node_type, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                         label = excluded.label,
                         content = excluded.content,
                         node_type = excluded.node_type,
                         metadata = excluded.metadata",
                    params![
                        id.as_str(),
                        label.as_str(),
                        content.as_str(),
                        node_type.as_deref(),
                        metadata.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Create an edge between knowledge nodes
    pub async fn create_knowledge_edge(
        &self,
        source_id: &str,
        target_id: &str,
        relation: &str,
        weight: f32,
        _metadata: Option<&str>,
    ) -> Result<(), StoreError> {
        let source_id = source_id.to_string();
        let target_id = target_id.to_string();
        let relation = relation.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO knowledge_edges (src_id, dst_id, relation, weight)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(src_id, dst_id, relation) DO UPDATE SET
                         weight = excluded.weight",
                    params![
                        source_id.as_str(),
                        target_id.as_str(),
                        relation.as_str(),
                        weight
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch neighboring nodes along with their relations
    pub async fn get_knowledge_neighbors(
        &self,
        node_id: &str,
    ) -> Result<Vec<(String, String, String, f32)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT e.dst_id, n.label, e.relation, e.weight
                 FROM knowledge_edges e
                 JOIN knowledge_nodes n ON e.dst_id = n.id
                 WHERE e.src_id = ?1
                 UNION
                 SELECT e.src_id, n.label, e.relation, e.weight
                 FROM knowledge_edges e
                 JOIN knowledge_nodes n ON e.src_id = n.id
                 WHERE e.dst_id = ?1",
                params![node_id],
            )
            .await?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let label: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let rel: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let w: f64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((id, label, rel, w as f32));
        }
        Ok(out)
    }

    /// Full-text LIKE search over `knowledge_nodes` (label + content).
    ///
    /// Returns `(id, label, snippet)` — snippet is the first 200 chars of `content`.
    /// Called from `vox-db/src/lib.rs` `VoxDb::search_memories`.
    pub async fn query_knowledge_nodes(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let lim = limit.clamp(1, 1_000);
        let use_fts = self
            .sqlite_capabilities_snapshot()
            .await
            .ok()
            .is_some_and(|p| p.fts5_reported);
        if use_fts && self.knowledge_nodes_fts_ready().await.unwrap_or(false) {
            let q = super::sanitize_fts_query(query);
            if !q.is_empty()
                && let Ok(out) = self.query_knowledge_nodes_fts(&q, lim).await
                && !out.is_empty()
            {
                return Ok(out);
            }
        }
        self.query_knowledge_nodes_like(query, lim).await
    }

    async fn knowledge_nodes_fts_ready(&self) -> Result<bool, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'knowledge_nodes_fts' LIMIT 1",
                (),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn query_knowledge_nodes_like(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let pat = format!("%{query}%");
        let mut rows = self
            .conn
            .query(
                "SELECT id, label, COALESCE(SUBSTR(content, 1, 200), '')
                 FROM knowledge_nodes
                 WHERE label LIKE ?1 OR content LIKE ?1
                 ORDER BY created_at DESC LIMIT ?2",
                params![pat, limit],
            )
            .await?;
        collect_knowledge_node_rows(&mut rows).await
    }

    async fn query_knowledge_nodes_fts(
        &self,
        match_query: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT k.id, k.label, COALESCE(SUBSTR(k.content, 1, 200), '')
                 FROM knowledge_nodes_fts f
                 JOIN knowledge_nodes k ON k.rowid = f.rowid
                 WHERE knowledge_nodes_fts MATCH ?1
                 ORDER BY k.created_at DESC LIMIT ?2",
                params![match_query, limit],
            )
            .await?;
        collect_knowledge_node_rows(&mut rows).await
    }
}

async fn collect_knowledge_node_rows(
    rows: &mut turso::Rows,
) -> Result<Vec<(String, String, String)>, StoreError> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let label: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        let snippet: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
        out.push((id, label, snippet));
    }
    Ok(out)
}
