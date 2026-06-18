//! Memory, knowledge graph, embeddings, and component registry for [`VoxDb`].
//!
//! Tables covered (all defined in V3 schema or `schema/domains/agents.rs`):
//! - **`memories`** — episodic agent/session memory rows.
//! - **`knowledge_nodes`** — labelled concept graph nodes (full-text searchable).
//! - **`embeddings`** — raw f32 vector blobs for similarity search.
//! - **`components`** — registered Vox UI/service component metadata.

use turso::params;

use crate::store::types::{EmbeddingEntry, MemoryEntry, SaveMemoryParams, StoreError};
use crate::{
    RetrievalEvidenceSource, RetrievalResult, SearchBackend, SearchDiagnostics, fuse_hybrid_results,
};
use vox_db_types::{DbAgentId, DbSessionId};

impl crate::VoxDb {
    // ── Memories (memories) ───────────────────────────────────────────────────

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

    /// Delete `memories` rows for `agent_id` with `created_at` strictly before `created_before`
    /// (ISO-like timestamp string compared as SQLite TEXT).
    pub async fn delete_memories_created_before(
        &self,
        agent_id: &str,
        created_before: &str,
    ) -> Result<u64, StoreError> {
        let agent_id = agent_id.to_string();
        let created_before = created_before.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn
                    .execute(
                        "DELETE FROM memories WHERE agent_id = ?1 AND created_at < ?2",
                        params![agent_id.as_str(), created_before.as_str()],
                    )
                    .await?;
                Ok::<_, StoreError>(n)
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

    // ── Knowledge Nodes (knowledge_nodes) ────────────────────────────────────

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
            let q = sanitize_fts_query(query);
            if !q.is_empty() {
                if let Ok(out) = self.query_knowledge_nodes_fts(&q, lim).await {
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
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

    /// Full-text search over `search_document_chunks` joined with `search_documents` titles.
    ///
    /// Uses FTS5 when enabled and the shadow table exists; otherwise `body_text LIKE '%q%'`.
    /// Returns `(chunk_id, document_id, body_snippet_200, document_title)`.
    pub async fn query_search_document_chunks(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<(i64, i64, String, String)>, StoreError> {
        let lim = limit.clamp(1, 1_000);
        let use_fts = self
            .sqlite_capabilities_snapshot()
            .await
            .ok()
            .is_some_and(|p| p.fts5_reported);
        if use_fts
            && self
                .search_document_chunks_fts_ready()
                .await
                .unwrap_or(false)
        {
            let q = sanitize_fts_query(query);
            if !q.is_empty() {
                if let Ok(out) = self.query_search_document_chunks_fts(&q, lim).await {
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
        }
        self.query_search_document_chunks_like(query, lim).await
    }

    /// Upsert one `search_documents` row and return its id.
    pub async fn upsert_search_document(
        &self,
        source_uri: &str,
        title: &str,
        mime_type: &str,
        content_hash: &str,
    ) -> Result<i64, StoreError> {
        let source_uri = source_uri.to_string();
        let title = title.to_string();
        let mime_type = mime_type.to_string();
        let content_hash = content_hash.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO search_documents (source_uri, title, mime_type, content_hash)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(source_uri) DO UPDATE SET
                        title = excluded.title,
                        mime_type = excluded.mime_type,
                        content_hash = excluded.content_hash,
                        ingested_at = datetime('now')",
                    params![
                        source_uri.as_str(),
                        title.as_str(),
                        mime_type.as_str(),
                        content_hash.as_str()
                    ],
                )
                .await?;
                let mut rows = conn
                    .query(
                        "SELECT id FROM search_documents WHERE source_uri = ?1 LIMIT 1",
                        params![source_uri.as_str()],
                    )
                    .await?;
                let row = rows.next().await?.ok_or_else(|| {
                    StoreError::Db("search_documents upsert did not return id".into())
                })?;
                let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok::<_, StoreError>(id)
            })
            .await
    }

    /// Replace all chunks for `document_id` with the provided ordered bodies.
    pub async fn replace_search_document_chunks(
        &self,
        document_id: i64,
        chunk_bodies: &[String],
    ) -> Result<(), StoreError> {
        let refs: Vec<Option<String>> = vec![None; chunk_bodies.len()];
        self.replace_search_document_chunks_with_refs(document_id, chunk_bodies, &refs)
            .await
    }

    /// Replace all chunks for `document_id`, preserving optional embedding references per chunk.
    pub async fn replace_search_document_chunks_with_refs(
        &self,
        document_id: i64,
        chunk_bodies: &[String],
        embedding_refs: &[Option<String>],
    ) -> Result<(), StoreError> {
        let chunk_bodies = chunk_bodies.to_vec();
        let embedding_refs = embedding_refs.to_vec();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM search_document_chunks WHERE document_id = ?1",
                    params![document_id],
                )
                .await?;
                for (idx, body) in chunk_bodies.iter().enumerate() {
                    let embedding_ref = embedding_refs
                        .get(idx)
                        .and_then(|r| r.as_deref())
                        .map(str::to_string);
                    conn.execute(
                        "INSERT INTO search_document_chunks (document_id, chunk_index, body_text, embedding_ref)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            document_id,
                            idx as i64,
                            body.as_str(),
                            embedding_ref.as_deref()
                        ],
                    )
                    .await?;
                }
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Hybrid retrieval over normalized `search_document_chunks` using lexical matches and optional
    /// embedding similarity. Returns fused typed results plus execution diagnostics.
    pub async fn query_search_document_chunks_hybrid(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        limit: i64,
        chunk_vector_fusion_weight: f32,
    ) -> Result<(Vec<RetrievalResult>, SearchDiagnostics), StoreError> {
        let lim = limit.clamp(1, 1_000);
        let lexical_rows = self.query_search_document_chunks(query, lim).await?;
        let mut diagnostics = SearchDiagnostics {
            selected_mode: Some(if query_vector.is_some() {
                crate::RetrievalMode::Hybrid
            } else {
                crate::RetrievalMode::FullText
            }),
            initial_top_score: None,
            ..SearchDiagnostics::default()
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut text_hits = Vec::new();
        for (rank, (chunk_id, _doc_id, snippet, title)) in lexical_rows.iter().enumerate() {
            let score = 1.0_f32 / (1.0_f32 + rank as f32);
            text_hits.push(RetrievalResult {
                chunk_id: chunk_id.to_string(),
                source: title.clone(),
                score,
                snippet: snippet.clone(),
                evidence_source: RetrievalEvidenceSource::FullText,
                retrieved_at_ms: Some(now_ms),
                query_id: Some(format!("chunk-text:{query}")),
                supporting_claim_ids: Vec::new(),
                contradiction_hints: Vec::new(),
            });
        }
        if !text_hits.is_empty() {
            diagnostics.backends_used.push(SearchBackend::ChunkFts);
            diagnostics.initial_top_score = text_hits.first().map(|h| f64::from(h.score));
        }

        let mut vector_hits = Vec::new();
        if let Some(vector) = query_vector {
            let embed_rows = self
                .search_similar_embeddings(vector, Some("search_document_chunk"), lim)
                .await?;
            for (entry, sim) in embed_rows {
                let snippet = entry.metadata.unwrap_or_default();
                vector_hits.push(RetrievalResult {
                    chunk_id: entry.source_id.clone(),
                    source: entry.source_id,
                    score: sim.clamp(0.0, 1.0) * 2.0_f32,
                    snippet,
                    evidence_source: RetrievalEvidenceSource::Vector,
                    retrieved_at_ms: Some(now_ms),
                    query_id: Some(format!("chunk-vector:{query}")),
                    supporting_claim_ids: Vec::new(),
                    contradiction_hints: Vec::new(),
                });
            }
            if !vector_hits.is_empty() {
                diagnostics.backends_used.push(SearchBackend::ChunkVector);
            }
        }

        let mut fused = if vector_hits.is_empty() {
            text_hits
        } else if text_hits.is_empty() {
            vector_hits
        } else {
            fuse_hybrid_results(
                &vector_hits,
                &text_hits,
                chunk_vector_fusion_weight.clamp(0.0, 1.0),
            )
        };
        diagnostics.source_diversity = usize::from(!fused.is_empty());
        diagnostics.evidence_quality = fused
            .first()
            .map(|h| f64::from(h.score).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        diagnostics.citation_coverage = if fused.is_empty() { 0.0 } else { 1.0 };
        diagnostics.verified_top_score = fused.first().map(|h| f64::from(h.score));
        diagnostics.verification_top_score_delta = match (
            diagnostics.verified_top_score,
            diagnostics.initial_top_score,
        ) {
            (Some(after), Some(before)) => Some(after - before),
            _ => None,
        };
        fused.truncate(lim as usize);
        Ok((fused, diagnostics))
    }

    async fn search_document_chunks_fts_ready(&self) -> Result<bool, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'search_document_chunks_fts' LIMIT 1",
                (),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn query_search_document_chunks_like(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<(i64, i64, String, String)>, StoreError> {
        let pat = format!("%{query}%");
        let mut rows = self
            .conn
            .query(
                "SELECT c.id, c.document_id, COALESCE(SUBSTR(c.body_text, 1, 200), ''), COALESCE(d.title, '')
                 FROM search_document_chunks c
                 JOIN search_documents d ON d.id = c.document_id
                 WHERE c.body_text LIKE ?1
                 ORDER BY c.created_at DESC LIMIT ?2",
                params![pat, limit],
            )
            .await?;
        collect_search_chunk_rows(&mut rows).await
    }

    async fn query_search_document_chunks_fts(
        &self,
        match_query: &str,
        limit: i64,
    ) -> Result<Vec<(i64, i64, String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT c.id, c.document_id, COALESCE(SUBSTR(c.body_text, 1, 200), ''), COALESCE(d.title, '')
                 FROM search_document_chunks_fts f
                 JOIN search_document_chunks c ON c.rowid = f.rowid
                 JOIN search_documents d ON d.id = c.document_id
                 WHERE search_document_chunks_fts MATCH ?1
                 ORDER BY c.created_at DESC LIMIT ?2",
                params![match_query, limit],
            )
            .await?;
        collect_search_chunk_rows(&mut rows).await
    }

    // ── Embeddings (embeddings) ───────────────────────────────────────────────

    /// Store a raw embedding vector.
    pub async fn store_embedding(
        &self,
        source_type: &str,
        source_id: &str,
        _model: &str,
        vector: &[f32],
        metadata: Option<&str>,
        _vcs_snapshot_id: Option<&str>,
    ) -> Result<i64, StoreError> {
        let source_type = source_type.to_string();
        let source_id = source_id.to_string();
        let metadata = metadata.map(str::to_string);
        let mut blob = Vec::with_capacity(vector.len() * 4);
        for &v in vector {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        let dim = vector.len() as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO embeddings (source_type, source_id, dim, vector, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        source_type.as_str(),
                        source_id.as_str(),
                        dim,
                        blob,
                        metadata.as_deref()
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Brute-force cosine similarity search over the `embeddings` table.
    ///
    /// Fetches up to `limit * 10` candidate rows, scores each, and returns the top `limit`
    /// sorted by similarity descending. Suitable for small tables (< 10 k rows).
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::search_embeddings`.
    pub async fn search_similar_embeddings(
        &self,
        vector: &[f32],
        source_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(EmbeddingEntry, f32)>, StoreError> {
        let lim = limit.clamp(1, 500);
        let probe = self.sqlite_capabilities_snapshot().await.ok();
        let candidate_cap = crate::capabilities::embedding_candidate_cap(lim, 10, probe.as_ref());
        let mut rows = match source_type {
            Some(st) => {
                self.conn
                    .query(
                        "SELECT id, source_type, source_id, dim, vector, metadata
                         FROM embeddings WHERE source_type = ?1
                         ORDER BY created_at DESC LIMIT ?2",
                        params![st, candidate_cap],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT id, source_type, source_id, dim, vector, metadata
                         FROM embeddings ORDER BY created_at DESC LIMIT ?1",
                        params![candidate_cap],
                    )
                    .await?
            }
        };

        let mut scored: Vec<(EmbeddingEntry, f32)> = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let st: Option<String> = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let source_id: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let dim: i64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let blob: Vec<u8> = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            let metadata: Option<String> = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
            // Deserialise little-endian f32 bytes
            let stored: Vec<f32> = blob
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let dot: f32 = vector.iter().zip(stored.iter()).map(|(a, b)| a * b).sum();
            let mag_a: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            let mag_b: f32 = stored.iter().map(|x| x * x).sum::<f32>().sqrt();
            let sim = if mag_a > 0.0 && mag_b > 0.0 {
                dot / (mag_a * mag_b)
            } else {
                0.0
            };
            scored.push((
                EmbeddingEntry {
                    id,
                    source_type: st,
                    source_id,
                    dim,
                    metadata,
                },
                sim,
            ));
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(lim as usize);
        Ok(scored)
    }

    // ── Components (components) ───────────────────────────────────────────────

    /// Upsert a row in `components`.
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::register_local_project`.
    pub async fn register_component(
        &self,
        name: &str,
        namespace: &str,
        schema_hash: Option<&str>,
        description: Option<&str>,
        version: &str,
    ) -> Result<(), StoreError> {
        let name = name.to_string();
        let namespace = namespace.to_string();
        let schema_hash = schema_hash.map(str::to_string);
        let description = description.map(str::to_string);
        let version = version.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO components (name, namespace, schema_hash, version, description)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(name)
                 DO UPDATE SET namespace   = excluded.namespace,
                               schema_hash = COALESCE(excluded.schema_hash, components.schema_hash),
                               version     = excluded.version,
                               description = COALESCE(excluded.description, components.description)",
                    params![
                        name.as_str(),
                        namespace.as_str(),
                        schema_hash.as_deref(),
                        version.as_str(),
                        description.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}

async fn collect_search_chunk_rows(
    rows: &mut turso::Rows,
) -> Result<Vec<(i64, i64, String, String)>, StoreError> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let chunk_id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let doc_id: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        let snippet: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
        let title: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
        out.push((chunk_id, doc_id, snippet, title));
    }
    Ok(out)
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

fn sanitize_fts_query(input: &str) -> String {
    let cleaned = input
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Knowledge Base ops ────────────────────────────────────────────────────────
// These are in a new impl block (Rust allows multiple impl blocks per type).

impl crate::VoxDb {
    /// Insert a new knowledge base row.
    pub async fn kb_create(
        &self,
        id: &str,
        name: &str,
        description: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let name = name.to_string();
        let description = description.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO knowledge_bases (id, name, description, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id.as_str(), name.as_str(), description.as_str(), now_ms],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List all knowledge bases ordered by name.
    pub async fn kb_list(&self) -> Result<Vec<crate::KbRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, name, description, created_at_ms, updated_at_ms, entry_count
                         FROM knowledge_bases ORDER BY name",
                        params![],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(crate::KbRow {
                        id: row.get::<String>(0)?,
                        name: row.get::<String>(1)?,
                        description: row.get::<String>(2)?,
                        created_at_ms: row.get::<i64>(3)?,
                        updated_at_ms: row.get::<i64>(4)?,
                        entry_count: row.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Delete a knowledge base by id (cascades to entries and rules via FK).
    pub async fn kb_delete(&self, id: &str) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute("DELETE FROM knowledge_bases WHERE id = ?1", params![id.as_str()])
                    .await?;
                Ok(())
            })
            .await
    }

    /// Insert a KB entry and increment `entry_count` on the parent KB.
    /// Both SQL statements run in a transaction to prevent drift.
    pub async fn kb_add_entry(
        &self,
        entry_id: &str,
        kb_id: &str,
        content: &str,
        source_signal: &str,
        source_ref: Option<&str>,
        routing_confidence: f64,
        tags: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let source_signal = source_signal.to_string();
        let source_ref = source_ref.map(str::to_string);
        let tags = tags.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                conn.execute(
                    "INSERT INTO kb_entries
                         (id, kb_id, content, source_signal, source_ref, routing_confidence,
                          tags, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        entry_id.as_str(),
                        kb_id.as_str(),
                        content.as_str(),
                        source_signal.as_str(),
                        source_ref.as_deref(),
                        routing_confidence,
                        tags.as_str(),
                        now_ms,
                    ],
                )
                .await?;
                conn.execute(
                    "UPDATE knowledge_bases
                     SET entry_count = entry_count + 1, updated_at_ms = ?2
                     WHERE id = ?1",
                    params![kb_id.as_str(), now_ms],
                )
                .await?;
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List entries for a KB, newest first.
    pub async fn kb_list_entries(
        &self,
        kb_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries WHERE kb_id = ?1
                         ORDER BY created_at_ms DESC LIMIT ?2 OFFSET ?3",
                        params![kb_id.as_str(), limit, offset],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Set the `accepted` flag on an entry and optionally mark it for MENS queuing.
    pub async fn kb_review_entry(
        &self,
        entry_id: &str,
        accepted: bool,
        queue_mens: bool,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let accepted_int: i64 = if accepted { 1 } else { 0 };
        let mens_int: i64 = if queue_mens { 1 } else { 0 };
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE kb_entries SET accepted = ?2, mens_queued = ?3 WHERE id = ?1",
                    params![entry_id.as_str(), accepted_int, mens_int],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Delete a specific entry and decrement the parent KB's `entry_count`.
    pub async fn kb_delete_entry(
        &self,
        entry_id: &str,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                // Fetch the kb_id first so we can decrement entry_count
                let mut rows = conn
                    .query(
                        "SELECT kb_id FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                if let Some(row) = rows.next().await? {
                    let kb_id: String = row.get::<String>(0)?;
                    conn.execute(
                        "DELETE FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                    conn.execute(
                        "UPDATE knowledge_bases
                         SET entry_count = MAX(0, entry_count - 1)
                         WHERE id = ?1",
                        params![kb_id.as_str()],
                    )
                    .await?;
                }
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List recent entries across all KBs (the "knowledge feed"), newest first.
    pub async fn kb_get_feed(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries ORDER BY created_at_ms DESC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Insert a routing rule.
    pub async fn kb_add_rule(
        &self,
        rule_id: &str,
        kb_id: &str,
        rule_type: &str,
        pattern: &str,
        priority: i64,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let rule_id = rule_id.to_string();
        let kb_id = kb_id.to_string();
        let rule_type = rule_type.to_string();
        let pattern = pattern.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO kb_routing_rules
                         (id, kb_id, rule_type, pattern, priority, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        rule_id.as_str(),
                        kb_id.as_str(),
                        rule_type.as_str(),
                        pattern.as_str(),
                        priority,
                        now_ms,
                    ],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List routing rules for a KB, ordered by priority descending.
    pub async fn kb_list_rules(
        &self,
        kb_id: &str,
    ) -> Result<Vec<crate::KbRuleRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, rule_type, pattern, priority, created_at_ms
                         FROM kb_routing_rules WHERE kb_id = ?1
                         ORDER BY priority DESC",
                        params![kb_id.as_str()],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbRuleRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        rule_type: r.get::<String>(2)?,
                        pattern: r.get::<String>(3)?,
                        priority: r.get::<i64>(4)?,
                        created_at_ms: r.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Substring search over accepted KB entries.
    /// Used for BM25-style routing tier and retrieval bundle injection.
    pub async fn kb_search_entries(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let query_lower = format!("%{}%", query.to_ascii_lowercase());
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE accepted = 1 AND lower(content) LIKE ?1
                         ORDER BY routing_confidence DESC, created_at_ms DESC LIMIT ?2",
                        params![query_lower.as_str(), limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Fetch entries not yet queued for MENS training (both accepted and rejected).
    /// Accepted entries → SFT pairs; rejected entries → DPO pairs.
    pub async fn kb_unqueued_training_entries(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE mens_queued = 0
                         ORDER BY created_at_ms ASC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Mark a batch of entries as MENS-queued.
    pub async fn kb_mark_mens_queued(
        &self,
        ids: &[String],
    ) -> Result<(), crate::store::types::StoreError> {
        for id in ids {
            let id = id.clone();
            let conn = self.conn.clone();
            let breaker = self.breaker.clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "UPDATE kb_entries SET mens_queued = 1 WHERE id = ?1",
                        params![id.as_str()],
                    )
                    .await?;
                    Ok::<_, crate::store::types::StoreError>(())
                })
                .await?;
        }
        Ok(())
    }

    /// Check if content already exists in a KB (content-hash deduplication).
    /// Returns the ID of an existing identical entry, or None.
    pub async fn kb_find_duplicate(
        &self,
        kb_id: &str,
        content: &str,
    ) -> Result<Option<String>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id FROM kb_entries WHERE kb_id = ?1 AND content = ?2 LIMIT 1",
                        params![kb_id.as_str(), content.as_str()],
                    )
                    .await?;
                if let Some(r) = rows.next().await? {
                    Ok(Some(r.get::<String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
    }
}
