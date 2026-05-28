use super::common::now_ms;
use crate::{VoxDb, store::StoreError, store::types::*};

impl VoxDb {
    pub async fn upsert_publication_manifest(
        &self,
        params: PublicationManifestParams<'_>,
    ) -> Result<(), StoreError> {
        let ts = now_ms();
        let publication_id = params.publication_id.to_string();
        let content_type = params.content_type.to_string();
        let source_ref = params.source_ref.map(std::string::ToString::to_string);
        let title = params.title.to_string();
        let author = params.author.to_string();
        let abstract_text = params.abstract_text.map(std::string::ToString::to_string);
        let body_markdown = params.body_markdown.to_string();
        let citations_json = params.citations_json.map(std::string::ToString::to_string);
        let metadata_json = params.metadata_json.map(std::string::ToString::to_string);
        let revision_history_json = params
            .revision_history_json
            .map(std::string::ToString::to_string);
        let content_sha3_256 = params.content_sha3_256.to_string();
        let state = params.state.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO publication_manifests (
                    publication_id, content_type, source_ref, title, author, abstract_text,
                    body_markdown, citations_json, metadata_json, revision_history_json, content_sha3_256, state,
                    created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
                ON CONFLICT(publication_id) DO UPDATE SET
                    content_type=excluded.content_type,
                    source_ref=excluded.source_ref,
                    title=excluded.title,
                    author=excluded.author,
                    abstract_text=excluded.abstract_text,
                    body_markdown=excluded.body_markdown,
                    citations_json=excluded.citations_json,
                    metadata_json=excluded.metadata_json,
                    revision_history_json=excluded.revision_history_json,
                    content_sha3_256=excluded.content_sha3_256,
                    state=excluded.state,
                    version=CASE
                        WHEN publication_manifests.content_sha3_256 = excluded.content_sha3_256
                            THEN publication_manifests.version
                        ELSE publication_manifests.version + 1
                    END,
                    updated_at_ms=excluded.updated_at_ms",
                    (
                        publication_id,
                        content_type,
                        source_ref,
                        title,
                        author,
                        abstract_text,
                        body_markdown,
                        citations_json,
                        metadata_json,
                        revision_history_json,
                        content_sha3_256,
                        state,
                        ts,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch one manifest by id.
    pub async fn get_publication_manifest(
        &self,
        publication_id: &str,
    ) -> Result<Option<PublicationManifestRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT publication_id, content_type, source_ref, title, author, abstract_text, body_markdown, citations_json, metadata_json, revision_history_json, content_sha3_256, version, state, created_at_ms, updated_at_ms FROM publication_manifests WHERE publication_id = ?1",
                (publication_id.to_string(),),
            )
            .await?;
        let Some(r) = rows.first() else {
            return Ok(None);
        };
        Ok(Some(PublicationManifestRow {
            publication_id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            content_type: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
            source_ref: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            title: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            author: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            abstract_text: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            body_markdown: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            citations_json: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            metadata_json: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
            revision_history_json: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
            content_sha3_256: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
            version: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
            state: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
            created_at_ms: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
            updated_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
        }))
    }

    /// List manifests for discovery scans (`content_type` / `state` filter when `Some`).
    pub async fn list_publication_manifests(
        &self,
        content_type: Option<&str>,
        state: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PublicationManifestRow>, StoreError> {
        let limit = limit.clamp(1, 500);
        let ct = content_type.map(str::trim).filter(|s| !s.is_empty());
        let st = state.map(str::trim).filter(|s| !s.is_empty());
        let rows = self
            .query_all(
                "SELECT publication_id, content_type, source_ref, title, author, abstract_text, body_markdown, citations_json, metadata_json, revision_history_json, content_sha3_256, version, state, created_at_ms, updated_at_ms FROM publication_manifests WHERE (?1 IS NULL OR content_type = ?1) AND (?2 IS NULL OR state = ?2) ORDER BY updated_at_ms DESC LIMIT ?3",
                (ct, st, limit),
            )
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(PublicationManifestRow {
                publication_id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                content_type: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                source_ref: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                title: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                author: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                abstract_text: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                body_markdown: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                citations_json: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                metadata_json: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                revision_history_json: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
                content_sha3_256: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
                version: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
                state: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
                updated_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// Digest-bound approval for any publication type.
    pub async fn record_publication_approval_for_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
        approver: &str,
    ) -> Result<(), StoreError> {
        let publication_id = publication_id.to_string();
        let content_sha3_256 = content_sha3_256.to_string();
        let approver = approver.to_string();
        let ts = now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO publication_approvals (publication_id, content_sha3_256, approver, approved_at_ms) VALUES (?1, ?2, ?3, ?4)",
                    (publication_id, content_sha3_256, approver, ts),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Count distinct approvers for id+digest.
    pub async fn count_publication_approvers_for_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
    ) -> Result<i64, StoreError> {
        let rows = self
            .query_all(
                "SELECT COUNT(DISTINCT approver) AS c FROM publication_approvals WHERE publication_id = ?1 AND content_sha3_256 = ?2",
                (publication_id.to_string(), content_sha3_256.to_string()),
            )
            .await?;
        let row = rows
            .first()
            .ok_or_else(|| StoreError::Db("publication approval count: no row".into()))?;
        row.get(0).map_err(|e| StoreError::Db(e.to_string()))
    }

    /// True when at least two distinct approvers exist for id+digest.
    pub async fn has_dual_publication_approval_for_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
    ) -> Result<bool, StoreError> {
        Ok(self
            .count_publication_approvers_for_digest(publication_id, content_sha3_256)
            .await?
            >= 2)
    }

    // ── Phase D — solo-critic gate ────────────────────────────────────────

    /// Record an [`ApproverRole::AuditedLLMCritic`]-shaped approval bound to
    /// the same digest as the human approvers. The Rust caller is
    /// responsible for having already validated the critic via
    /// `vox_critic_gate::evaluate_gate`; this store op simply persists.
    ///
    /// [`ApproverRole::AuditedLLMCritic`]: vox_critic_gate::ApproverRole::AuditedLLMCritic
    pub async fn record_publication_critic_approval_for_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
        critic_id: &str,
        critic_fingerprint_json: &str,
        critic_report_uri: Option<&str>,
    ) -> Result<(), StoreError> {
        let publication_id = publication_id.to_string();
        let content_sha3_256 = content_sha3_256.to_string();
        let critic_id = critic_id.to_string();
        let fingerprint = critic_fingerprint_json.to_string();
        let report = critic_report_uri.map(str::to_string);
        let ts = crate::store::ops_publication::manifest::now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR REPLACE INTO publication_approvals (\
                        publication_id, content_sha3_256, approver, approved_at_ms, \
                        approver_role, critic_fingerprint_json, critic_report_uri\
                     ) VALUES (?1, ?2, ?3, ?4, 'audited_llm_critic', ?5, ?6)",
                    (
                        publication_id,
                        content_sha3_256,
                        critic_id,
                        ts,
                        fingerprint,
                        report,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Return `(human_count, critic_count)` distinct approvers for id+digest.
    pub async fn count_publication_approvers_by_role(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
    ) -> Result<(i64, i64), StoreError> {
        let rows = self
            .query_all(
                "SELECT \
                    SUM(CASE WHEN approver_role = 'human' THEN 1 ELSE 0 END), \
                    SUM(CASE WHEN approver_role = 'audited_llm_critic' THEN 1 ELSE 0 END) \
                 FROM (SELECT DISTINCT approver, approver_role \
                       FROM publication_approvals \
                       WHERE publication_id = ?1 AND content_sha3_256 = ?2)",
                (publication_id.to_string(), content_sha3_256.to_string()),
            )
            .await?;
        let row = rows.first().ok_or_else(|| {
            StoreError::Db("publication approval by-role count: no row".into())
        })?;
        // SUM(...) returns NULL when there are no matching rows; coerce to 0.
        let human: Option<i64> =
            row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let critic: Option<i64> =
            row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok((human.unwrap_or(0), critic.unwrap_or(0)))
    }

    /// List every approval row for id+digest, projected for the critic-gate
    /// evaluator. Critics carry their fingerprint JSON; humans don't.
    pub async fn list_publication_approvals_for_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
    ) -> Result<Vec<PublicationApprovalRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT approver, approver_role, critic_fingerprint_json, \
                        critic_report_uri, approved_at_ms \
                 FROM publication_approvals \
                 WHERE publication_id = ?1 AND content_sha3_256 = ?2 \
                 ORDER BY approved_at_ms ASC",
                (publication_id.to_string(), content_sha3_256.to_string()),
            )
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(PublicationApprovalRow {
                approver: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                approver_role: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                critic_fingerprint_json: r
                    .get(2)
                    .map_err(|e| StoreError::Db(e.to_string()))?,
                critic_report_uri: r
                    .get(3)
                    .map_err(|e| StoreError::Db(e.to_string()))?,
                approved_at_ms: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }
}

/// Approval row projected for callers that need role information.
#[derive(Debug, Clone, PartialEq)]
pub struct PublicationApprovalRow {
    pub approver: String,
    /// `'human'` or `'audited_llm_critic'`.
    pub approver_role: String,
    pub critic_fingerprint_json: Option<String>,
    pub critic_report_uri: Option<String>,
    pub approved_at_ms: i64,
}

impl PublicationApprovalRow {
    /// Convenience: `approver_role == "audited_llm_critic"`.
    pub fn is_critic(&self) -> bool {
        self.approver_role == "audited_llm_critic"
    }
}

impl VoxDb {
    /// Record one publication attempt outcome for a delivery channel.
    pub async fn record_publication_attempt(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
        channel: &str,
        outcome_json: &str,
    ) -> Result<(), StoreError> {
        let publication_id = publication_id.to_string();
        let content_sha3_256 = content_sha3_256.to_string();
        let channel = channel.to_string();
        let outcome_json = outcome_json.to_string();
        let ts = now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO publication_attempts (publication_id, content_sha3_256, channel, attempted_at_ms, outcome_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (
                        publication_id,
                        content_sha3_256,
                        channel,
                        ts,
                        outcome_json,
                    ),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Set the current manifest state and append an immutable status event.
    pub async fn set_publication_state(
        &self,
        publication_id: &str,
        state: &str,
        detail_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let ts = now_ms();
        let publication_id = publication_id.to_string();
        let state = state.to_string();
        let detail_json = detail_json.map(std::string::ToString::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE publication_manifests SET state = ?2, updated_at_ms = ?3 WHERE publication_id = ?1",
                    (
                        publication_id.clone(),
                        state.clone(),
                        ts,
                    ),
                )
                .await?;
                conn.execute(
                    "INSERT INTO publication_status_events (publication_id, status, detail_json, recorded_at_ms) VALUES (?1, ?2, ?3, ?4)",
                    (publication_id, state, detail_json, ts),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Append one `publication_status_events` row without updating `publication_manifests.state`.
    pub async fn append_publication_status_event(
        &self,
        publication_id: &str,
        status: &str,
        detail_json: Option<&str>,
    ) -> Result<(), StoreError> {
        let ts = now_ms();
        let publication_id = publication_id.to_string();
        let status = status.to_string();
        let detail_json = detail_json.map(std::string::ToString::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO publication_status_events (publication_id, status, detail_json, recorded_at_ms) VALUES (?1, ?2, ?3, ?4)",
                    (publication_id, status, detail_json, ts),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}
