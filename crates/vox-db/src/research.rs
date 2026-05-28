//! Ingest external research into **Codex** (`knowledge_nodes`, `snippets`) and maintain
//! `codex_capability_map` for competitive / capability tracking.
//!
//! Use [`crate::VoxDb::ingest_research_document_async`] or [`crate::VoxDb::ingest_research_document`]
//! with a [`ResearchIngestRequest`]. `codex_capability_map` is created by Arca baseline.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

pub use vox_db_types::store_types::research::*;

fn blake3_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

fn chunk_text(body: &str, max_chunk: usize) -> Vec<String> {
    if body.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in body.split_whitespace() {
        if cur.len() + word.len() + 1 > max_chunk && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

impl VoxDb {
    /// Persist a fetched document: metadata JSON in `knowledge_nodes`, text chunks in `snippets`.
    ///
    /// Overwrites [`ResearchIngestRequest::packet`].`content_hash` from `body`. Mutates `req` in place.
    pub async fn ingest_research_document_async(
        &self,
        req: &mut ResearchIngestRequest,
    ) -> Result<ResearchIngestResult, StoreError> {
        let hash = blake3_hex(req.body.as_bytes());
        req.packet.content_hash = hash.clone();
        let node_id = format!("research:{hash}");
        let meta = serde_json::to_string(&req.packet)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        let title = req.packet.title.clone();
        let body = req.body.clone();
        let meta_exec = meta.clone();
        let nid = node_id.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO knowledge_nodes (id, label, content, node_type, metadata, created_at)
                 VALUES (?1, ?2, ?3, 'external_research', ?4, datetime('now'))",
                    params![nid.as_str(), title.as_str(), body.as_str(), meta_exec.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;

        let packet_rowid = self.conn.last_insert_rowid();

        let source_url_cl = req.packet.source_url.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM snippets WHERE language = 'research_chunk' AND source_ref = ?1",
                    params![source_url_cl.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;

        let chunks = chunk_text(&req.body, 2048);
        let mut chunk_ids = Vec::new();
        let topic = req.packet.topic.clone();
        let summary = req.packet.summary.clone();
        let tags_joined = req.packet.tags.join(",");
        let source_url = req.packet.source_url.clone();
        for (i, chunk) in chunks.iter().enumerate() {
            let title = format!("{}#{}", topic, i);
            let chunk = chunk.to_string();
            let summary_cl = summary.clone();
            let tags_cl = tags_joined.clone();
            let url_cl = source_url.clone();
            let breaker = self.breaker.clone();
            let conn = self.conn.clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "INSERT INTO snippets (language, title, code, description, tags, author_id, source_ref, embedding_ref)
                     VALUES ('research_chunk', ?1, ?2, ?3, ?4, NULL, ?5, NULL)",
                        params![
                            title.as_str(),
                            chunk.as_str(),
                            summary_cl.as_str(),
                            tags_cl.as_str(),
                            url_cl.as_str(),
                        ],
                    )
                    .await?;
                    Ok::<(), StoreError>(())
                })
                .await?;
            chunk_ids.push(self.conn.last_insert_rowid());
        }

        let document_id = self
            .upsert_search_document(
                &req.packet.source_url,
                &req.packet.title,
                &req.packet.source_type,
                &hash,
            )
            .await?;
        let embedding_refs = vec![None; chunks.len()];
        self.replace_search_document_chunks_with_refs(document_id, &chunks, &embedding_refs)
            .await?;
        if req.embeddings.len() == chunks.len() {
            for (idx, vector) in req.embeddings.iter().enumerate() {
                if vector.is_empty() {
                    continue;
                }
                let rows = self
                    .query_all(
                        "SELECT id FROM search_document_chunks
                         WHERE document_id = ?1 AND chunk_index = ?2 LIMIT 1",
                        params![document_id, idx as i64],
                    )
                    .await?;
                let Some(row) = rows.into_iter().next() else {
                    continue;
                };
                let chunk_id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                let snippet: String = chunks[idx].chars().take(240).collect();
                let embedding_id = self
                    .store_embedding(
                        "search_document_chunk",
                        &chunk_id.to_string(),
                        "research_ingest",
                        vector,
                        Some(snippet.as_str()),
                        None,
                    )
                    .await?;
                self.connection()
                    .execute(
                        "UPDATE search_document_chunks SET embedding_ref = ?1 WHERE id = ?2",
                        params![embedding_id.to_string(), chunk_id],
                    )
                    .await?;
            }
        }

        Ok(ResearchIngestResult {
            packet_id: packet_rowid,
            document_id: Some(document_id),
            chunk_ids,
            kb_id: req.kb_id.clone(),
            content_hash: hash,
        })
    }

    /// Same as [`Self::ingest_research_document_async`] but blocks on the store’s internal runtime.
    pub fn ingest_research_document(
        &self,
        req: &mut ResearchIngestRequest,
    ) -> Result<ResearchIngestResult, StoreError> {
        self.block_on(self.ingest_research_document_async(req))
    }

    /// Deserialize recent `external_research` rows from `knowledge_nodes` (optional vendor/topic filter).
    pub fn list_research_packets(
        &self,
        vendor: Option<&str>,
        topic: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ExternalResearchPacket>, StoreError> {
        let limit = limit.clamp(1, 50_000);
        let vendor = vendor.map(str::to_string);
        let topic = topic.map(str::to_string);
        self.block_on(async {
            let lim = limit;
            let mut rows = match (&vendor, &topic) {
                (Some(v), Some(t)) => {
                    self
                        .connection()
                        .query(
                            "SELECT metadata FROM knowledge_nodes WHERE node_type = 'external_research'
                             AND json_extract(metadata, '$.vendor') = ?1 AND json_extract(metadata, '$.topic') = ?2
                             ORDER BY created_at DESC LIMIT ?3",
                            params![v.as_str(), t.as_str(), lim],
                        )
                        .await?
                }
                (Some(v), None) => {
                    self
                        .connection()
                        .query(
                            "SELECT metadata FROM knowledge_nodes WHERE node_type = 'external_research'
                             AND json_extract(metadata, '$.vendor') = ?1
                             ORDER BY created_at DESC LIMIT ?2",
                            params![v.as_str(), lim],
                        )
                        .await?
                }
                (None, Some(t)) => {
                    self
                        .connection()
                        .query(
                            "SELECT metadata FROM knowledge_nodes WHERE node_type = 'external_research'
                             AND json_extract(metadata, '$.topic') = ?1
                             ORDER BY created_at DESC LIMIT ?2",
                            params![t.as_str(), lim],
                        )
                        .await?
                }
                (None, None) => {
                    self
                        .connection()
                        .query(
                            "SELECT metadata FROM knowledge_nodes WHERE node_type = 'external_research'
                             ORDER BY created_at DESC LIMIT ?1",
                            params![lim],
                        )
                        .await?
                }
            };
            let mut out = Vec::new();
            while let Some(row) = rows.next().await? {
                let s: String = row.get::<String>(0)?;
                let p: ExternalResearchPacket =
                    serde_json::from_str(&s).map_err(|e| StoreError::Serialization(e.to_string()))?;
                out.push(p);
            }
            Ok(out)
        })
    }

    /// Insert one row into `codex_capability_map` (ensures table exists). Returns SQLite rowid.
    pub fn store_capability_map_record(
        &self,
        rec: &CapabilityMapRecord,
    ) -> Result<i64, StoreError> {
        self.block_on(async {
            let linked = serde_json::to_string(&rec.linked_paths)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            let meta = serde_json::to_string(&rec.metadata)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            let rec = rec.clone();
            let breaker = self.breaker.clone();
            let conn = self.conn.clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "INSERT INTO codex_capability_map (
                        topic, vendor, area, openclaw_capability, vox_evidence, status,
                        advantage_direction, recommended_action, linked_paths_json, metadata_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            rec.topic.as_str(),
                            rec.vendor.as_str(),
                            rec.area.as_str(),
                            rec.openclaw_capability.as_str(),
                            rec.vox_evidence.as_str(),
                            rec.status.as_str(),
                            rec.advantage_direction.as_str(),
                            rec.recommended_action.as_str(),
                            linked.as_str(),
                            meta.as_str(),
                        ],
                    )
                    .await?;
                    Ok::<(), StoreError>(())
                })
                .await?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// List capability-map rows newest-first, optionally filtered by vendor and/or topic.
    pub fn list_capability_map_records(
        &self,
        vendor: Option<&str>,
        topic: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CapabilityMapRecord>, StoreError> {
        let limit = limit.clamp(1, 50_000);
        let vendor = vendor.map(str::to_string);
        let topic = topic.map(str::to_string);
        self.block_on(async {
            let mut rows = match (&vendor, &topic) {
                (Some(v), Some(t)) => {
                    self
                        .connection()
                        .query(
                            "SELECT topic, vendor, area, openclaw_capability, vox_evidence, status,
                                    advantage_direction, recommended_action, linked_paths_json, metadata_json
                             FROM codex_capability_map WHERE vendor = ?1 AND topic = ?2
                             ORDER BY id DESC LIMIT ?3",
                            params![v.as_str(), t.as_str(), limit],
                        )
                        .await?
                }
                (Some(v), None) => {
                    self
                        .connection()
                        .query(
                            "SELECT topic, vendor, area, openclaw_capability, vox_evidence, status,
                                    advantage_direction, recommended_action, linked_paths_json, metadata_json
                             FROM codex_capability_map WHERE vendor = ?1
                             ORDER BY id DESC LIMIT ?2",
                            params![v.as_str(), limit],
                        )
                        .await?
                }
                (None, Some(t)) => {
                    self
                        .connection()
                        .query(
                            "SELECT topic, vendor, area, openclaw_capability, vox_evidence, status,
                                    advantage_direction, recommended_action, linked_paths_json, metadata_json
                             FROM codex_capability_map WHERE topic = ?1
                             ORDER BY id DESC LIMIT ?2",
                            params![t.as_str(), limit],
                        )
                        .await?
                }
                (None, None) => {
                    self
                        .connection()
                        .query(
                            "SELECT topic, vendor, area, openclaw_capability, vox_evidence, status,
                                    advantage_direction, recommended_action, linked_paths_json, metadata_json
                             FROM codex_capability_map
                             ORDER BY id DESC LIMIT ?1",
                            params![limit],
                        )
                        .await?
                }
            };
            let mut out = Vec::new();
            while let Some(row) = rows.next().await? {
                let linked: String = row.get::<String>(8)?;
                let meta: String = row.get::<String>(9)?;
                out.push(CapabilityMapRecord {
                    topic: row.get::<String>(0)?,
                    vendor: row.get::<String>(1)?,
                    area: row.get::<String>(2)?,
                    openclaw_capability: row.get::<String>(3)?,
                    vox_evidence: row.get::<String>(4)?,
                    status: row.get::<String>(5)?,
                    advantage_direction: row.get::<String>(6)?,
                    recommended_action: row.get::<String>(7)?,
                    linked_paths: serde_json::from_str(&linked).unwrap_or_default(),
                    metadata: serde_json::from_str(&meta).unwrap_or_else(|_| serde_json::json!({})),
                });
            }
            Ok(out)
        })
    }

    /// Persist a consolidated research evaluation run record.
    pub async fn record_research_eval_run(
        &self,
        rec: &ResearchEvalRunRecord,
    ) -> Result<i64, StoreError> {
        let config_str = serde_json::to_string(&rec.config)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let metrics_str = serde_json::to_string(&rec.metrics)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tier_str = serde_json::to_string(&rec.tier_distribution)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO research_eval_runs (
                        run_id, model_id, config_json, metrics_json,
                        latency_p50_ms, latency_p99_ms, tier_distribution_json, created_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        rec.run_id,
                        rec.model_id,
                        config_str,
                        metrics_str,
                        rec.latency_p50_ms,
                        rec.latency_p99_ms,
                        tier_str,
                        rec.created_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Persist a single research evaluation sample.
    pub async fn record_research_eval_sample(
        &self,
        rec: &ResearchEvalSampleRecord,
    ) -> Result<i64, StoreError> {
        let evidence_str = serde_json::to_string(&rec.evidence)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        let rec_cl = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO research_eval_samples (
                        run_id, query, gold_answer, model_answer,
                        recall_at_5, groundedness, quality_score, latency_ms,
                        evidence_json, recorded_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        rec_cl.run_id,
                        rec_cl.query,
                        rec_cl.gold_answer,
                        rec_cl.model_answer,
                        rec_cl.recall_at_5,
                        rec_cl.groundedness,
                        rec_cl.quality_score,
                        rec_cl.latency_ms,
                        evidence_str,
                        rec_cl.recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;

        // Also populate the flat telemetry table for evaluation SSOT consistency
        let _ = self
            .insert_telemetry_flat_raw(
                "eval-harness",
                &rec.run_id,
                None,
                "vox-research",
                "ResearchEvalSample",
                Some("eval-search"),
                Some("localized-dispatcher-0.1"),
                Some("local"),
                rec.latency_ms,
                None,
                None,
                None,
                Some(
                    &serde_json::to_string(&serde_json::json!({
                        "query": rec.query,
                        "quality_score": rec.quality_score,
                        "groundedness": rec.groundedness,
                    }))
                    .unwrap_or_default(),
                ),
            )
            .await
            .ok();

        Ok(self.conn.last_insert_rowid())
    }
}

/// Aggregate counts for `vox db retrieval-status` and similar diagnostics.
#[derive(Debug, Clone)]
pub struct RetrievalDiagnostics {
    /// Rows in `embeddings`.
    pub embeddings_count: usize,
    /// Rows in `knowledge_nodes`.
    pub knowledge_nodes_count: usize,
    /// Rows in `knowledge_edges`.
    pub knowledge_edges_count: usize,
    /// Default vector weight for hybrid fusion (informational; not persisted).
    pub vector_weight: f64,
    /// Placeholder for future latency tracking.
    pub last_retrieval_latency_ms: Option<u64>,
    /// Placeholder labels for modality usage splits.
    pub retrieval_mode_splits: Vec<String>,
}

/// Cheap `COUNT(*)` snapshots over retrieval-related tables.
pub fn retrieval_diagnostics(store: &crate::VoxDb) -> Result<RetrievalDiagnostics, StoreError> {
    store.block_on(async {
        let mut r1 = store
            .connection()
            .query("SELECT COUNT(*) FROM embeddings", ())
            .await?;
        let embeddings_count = r1
            .next()
            .await?
            .map(|row| row.get::<i64>(0).unwrap_or(0) as usize)
            .unwrap_or(0);

        let mut r2 = store
            .connection()
            .query("SELECT COUNT(*) FROM knowledge_nodes", ())
            .await?;
        let knowledge_nodes_count = r2
            .next()
            .await?
            .map(|row| row.get::<i64>(0).unwrap_or(0) as usize)
            .unwrap_or(0);

        let mut r3 = store
            .connection()
            .query("SELECT COUNT(*) FROM knowledge_edges", ())
            .await?;
        let knowledge_edges_count = r3
            .next()
            .await?
            .map(|row| row.get::<i64>(0).unwrap_or(0) as usize)
            .unwrap_or(0);

        Ok(RetrievalDiagnostics {
            embeddings_count,
            knowledge_nodes_count,
            knowledge_edges_count,
            vector_weight: 0.65,
            last_retrieval_latency_ms: None,
            retrieval_mode_splits: vec!["hybrid".into(), "vector".into(), "fulltext".into()],
        })
    })
}
