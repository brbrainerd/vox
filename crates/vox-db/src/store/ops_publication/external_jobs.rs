use super::common::now_ms;
use crate::{VoxDb, store::StoreError, store::types::*};

impl VoxDb {
    pub async fn upsert_external_submission_job(
        &self,
        p: ExternalSubmissionJobUpsertParams<'_>,
    ) -> Result<i64, StoreError> {
        if p.attempt_count < 0 {
            return Err(StoreError::Db(
                "external_submission_jobs.attempt_count must be >= 0".into(),
            ));
        }
        if let Some(existing) = self
            .get_external_submission_job_by_idempotency_key(p.idempotency_key)
            .await?
            && (existing.publication_id != p.publication_id
                || existing.content_sha3_256 != p.content_sha3_256
                || existing.adapter != p.adapter
                || existing.operation != p.operation)
        {
            return Err(StoreError::UpsertIdentityMismatch(format!(
                "external_submission_jobs.idempotency_key={} already maps to \
                     publication_id={} digest={} adapter={} operation={}; \
                     upsert refused mismatched identity",
                p.idempotency_key,
                existing.publication_id,
                existing.content_sha3_256,
                existing.adapter,
                existing.operation,
            )));
        }
        let ts = now_ms();
        let publication_id = p.publication_id.to_string();
        let content_sha3_256 = p.content_sha3_256.to_string();
        let adapter = p.adapter.to_string();
        let operation = p.operation.to_string();
        let idempotency_key = p.idempotency_key.to_string();
        let idempotency_key_for_row = idempotency_key.clone();
        let status = p.status.to_string();
        let lock_owner = p.lock_owner.map(std::string::ToString::to_string);
        let lock_expires_at_ms = p.lock_expires_at_ms;
        let next_retry_at_ms = p.next_retry_at_ms;
        let attempt_count = p.attempt_count;
        let last_error_class = p.last_error_class.map(std::string::ToString::to_string);
        let last_error_message = p.last_error_message.map(std::string::ToString::to_string);
        let metadata_json = p.metadata_json.map(std::string::ToString::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                "INSERT INTO external_submission_jobs (
                    publication_id, content_sha3_256, adapter, operation, idempotency_key,
                    status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                    last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
                ON CONFLICT(idempotency_key) DO UPDATE SET
                    status = excluded.status,
                    lock_owner = excluded.lock_owner,
                    lock_expires_at_ms = excluded.lock_expires_at_ms,
                    next_retry_at_ms = excluded.next_retry_at_ms,
                    last_error_class = excluded.last_error_class,
                    last_error_message = excluded.last_error_message,
                    metadata_json = excluded.metadata_json,
                    updated_at_ms = excluded.updated_at_ms",
                (
                    publication_id,
                    content_sha3_256,
                    adapter,
                    operation,
                    idempotency_key.clone(),
                    status,
                    lock_owner,
                    lock_expires_at_ms,
                    next_retry_at_ms,
                    attempt_count,
                    last_error_class,
                    last_error_message,
                    metadata_json,
                    ts,
                ),
            )
            .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        let rows = self
            .query_all(
                "SELECT id FROM external_submission_jobs WHERE idempotency_key = ?1",
                (idempotency_key_for_row,),
            )
            .await?;
        let row = rows.first().ok_or_else(|| {
            StoreError::Db("external_submission_jobs id lookup missing row".into())
        })?;
        row.get(0).map_err(|e| StoreError::Db(e.to_string()))
    }

    /// Fetch one external submission job by idempotency key.
    pub async fn get_external_submission_job_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<ExternalSubmissionJobRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT id, publication_id, content_sha3_256, adapter, operation, idempotency_key,
                        status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                        last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                 FROM external_submission_jobs WHERE idempotency_key = ?1",
                (idempotency_key.to_string(),),
            )
            .await?;
        let Some(r) = rows.first() else {
            return Ok(None);
        };
        Ok(Some(ExternalSubmissionJobRow {
            id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            publication_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
            content_sha3_256: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            adapter: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            operation: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            idempotency_key: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            status: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            lock_owner: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            lock_expires_at_ms: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
            next_retry_at_ms: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
            attempt_count: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
            last_error_class: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
            last_error_message: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
            metadata_json: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
            created_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
            updated_at_ms: r.get(15).map_err(|e| StoreError::Db(e.to_string()))?,
        }))
    }

    /// Fetch one external submission job by primary key.
    pub async fn get_external_submission_job_by_id(
        &self,
        job_id: i64,
    ) -> Result<Option<ExternalSubmissionJobRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT id, publication_id, content_sha3_256, adapter, operation, idempotency_key,
                        status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                        last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                 FROM external_submission_jobs WHERE id = ?1",
                (job_id,),
            )
            .await?;
        let Some(r) = rows.first() else {
            return Ok(None);
        };
        Ok(Some(ExternalSubmissionJobRow {
            id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            publication_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
            content_sha3_256: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
            adapter: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            operation: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
            idempotency_key: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            status: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            lock_owner: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
            lock_expires_at_ms: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
            next_retry_at_ms: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
            attempt_count: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
            last_error_class: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
            last_error_message: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
            metadata_json: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
            created_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
            updated_at_ms: r.get(15).map_err(|e| StoreError::Db(e.to_string()))?,
        }))
    }

    /// Jobs ready for worker processing: due `queued` / `retryable_failed`, or stale `running` with expired lock.
    pub async fn list_external_submission_jobs_due(
        &self,
        before_ms_inclusive: i64,
        limit: i64,
    ) -> Result<Vec<ExternalSubmissionJobRow>, StoreError> {
        let lim = limit.clamp(1, 500);
        let rows = self
            .query_all(
                "SELECT id, publication_id, content_sha3_256, adapter, operation, idempotency_key,
                        status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                        last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                 FROM external_submission_jobs
                 WHERE (
                    (status IN ('queued', 'retryable_failed')
                     AND (next_retry_at_ms IS NULL OR next_retry_at_ms <= ?1))
                    OR (status = 'running'
                        AND lock_expires_at_ms IS NOT NULL
                        AND lock_expires_at_ms < ?1)
                 )
                 ORDER BY COALESCE(next_retry_at_ms, 0) ASC, id ASC
                 LIMIT ?2",
                (before_ms_inclusive, lim),
            )
            .await?;
        rows.into_iter()
            .map(|r| {
                Ok(ExternalSubmissionJobRow {
                    id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    publication_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    content_sha3_256: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                    adapter: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                    operation: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                    idempotency_key: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                    status: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_owner: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_expires_at_ms: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                    next_retry_at_ms: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
                    attempt_count: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_class: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_message: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
                    metadata_json: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
                    created_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
                    updated_at_ms: r.get(15).map_err(|e| StoreError::Db(e.to_string()))?,
                })
            })
            .collect()
    }

    /// Permanently failed scholarly outbound jobs (`status = failed`), newest first (dead-letter review).
    pub async fn list_external_submission_jobs_failed(
        &self,
        limit: i64,
    ) -> Result<Vec<ExternalSubmissionJobRow>, StoreError> {
        let lim = limit.clamp(1, 500);
        let rows = self
            .query_all(
                "SELECT id, publication_id, content_sha3_256, adapter, operation, idempotency_key,
                        status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                        last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                 FROM external_submission_jobs
                 WHERE status = 'failed'
                 ORDER BY updated_at_ms DESC, id DESC
                 LIMIT ?1",
                (lim,),
            )
            .await?;
        rows.into_iter()
            .map(|r| {
                Ok(ExternalSubmissionJobRow {
                    id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    publication_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    content_sha3_256: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                    adapter: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                    operation: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                    idempotency_key: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                    status: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_owner: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_expires_at_ms: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                    next_retry_at_ms: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
                    attempt_count: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_class: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_message: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
                    metadata_json: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
                    created_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
                    updated_at_ms: r.get(15).map_err(|e| StoreError::Db(e.to_string()))?,
                })
            })
            .collect()
    }

    /// Operator replay: move one **terminal** `failed` job back to `queued` (clears lease, retry schedule, last error).
    ///
    /// Fails if the row is missing or `status` is not `failed`. `attempt_count` is preserved for audit.
    pub async fn replay_failed_external_submission_job_to_queued(
        &self,
        job_id: i64,
    ) -> Result<ExternalSubmissionJobRow, StoreError> {
        let ts = now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let n = breaker
            .call(|| async move {
                let n = conn
                    .execute(
                        "UPDATE external_submission_jobs SET
                    status = 'queued',
                    lock_owner = NULL,
                    lock_expires_at_ms = NULL,
                    next_retry_at_ms = NULL,
                    last_error_class = NULL,
                    last_error_message = NULL,
                    updated_at_ms = ?1
                 WHERE id = ?2 AND status = 'failed'",
                        (ts, job_id),
                    )
                    .await?;
                Ok::<u64, StoreError>(n)
            })
            .await?;
        if n == 0 {
            let Some(j) = self.get_external_submission_job_by_id(job_id).await? else {
                return Err(StoreError::NotFound(format!(
                    "external_submission_jobs id={job_id}"
                )));
            };
            return Err(StoreError::Db(format!(
                "replay only allowed when status is 'failed' (job {job_id} is {})",
                j.status
            )));
        }
        self.get_external_submission_job_by_id(job_id)
            .await?
            .ok_or_else(|| {
                StoreError::Db(format!(
                    "external_submission_jobs id={job_id} missing after replay update"
                ))
            })
    }

    /// Try to move a due job to `running` with a lease. Returns `true` if this worker won the race.
    pub async fn try_claim_external_submission_job(
        &self,
        job_id: i64,
        lock_owner: &str,
        lock_expires_at_ms: i64,
        now_ms_inclusive: i64,
    ) -> Result<bool, StoreError> {
        let ts = now_ms();
        let lock_owner = lock_owner.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let n = breaker
            .call(|| async move {
                let n = conn
                    .execute(
                        "UPDATE external_submission_jobs SET
                    status = 'running',
                    lock_owner = ?1,
                    lock_expires_at_ms = ?2,
                    updated_at_ms = ?3
                 WHERE id = ?4
                   AND (
                     (status IN ('queued', 'retryable_failed')
                      AND (next_retry_at_ms IS NULL OR next_retry_at_ms <= ?5))
                     OR (status = 'running'
                         AND lock_expires_at_ms IS NOT NULL
                         AND lock_expires_at_ms < ?5)
                   )",
                        (lock_owner, lock_expires_at_ms, ts, job_id, now_ms_inclusive),
                    )
                    .await?;
                Ok::<u64, StoreError>(n)
            })
            .await?;
        Ok(n > 0)
    }

    /// List external submission jobs for one publication id and content digest.
    pub async fn list_external_submission_jobs_for_publication_digest(
        &self,
        publication_id: &str,
        content_sha3_256: &str,
    ) -> Result<Vec<ExternalSubmissionJobRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT id, publication_id, content_sha3_256, adapter, operation, idempotency_key,
                        status, lock_owner, lock_expires_at_ms, next_retry_at_ms, attempt_count,
                        last_error_class, last_error_message, metadata_json, created_at_ms, updated_at_ms
                 FROM external_submission_jobs
                 WHERE publication_id = ?1 AND content_sha3_256 = ?2
                 ORDER BY id DESC",
                (
                    publication_id.to_string(),
                    content_sha3_256.to_string(),
                ),
            )
            .await?;
        rows.into_iter()
            .map(|r| {
                Ok(ExternalSubmissionJobRow {
                    id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    publication_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    content_sha3_256: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                    adapter: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                    operation: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                    idempotency_key: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                    status: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_owner: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                    lock_expires_at_ms: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                    next_retry_at_ms: r.get(9).map_err(|e| StoreError::Db(e.to_string()))?,
                    attempt_count: r.get(10).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_class: r.get(11).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_error_message: r.get(12).map_err(|e| StoreError::Db(e.to_string()))?,
                    metadata_json: r.get(13).map_err(|e| StoreError::Db(e.to_string()))?,
                    created_at_ms: r.get(14).map_err(|e| StoreError::Db(e.to_string()))?,
                    updated_at_ms: r.get(15).map_err(|e| StoreError::Db(e.to_string()))?,
                })
            })
            .collect()
    }

    /// Append one adapter HTTP attempt row and bump `attempt_count` on the parent job.
    pub async fn record_external_submission_attempt(
        &self,
        p: ExternalSubmissionAttemptParams<'_>,
    ) -> Result<(), StoreError> {
        let ts = now_ms();
        let retryable_i: i64 = if p.retryable { 1 } else { 0 };
        let job_id = p.job_id;
        let http_status = p.http_status.map(i64::from);
        let error_class = p.error_class.map(std::string::ToString::to_string);
        let request_fingerprint = p.request_fingerprint.map(std::string::ToString::to_string);
        let response_fingerprint = p.response_fingerprint.map(std::string::ToString::to_string);
        let detail_json = p.detail_json.map(std::string::ToString::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO external_submission_attempts (
                    job_id, attempted_at_ms, http_status, error_class, retryable,
                    request_fingerprint, response_fingerprint, detail_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    (
                        job_id,
                        ts,
                        http_status,
                        error_class,
                        retryable_i,
                        request_fingerprint,
                        response_fingerprint,
                        detail_json,
                    ),
                )
                .await?;
                conn.execute(
                    "UPDATE external_submission_jobs SET attempt_count = attempt_count + 1, updated_at_ms = ?2 WHERE id = ?1",
                    (job_id, ts),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List attempts for a job, oldest first.
    pub async fn list_external_submission_attempts_for_job(
        &self,
        job_id: i64,
    ) -> Result<Vec<ExternalSubmissionAttemptRow>, StoreError> {
        let rows = self
            .query_all(
                "SELECT id, job_id, attempted_at_ms, http_status, error_class, retryable,
                        request_fingerprint, response_fingerprint, detail_json
                 FROM external_submission_attempts WHERE job_id = ?1 ORDER BY id ASC",
                (job_id,),
            )
            .await?;
        rows.into_iter()
            .map(|r| {
                let retry_i: i64 = r.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok(ExternalSubmissionAttemptRow {
                    id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    job_id: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    attempted_at_ms: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                    http_status: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                    error_class: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                    retryable: retry_i != 0,
                    request_fingerprint: r.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                    response_fingerprint: r.get(7).map_err(|e| StoreError::Db(e.to_string()))?,
                    detail_json: r.get(8).map_err(|e| StoreError::Db(e.to_string()))?,
                })
            })
            .collect()
    }
}
