use turso::params;
use crate::store::types::StoreError;
use crate::{
    RetrievalEvidenceSource, RetrievalResult, SearchBackend, SearchDiagnostics, fuse_hybrid_results,
};

impl crate::VoxDb {
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
            let q = super::sanitize_fts_query(query);
            if !q.is_empty()
                && let Ok(out) = self.query_search_document_chunks_fts(&q, lim).await
                    && !out.is_empty() {
                        return Ok(out);
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
