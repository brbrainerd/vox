//! Codex methods for the SCIENTIA research pipeline (Phase 0d).
//!
//! These implement the DB half of the stubs in `vox-dei-shim/src/research/orchestrator/pipeline.rs`.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;
use vox_db_types::{
    ClaimsPendingCounts, ResearchArtifactRecord, ResearchSessionRecord, ResearchSessionSummary,
    ScientiaClaimWithVerdict,
};

impl VoxDb {
    /// Create a new research session and return its row id.
    pub async fn create_research_session(
        &self,
        session_key: &str,
        query_text: &str,
    ) -> Result<i64, StoreError> {
        let now = now_ms();
        let key = session_key.to_string();
        let q = query_text.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO scientia_research_sessions \
                     (session_key, status, started_at_ms, query_text) \
                     VALUES (?1, 'queued', ?2, ?3)",
                    params![key.as_str(), now, q.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Update the status of a research session.
    pub async fn update_research_session_status(
        &self,
        session_id: i64,
        status: &str,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let s = status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE scientia_research_sessions \
                     SET status = ?1, finished_at_ms = ?2 \
                     WHERE id = ?3",
                    params![s.as_str(), now, session_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch one SCIENTIA research session by row id.
    pub async fn get_research_session(
        &self,
        session_id: i64,
    ) -> Result<Option<ResearchSessionRecord>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, session_key, status, started_at_ms, finished_at_ms, query_text \
                         FROM scientia_research_sessions WHERE id = ?1",
                        params![session_id],
                    )
                    .await?;
                let Some(row) = rows.next().await? else {
                    return Ok::<Option<ResearchSessionRecord>, StoreError>(None);
                };
                Ok(Some(ResearchSessionRecord {
                    id: row.get::<i64>(0)?,
                    session_key: row.get::<String>(1)?,
                    status: row.get::<String>(2)?,
                    started_at_ms: row.get::<i64>(3)?,
                    finished_at_ms: row.get::<Option<i64>>(4)?,
                    query_text: row.get::<String>(5)?,
                }))
            })
            .await
    }

    /// List recent SCIENTIA research sessions newest-first.
    pub async fn list_recent_research_sessions(
        &self,
        limit: u32,
    ) -> Result<Vec<ResearchSessionSummary>, StoreError> {
        let lim = limit.clamp(1, 200) as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, session_key, status, started_at_ms, finished_at_ms, query_text \
                         FROM scientia_research_sessions ORDER BY started_at_ms DESC, id DESC LIMIT ?1",
                        params![lim],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(ResearchSessionSummary {
                        id: row.get::<i64>(0)?,
                        session_key: row.get::<String>(1)?,
                        status: row.get::<String>(2)?,
                        started_at_ms: row.get::<i64>(3)?,
                        finished_at_ms: row.get::<Option<i64>>(4)?,
                        query_text: row.get::<String>(5)?,
                    });
                }
                Ok::<Vec<ResearchSessionSummary>, StoreError>(out)
            })
            .await
    }

    /// Record a single research pipeline metric.
    ///
    /// Maps onto the existing `research_metrics` table (column `metric_value`, `created_at`).
    pub async fn record_research_metric(
        &self,
        session_id: i64,
        metric_type: &str,
        value: f64,
        metadata_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let sid = session_id.to_string();
        let mt = metric_type.to_string();
        let meta = metadata_json.map(|s| s.to_string());
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO research_metrics \
                     (session_id, metric_type, metric_value, metadata_json) \
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        sid.as_str(),
                        mt.as_str(),
                        value,
                        meta.as_deref().unwrap_or("{}")
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Store an extracted atomic claim.
    pub async fn store_claim(
        &self,
        session_id: i64,
        claim_id: u64,
        text: &str,
        is_numeric: bool,
        is_recent: bool,
        is_named_event: bool,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let t = text.to_string();
        let cid = claim_id as i64;
        let num = is_numeric as i64;
        let rec = is_recent as i64;
        let named = is_named_event as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO scientia_claims \
                     (claim_id, session_id, text, is_numeric, is_recent, is_named_event, created_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![cid, session_id, t.as_str(), num, rec, named, now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Set the verifiability score for a stored claim (best-effort; a no-op if
    /// the claim row does not exist). Kept separate from `store_claim` so its
    /// signature stays stable for existing callers.
    pub async fn update_claim_verifiability_score(
        &self,
        claim_id: u64,
        score: f64,
    ) -> Result<(), StoreError> {
        let cid = claim_id as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE scientia_claims SET verifiability_score = ?2 WHERE claim_id = ?1",
                    params![cid, score],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Store a claim verification verdict.
    pub async fn store_claim_verdict(
        &self,
        claim_id: u64,
        verdict: &str,
        confidence: f64,
        verifier_model: &str,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let cid = claim_id as i64;
        let v = verdict.to_string();
        let vm = verifier_model.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_claim_verdicts \
                     (claim_id, verdict, confidence, verifier_model, created_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![cid, v.as_str(), confidence, vm.as_str(), now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Store an evidence span for a claim verdict.
    pub async fn store_evidence_span(
        &self,
        claim_id: u64,
        span_start: usize,
        span_end: usize,
        span_text: &str,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let cid = claim_id as i64;
        let st = span_text.to_string();
        let ss = span_start as i64;
        let se = span_end as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_claim_verdicts \
                     (claim_id, verdict, confidence, span_start, span_end, span_text, created_at_ms) \
                     VALUES (?1, 'Unverified', 0.0, ?2, ?3, ?4, ?5)",
                    params![cid, ss, se, st.as_str(), now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List a publication's extracted claims, each joined to its latest
    /// non-span verdict. `session_id` is derived from the publication id by the
    /// caller (vox-cli). Newest claims first.
    pub async fn list_publication_claims(
        &self,
        session_id: i64,
    ) -> Result<Vec<ScientiaClaimWithVerdict>, StoreError> {
        let rows = self
            .query_all(
                "SELECT c.claim_id, c.text, c.is_numeric, c.verifiability_score, \
                   (SELECT v.verdict FROM scientia_claim_verdicts v \
                    WHERE v.claim_id = c.claim_id AND v.verdict <> 'Unverified' \
                    ORDER BY v.created_at_ms DESC, v.id DESC LIMIT 1) AS verdict, \
                   (SELECT v.confidence FROM scientia_claim_verdicts v \
                    WHERE v.claim_id = c.claim_id AND v.verdict <> 'Unverified' \
                    ORDER BY v.created_at_ms DESC, v.id DESC LIMIT 1) AS confidence, \
                   (SELECT v.verifier_model FROM scientia_claim_verdicts v \
                    WHERE v.claim_id = c.claim_id AND v.verdict <> 'Unverified' \
                    ORDER BY v.created_at_ms DESC, v.id DESC LIMIT 1) AS verifier_model, \
                   c.created_at_ms \
                 FROM scientia_claims c \
                 WHERE c.session_id = ?1 \
                 ORDER BY c.created_at_ms DESC, c.claim_id DESC",
                (session_id,),
            )
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let is_numeric: i64 = r.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(ScientiaClaimWithVerdict {
                claim_id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                text: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                is_numeric: is_numeric != 0,
                verifiability_score: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                verdict: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                confidence: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                verifier_model: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// Global claims-pending counts for the SCIENTIA dashboard: each claim
    /// bucketed by its latest non-span verdict (`Supported` → verifiable,
    /// `Abstain` → abstained, none yet → extraction_running).
    pub async fn scientia_claims_pending_summary(&self) -> Result<ClaimsPendingCounts, StoreError> {
        let rows = self
            .query_all(
                "SELECT \
                   COALESCE(SUM(CASE WHEN verdict = 'Supported' THEN 1 ELSE 0 END), 0), \
                   COALESCE(SUM(CASE WHEN verdict = 'Abstain' THEN 1 ELSE 0 END), 0), \
                   COALESCE(SUM(CASE WHEN verdict IS NULL THEN 1 ELSE 0 END), 0) \
                 FROM ( \
                   SELECT (SELECT v.verdict FROM scientia_claim_verdicts v \
                           WHERE v.claim_id = c.claim_id AND v.verdict <> 'Unverified' \
                           ORDER BY v.created_at_ms DESC, v.id DESC LIMIT 1) AS verdict \
                   FROM scientia_claims c \
                 )",
                (),
            )
            .await?;
        match rows.into_iter().next() {
            Some(r) => Ok(ClaimsPendingCounts {
                verifiable: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                abstained: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                extraction_running: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            }),
            None => Ok(ClaimsPendingCounts {
                verifiable: 0,
                abstained: 0,
                extraction_running: 0,
            }),
        }
    }

    /// Store a training pair (query + answer + quality score).
    pub async fn store_training_pair(
        &self,
        session_id: i64,
        query: &str,
        answer: &str,
        quality_score: i32,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let q = query.to_string();
        let a = answer.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_training_pairs \
                     (session_id, query_text, answer_text, quality_score, created_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![session_id, q.as_str(), a.as_str(), quality_score, now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Upsert the durable artifact for a completed research session.
    pub async fn store_research_artifact(
        &self,
        session_id: i64,
        artifact_json: &str,
        report_markdown: &str,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let artifact = artifact_json.to_string();
        let report = report_markdown.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_research_artifacts \
                     (session_id, artifact_json, report_markdown, created_at_ms, updated_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?4) \
                     ON CONFLICT(session_id) DO UPDATE SET \
                       artifact_json = excluded.artifact_json, \
                       report_markdown = excluded.report_markdown, \
                       updated_at_ms = excluded.updated_at_ms",
                    params![session_id, artifact.as_str(), report.as_str(), now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch the latest durable artifact for a research session.
    pub async fn get_research_artifact(
        &self,
        session_id: i64,
    ) -> Result<Option<ResearchArtifactRecord>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT session_id, artifact_json, report_markdown, created_at_ms, updated_at_ms \
                         FROM scientia_research_artifacts WHERE session_id = ?1",
                        params![session_id],
                    )
                    .await?;
                let Some(row) = rows.next().await? else {
                    return Ok::<Option<ResearchArtifactRecord>, StoreError>(None);
                };
                Ok(Some(ResearchArtifactRecord {
                    session_id: row.get::<i64>(0)?,
                    artifact_json: row.get::<String>(1)?,
                    report_markdown: row.get::<String>(2)?,
                    created_at_ms: row.get::<i64>(3)?,
                    updated_at_ms: row.get::<i64>(4)?,
                }))
            })
            .await
    }

    /// List memory entries by type (uses `knowledge_nodes` table). Returns content strings.
    pub async fn list_memories_by_type(
        &self,
        memory_type: &str,
        limit: u32,
    ) -> Result<Vec<String>, StoreError> {
        let mt = memory_type.to_string();
        let lim = limit as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let rows = breaker
            .call(|| async move {
                let mut stmt = conn
                    .query(
                        "SELECT content FROM knowledge_nodes \
                         WHERE node_type = ?1 \
                         ORDER BY created_at DESC LIMIT ?2",
                        params![mt.as_str(), lim],
                    )
                    .await?;
                let mut results = Vec::new();
                while let Some(row) = stmt.next().await? {
                    if let Ok(content) = row.get::<String>(0) {
                        results.push(content);
                    }
                }
                Ok::<Vec<String>, StoreError>(results)
            })
            .await?;
        Ok(rows)
    }

    /// Get the retrieval configuration from the DB (returns defaults if not configured).
    ///
    /// Phase 1 will persist this to a config table; for now a sensible static default is returned.
    pub async fn get_retrieval_config(&self) -> Result<serde_json::Value, StoreError> {
        Ok(serde_json::json!({
            "max_sources": 10,
            "min_score": 0.3,
            "timeout_ms": 30000
        }))
    }

    /// Start a provider search run within a session. Returns the new run row id.
    pub async fn start_provider_run(
        &self,
        session_id: i64,
        provider_name: &str,
    ) -> Result<i64, StoreError> {
        let now = now_ms();
        let pn = provider_name.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_provider_runs \
                     (session_id, provider_name, started_at_ms) \
                     VALUES (?1, ?2, ?3)",
                    params![session_id, pn.as_str(), now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Record a research source URL found during a provider run.
    ///
    /// Uses `knowledge_nodes` with `node_type = 'research_source'`. Returns the last insert rowid.
    pub async fn create_research_source(
        &self,
        session_id: i64,
        url: &str,
        title: Option<&str>,
    ) -> Result<i64, StoreError> {
        let u = url.to_string();
        let label = title.unwrap_or(url).to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO knowledge_nodes \
                     (id, label, content, node_type, metadata, created_at) \
                     VALUES (?1, ?2, '', 'research_source', '{}', datetime('now'))",
                    params![u.as_str(), label.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        // session_id correlation handled at the provider_runs level
        let _ = session_id;
        Ok(self.conn.last_insert_rowid())
    }

    /// Upsert a model profile metric using a running average (Mesh §5.7 / Phase 6).
    ///
    /// On first insert, `profile_value` and `sample_count = 1` are stored.
    /// On subsequent calls, the running mean is updated:
    ///   `new_mean = (old_mean * n + new_value) / (n + 1)`.
    pub async fn rollup_model_scoreboard_with_scientia(
        &self,
        provider: &str,
        model_id: &str,
        profile_key: &str,
        new_value: f64,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let p = provider.to_string();
        let m = model_id.to_string();
        let k = profile_key.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scientia_model_profile_learning \
                     (provider, model_id, profile_key, profile_value, sample_count, window_start_ms, window_end_ms, updated_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, ?5) \
                     ON CONFLICT(provider, model_id, profile_key) DO UPDATE SET \
                       profile_value = (profile_value * sample_count + excluded.profile_value) / (sample_count + 1), \
                       sample_count = sample_count + 1, \
                       window_end_ms = excluded.window_end_ms, \
                       updated_at_ms = excluded.updated_at_ms",
                    params![p.as_str(), m.as_str(), k.as_str(), new_value, now],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Mark a provider search run as complete.
    pub async fn finish_provider_run(
        &self,
        run_id: i64,
        hit_count: u32,
        elapsed_ms: u64,
    ) -> Result<(), StoreError> {
        let now = now_ms();
        let hc = hit_count as i64;
        let em = elapsed_ms as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE scientia_provider_runs \
                     SET hit_count = ?1, elapsed_ms = ?2, finished_at_ms = ?3 \
                     WHERE id = ?4",
                    params![hc, em, now, run_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
