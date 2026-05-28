//! External review (CodeRabbit/GitHub) persistence helpers.
//!
//! S2 telemetry surface: repository identifiers, code paths, and raw review payload fragments.

use turso::params;

use crate::store::types::{
    ExternalReviewDeadletterParams, ExternalReviewDeadletterRow, ExternalReviewFindingParams,
    ExternalReviewFindingRow, ExternalReviewFindingStateParams, ExternalReviewKpiSnapshotParams,
    ExternalReviewKpiSnapshotRow, ExternalReviewOutcomeParams, ExternalReviewRunParams,
    ExternalReviewRunRow, ExternalReviewThreadParams, StoreError,
};

impl crate::VoxDb {
    /// Insert one external review run and return row id.
    pub async fn insert_external_review_run(
        &self,
        p: ExternalReviewRunParams<'_>,
    ) -> Result<i64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_run
                    (provider, repository_id, owner, repo, pr_number, commit_sha, trigger_kind, idempotency_key, item_count, metadata_json)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        p.provider,
                        p.repository_id,
                        p.owner,
                        p.repo,
                        p.pr_number,
                        p.commit_sha,
                        p.trigger_kind,
                        p.idempotency_key,
                        p.item_count,
                        p.metadata_json,
                    ],
                )
                .await?;
                let mut rows = conn.query("SELECT last_insert_rowid()", ()).await?;
                let id: i64 = rows.next().await?.and_then(|r| r.get(0).ok()).unwrap_or(0);
                Ok::<i64, StoreError>(id)
            })
            .await
    }

    /// Upsert one thread payload record from source comments/reviews.
    pub async fn upsert_external_review_thread(
        &self,
        p: ExternalReviewThreadParams<'_>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_comment_thread
                    (provider, repository_id, pr_number, thread_identity, placement_kind, line_anchor_state, file_path, line_start, line_end, source_comment_id, parent_comment_id, source_payload_hash, raw_payload_json)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    ON CONFLICT(provider, repository_id, pr_number, thread_identity, source_payload_hash) DO UPDATE SET
                      placement_kind = excluded.placement_kind,
                      line_anchor_state = excluded.line_anchor_state,
                      file_path = excluded.file_path,
                      line_start = excluded.line_start,
                      line_end = excluded.line_end,
                      source_comment_id = excluded.source_comment_id,
                      parent_comment_id = excluded.parent_comment_id,
                      raw_payload_json = excluded.raw_payload_json",
                    params![
                        p.provider,
                        p.repository_id,
                        p.pr_number,
                        p.thread_identity,
                        p.placement_kind,
                        p.line_anchor_state,
                        p.file_path,
                        p.line_start,
                        p.line_end,
                        p.source_comment_id,
                        p.parent_comment_id,
                        p.source_payload_hash,
                        p.raw_payload_json,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Upsert one external review finding by stable fingerprint scope.
    pub async fn upsert_external_review_finding(
        &self,
        p: ExternalReviewFindingParams<'_>,
    ) -> Result<i64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_finding
                    (run_id, provider, repository_id, pr_number, finding_identity, thread_identity, source_comment_id, placement_kind, line_anchor_state, file_path, line_start, line_end, category, anti_pattern_id, severity, title, details, suggested_fix, extraction_confidence, source_payload_hash, fingerprint, status)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
                    ON CONFLICT(provider, repository_id, pr_number, fingerprint) DO UPDATE SET
                      run_id = excluded.run_id,
                      finding_identity = excluded.finding_identity,
                      thread_identity = excluded.thread_identity,
                      source_comment_id = excluded.source_comment_id,
                      placement_kind = excluded.placement_kind,
                      line_anchor_state = excluded.line_anchor_state,
                      file_path = excluded.file_path,
                      line_start = excluded.line_start,
                      line_end = excluded.line_end,
                      category = excluded.category,
                      anti_pattern_id = excluded.anti_pattern_id,
                      severity = excluded.severity,
                      title = excluded.title,
                      details = excluded.details,
                      suggested_fix = excluded.suggested_fix,
                      extraction_confidence = excluded.extraction_confidence,
                      source_payload_hash = excluded.source_payload_hash,
                      status = excluded.status",
                    params![
                        p.run_id,
                        p.provider,
                        p.repository_id,
                        p.pr_number,
                        p.finding_identity,
                        p.thread_identity,
                        p.source_comment_id,
                        p.placement_kind,
                        p.line_anchor_state,
                        p.file_path,
                        p.line_start,
                        p.line_end,
                        p.category,
                        p.anti_pattern_id,
                        p.severity,
                        p.title,
                        p.details,
                        p.suggested_fix,
                        p.extraction_confidence,
                        p.source_payload_hash,
                        p.fingerprint,
                        p.status,
                    ],
                )
                .await?;

                let mut rows = conn
                    .query(
                        "SELECT id FROM external_review_finding WHERE provider=?1 AND repository_id=?2 AND pr_number=?3 AND fingerprint=?4 LIMIT 1",
                        params![p.provider, p.repository_id, p.pr_number, p.fingerprint],
                    )
                    .await?;
                let id: i64 = rows.next().await?.and_then(|r| r.get(0).ok()).unwrap_or(0);
                Ok::<i64, StoreError>(id)
            })
            .await
    }

    /// Append a correctness/state transition for a finding.
    pub async fn append_external_review_finding_state(
        &self,
        p: ExternalReviewFindingStateParams<'_>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_finding_state_history
                    (finding_id, previous_state, new_state, reason, confidence, evidence_ref)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        p.finding_id,
                        p.previous_state,
                        p.new_state,
                        p.reason,
                        p.confidence,
                        p.evidence_ref,
                    ],
                )
                .await?;
                conn.execute(
                    "UPDATE external_review_finding SET status = ?2 WHERE id = ?1",
                    params![p.finding_id, p.new_state],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record one outcome event linked to a finding.
    pub async fn insert_external_review_outcome(
        &self,
        p: ExternalReviewOutcomeParams<'_>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_outcome (finding_id, outcome_kind, outcome_ref, outcome_json)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![p.finding_id, p.outcome_kind, p.outcome_ref, p.outcome_json],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Record one parse/normalization dead-letter row.
    pub async fn insert_external_review_deadletter(
        &self,
        p: ExternalReviewDeadletterParams<'_>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_deadletter
                    (provider, repository_id, pr_number, source_kind, source_comment_id, source_payload_hash, error_class, error_message, raw_payload_json)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        p.provider,
                        p.repository_id,
                        p.pr_number,
                        p.source_kind,
                        p.source_comment_id,
                        p.source_payload_hash,
                        p.error_class,
                        p.error_message,
                        p.raw_payload_json,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Retry one dead-letter row by marking it retried.
    pub async fn mark_external_review_deadletter_retried(&self, id: i64) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE external_review_deadletter
                     SET retry_state='retried', retried_at=(strftime('%Y-%m-%dT%H:%M:%SZ','now'))
                     WHERE id=?1",
                    params![id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List pending dead-letter rows for a repository/pr.
    pub async fn list_external_review_deadletters(
        &self,
        repository_id: &str,
        pr_number: i64,
        limit: i64,
    ) -> Result<Vec<ExternalReviewDeadletterRow>, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, provider, repository_id, pr_number, source_kind, source_comment_id, source_payload_hash, error_class, error_message, raw_payload_json, retry_state, created_at, retried_at
                         FROM external_review_deadletter
                         WHERE repository_id=?1 AND pr_number=?2
                         ORDER BY id DESC LIMIT ?3",
                        params![repository_id.as_str(), pr_number, limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(ExternalReviewDeadletterRow {
                        id: row.get(0)?,
                        provider: row.get(1)?,
                        repository_id: row.get(2)?,
                        pr_number: row.get(3)?,
                        source_kind: row.get(4)?,
                        source_comment_id: row.get(5)?,
                        source_payload_hash: row.get(6)?,
                        error_class: row.get(7)?,
                        error_message: row.get(8)?,
                        raw_payload_json: row.get(9)?,
                        retry_state: row.get(10)?,
                        created_at: row.get(11)?,
                        retried_at: row.get(12)?,
                    });
                }
                Ok::<Vec<ExternalReviewDeadletterRow>, StoreError>(out)
            })
            .await
    }

    /// List findings in a repository from `id` descending window (for training exports).
    pub async fn list_external_review_findings_for_training_window(
        &self,
        repository_id: &str,
        limit: i64,
    ) -> Result<Vec<ExternalReviewFindingRow>, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, run_id, provider, repository_id, pr_number, finding_identity, thread_identity, source_comment_id, placement_kind, line_anchor_state, file_path, line_start, line_end, category, anti_pattern_id, severity, title, details, suggested_fix, extraction_confidence, source_payload_hash, fingerprint, status
                         FROM external_review_finding
                         WHERE repository_id=?1
                         ORDER BY id DESC LIMIT ?2",
                        params![repository_id.as_str(), limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(ExternalReviewFindingRow {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        provider: row.get(2)?,
                        repository_id: row.get(3)?,
                        pr_number: row.get(4)?,
                        finding_identity: row.get(5)?,
                        thread_identity: row.get(6)?,
                        source_comment_id: row.get(7)?,
                        placement_kind: row.get(8)?,
                        line_anchor_state: row.get(9)?,
                        file_path: row.get(10)?,
                        line_start: row.get(11)?,
                        line_end: row.get(12)?,
                        category: row.get(13)?,
                        anti_pattern_id: row.get(14)?,
                        severity: row.get(15)?,
                        title: row.get(16)?,
                        details: row.get(17)?,
                        suggested_fix: row.get(18)?,
                        extraction_confidence: row.get(19)?,
                        source_payload_hash: row.get(20)?,
                        fingerprint: row.get(21)?,
                        status: row.get(22)?,
                    });
                }
                Ok::<Vec<ExternalReviewFindingRow>, StoreError>(out)
            })
            .await
    }

    /// Insert a KPI snapshot row.
    pub async fn insert_external_review_kpi_snapshot(
        &self,
        p: ExternalReviewKpiSnapshotParams<'_>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_review_kpi_snapshot
                    (repository_id, period_start, period_end, coverage_ratio, ingest_to_fix_latency_ms, repeated_finding_rate, post_training_regression_rate, auto_fix_acceptance_rate)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        p.repository_id,
                        p.period_start,
                        p.period_end,
                        p.coverage_ratio,
                        p.ingest_to_fix_latency_ms,
                        p.repeated_finding_rate,
                        p.post_training_regression_rate,
                        p.auto_fix_acceptance_rate,
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Return latest KPI snapshots for a repository.
    pub async fn list_external_review_kpi_snapshots(
        &self,
        repository_id: &str,
        limit: i64,
    ) -> Result<Vec<ExternalReviewKpiSnapshotRow>, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, repository_id, period_start, period_end, coverage_ratio, ingest_to_fix_latency_ms, repeated_finding_rate, post_training_regression_rate, auto_fix_acceptance_rate, created_at
                         FROM external_review_kpi_snapshot
                         WHERE repository_id=?1
                         ORDER BY id DESC LIMIT ?2",
                        params![repository_id.as_str(), limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(ExternalReviewKpiSnapshotRow {
                        id: row.get(0)?,
                        repository_id: row.get(1)?,
                        period_start: row.get(2)?,
                        period_end: row.get(3)?,
                        coverage_ratio: row.get(4)?,
                        ingest_to_fix_latency_ms: row.get(5)?,
                        repeated_finding_rate: row.get(6)?,
                        post_training_regression_rate: row.get(7)?,
                        auto_fix_acceptance_rate: row.get(8)?,
                        created_at: row.get(9)?,
                    });
                }
                Ok::<Vec<ExternalReviewKpiSnapshotRow>, StoreError>(out)
            })
            .await
    }

    /// Fetch latest run for one repository/pr pair.
    pub async fn latest_external_review_run(
        &self,
        repository_id: &str,
        pr_number: i64,
    ) -> Result<Option<ExternalReviewRunRow>, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, provider, repository_id, owner, repo, pr_number, commit_sha, trigger_kind, idempotency_key, item_count, started_at, finished_at, metadata_json
                         FROM external_review_run
                         WHERE repository_id=?1 AND pr_number=?2
                         ORDER BY id DESC LIMIT 1",
                        params![repository_id.as_str(), pr_number],
                    )
                    .await?;
                let row = match rows.next().await? {
                    Some(row) => Some(ExternalReviewRunRow {
                        id: row.get(0)?,
                        provider: row.get(1)?,
                        repository_id: row.get(2)?,
                        owner: row.get(3)?,
                        repo: row.get(4)?,
                        pr_number: row.get(5)?,
                        commit_sha: row.get(6)?,
                        trigger_kind: row.get(7)?,
                        idempotency_key: row.get(8)?,
                        item_count: row.get(9)?,
                        started_at: row.get(10)?,
                        finished_at: row.get(11)?,
                        metadata_json: row.get(12)?,
                    }),
                    None => None,
                };
                Ok::<Option<ExternalReviewRunRow>, StoreError>(row)
            })
            .await
    }

    /// Count `external_review_run` rows for one `repository_id`.
    pub async fn count_external_review_runs_for_repository(
        &self,
        repository_id: &str,
    ) -> Result<i64, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT COUNT(*) FROM external_review_run WHERE repository_id=?1",
                        params![repository_id.as_str()],
                    )
                    .await?;
                let n: i64 = rows.next().await?.and_then(|r| r.get(0).ok()).unwrap_or(0);
                Ok::<i64, StoreError>(n)
            })
            .await
    }

    /// Count `external_review_finding` rows for one `repository_id`.
    pub async fn count_external_review_findings_for_repository(
        &self,
        repository_id: &str,
    ) -> Result<i64, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT COUNT(*) FROM external_review_finding WHERE repository_id=?1",
                        params![repository_id.as_str()],
                    )
                    .await?;
                let n: i64 = rows.next().await?.and_then(|r| r.get(0).ok()).unwrap_or(0);
                Ok::<i64, StoreError>(n)
            })
            .await
    }

    /// Count pending `external_review_deadletter` rows for one `repository_id`.
    pub async fn count_external_review_deadletters_pending_for_repository(
        &self,
        repository_id: &str,
    ) -> Result<i64, StoreError> {
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT COUNT(*) FROM external_review_deadletter WHERE repository_id=?1 AND retry_state='pending'",
                        params![repository_id.as_str()],
                    )
                    .await?;
                let n: i64 = rows
                    .next()
                    .await?
                    .and_then(|r| r.get(0).ok())
                    .unwrap_or(0);
                Ok::<i64, StoreError>(n)
            })
            .await
    }
}
