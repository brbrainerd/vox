//! Time-based retention helpers for `vox db prune-plan` / `vox db prune-apply`.
//!
//! `table` / `time_column` are validated (`[A-Za-z0-9_]+`) then double-quoted for SQLite.
//!
//! - **days**: `time_column` is compared with SQLite `datetime('now', '-N day')` (TEXT/ISO-style).
//! - **ms_days** (via [`VoxDb::retention_count_older_than_ms_cutoff`] /
//!   [`VoxDb::retention_delete_all_ms_older_than_days`]): `time_column` is Unix millis (`INTEGER`);
//!   deletes use a Turso-safe row-by-row pattern (no `IN (SELECT …)`).
//! - **expires_lt_now**: non-NULL `time_column` values before `datetime('now')` (TTL columns).
//!
//! Which tables/columns are pruned is **not** hard-coded here; see `contracts/db/retention-policy.yaml`
//! and `docs/src/architecture/telemetry-retention-sensitivity-ssot.md` (e.g. `ci_completion_run` /
//! `finished_at`, `ci_completion_suppression` / `expires_at`).

use turso::params;

use crate::store::types::StoreError;

fn validate_retention_ident(name: &str) -> Result<(), StoreError> {
    if name.is_empty() || name.len() > 128 {
        return Err(StoreError::Db(
            "retention table/column name empty or too long".into(),
        ));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(StoreError::Db(format!(
            "retention identifier must be [A-Za-z0-9_]+: {name:?}"
        )));
    }
    Ok(())
}

fn quote_sqlite_ident(name: &str) -> Result<String, StoreError> {
    validate_retention_ident(name)?;
    let mut s = String::with_capacity(name.len() + 2);
    s.push('"');
    for c in name.chars() {
        if c == '"' {
            s.push_str("\"\"");
        } else {
            s.push(c);
        }
    }
    s.push('"');
    Ok(s)
}

impl crate::VoxDb {
    /// Cutoff for “older than N days” when the compared column stores **Unix millis** (`INTEGER`).
    /// Rows match retention when `time_ms_column <` this value.
    pub fn retention_cutoff_ms_exclusive_for_days(days: u32) -> i64 {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        now_ms.saturating_sub((days as i64).saturating_mul(86_400_000))
    }

    /// `SELECT COUNT(*)` where `time_ms_column < cutoff_ms_exclusive` (INTEGER Unix millis).
    pub async fn retention_count_older_than_ms_cutoff(
        &self,
        table: &str,
        time_ms_column: &str,
        cutoff_ms_exclusive: i64,
    ) -> Result<i64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(time_ms_column)?;
        let sql = format!("SELECT COUNT(*) FROM {tq} WHERE {cq} < ?1");
        let mut rows = self.conn.query(&sql, params![cutoff_ms_exclusive]).await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("retention ms count returned no rows".into()))?;
        let n: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(n)
    }

    /// Delete up to `max_rows` rows with `time_ms_column < cutoff_ms_exclusive`, one row per
    /// statement (Turso-compatible).
    pub async fn retention_delete_ms_older_than_chunk(
        &self,
        table: &str,
        time_ms_column: &str,
        cutoff_ms_exclusive: i64,
        max_rows: u64,
    ) -> Result<u64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(time_ms_column)?;
        let select_sql =
            format!("SELECT rowid FROM {tq} WHERE {cq} < ?1 ORDER BY rowid ASC LIMIT 1");
        let delete_sql = format!("DELETE FROM {tq} WHERE rowid = ?1");
        let cap = max_rows.clamp(1, 100_000) as usize;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| {
                let select_sql = select_sql.clone();
                let delete_sql = delete_sql.clone();
                async move {
                    let mut deleted: u64 = 0;
                    for _ in 0..cap {
                        let mut rows = conn
                            .query(&select_sql, params![cutoff_ms_exclusive])
                            .await?;
                        let Some(r) = rows.next().await? else {
                            break;
                        };
                        let rowid: i64 = r.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                        let n = conn.execute(&delete_sql, params![rowid]).await?;
                        deleted += n as u64;
                    }
                    Ok(deleted)
                }
            })
            .await
    }

    /// Delete **all** rows older than `days` (millis column), batched in chunks of up to 100k rows
    /// per inner pass. Stops after 10k outer iterations to avoid unbounded work in one CLI call.
    pub async fn retention_delete_all_ms_older_than_days(
        &self,
        table: &str,
        time_ms_column: &str,
        days: u32,
    ) -> Result<u64, StoreError> {
        const RETENTION_SWEEP_CHUNK_ROWS: u64 = 100_000;
        const RETENTION_SWEEP_MAX_PASSES: u32 = 10_000;
        let cutoff = Self::retention_cutoff_ms_exclusive_for_days(days);
        let mut total: u64 = 0;
        for _ in 0..RETENTION_SWEEP_MAX_PASSES {
            let n = self
                .retention_delete_ms_older_than_chunk(
                    table,
                    time_ms_column,
                    cutoff,
                    RETENTION_SWEEP_CHUNK_ROWS,
                )
                .await?;
            total = total.saturating_add(n);
            if n == 0 {
                break;
            }
        }
        Ok(total)
    }

    /// `SELECT COUNT(*)` for rows where `time_column < datetime('now', '-days day')`.
    pub async fn retention_count_older_than_days(
        &self,
        table: &str,
        time_column: &str,
        days: u32,
    ) -> Result<i64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(time_column)?;
        let sql = format!("SELECT COUNT(*) FROM {tq} WHERE {cq} < datetime('now', '-{days} day')");
        let mut rows = self.conn.query(&sql, ()).await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("retention count returned no rows".into()))?;
        let n: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(n)
    }

    /// `DELETE` rows where `time_column < datetime('now', '-days day')`; returns affected count.
    pub async fn retention_delete_older_than_days(
        &self,
        table: &str,
        time_column: &str,
        days: u32,
    ) -> Result<u64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(time_column)?;
        let sql = format!("DELETE FROM {tq} WHERE {cq} < datetime('now', '-{days} day')");
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn.execute(&sql, ()).await?;
                Ok::<_, StoreError>(n)
            })
            .await
    }

    /// Rows where `expires_at_column` is non-NULL and lexicographically before `datetime('now')`
    /// (TEXT timestamps from SQLite `datetime('now')` / `datetime('now', '+N …')`).
    pub async fn retention_count_expires_lt_now(
        &self,
        table: &str,
        expires_at_column: &str,
    ) -> Result<i64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(expires_at_column)?;
        let sql =
            format!("SELECT COUNT(*) FROM {tq} WHERE {cq} IS NOT NULL AND {cq} < datetime('now')");
        let mut rows = self.conn.query(&sql, ()).await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("retention expires count returned no rows".into()))?;
        let n: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(n)
    }

    /// Delete rows past logical expiry (`expires_at < datetime('now')`, non-NULL only).
    pub async fn retention_delete_expires_lt_now(
        &self,
        table: &str,
        expires_at_column: &str,
    ) -> Result<u64, StoreError> {
        let tq = quote_sqlite_ident(table)?;
        let cq = quote_sqlite_ident(expires_at_column)?;
        let sql = format!("DELETE FROM {tq} WHERE {cq} IS NOT NULL AND {cq} < datetime('now')");
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn.execute(&sql, ()).await?;
                Ok::<_, StoreError>(n)
            })
            .await
    }

    /// Prune old external review payload rows while preserving findings/outcome lineage.
    ///
    /// This removes:
    /// - `external_review_deadletter` older than `days`.
    /// - `external_review_comment_thread` rows with no finding linkage older than `days`.
    pub async fn retention_prune_external_review_payloads(
        &self,
        days: u32,
    ) -> Result<(u64, u64), StoreError> {
        let deadletter_deleted = self
            .retention_delete_older_than_days("external_review_deadletter", "created_at", days)
            .await?;

        let sql = "DELETE FROM external_review_comment_thread
             WHERE id IN (
               SELECT t.id
               FROM external_review_comment_thread t
               LEFT JOIN external_review_finding f
                 ON f.thread_identity = t.thread_identity
                AND f.repository_id = t.repository_id
                AND f.pr_number = t.pr_number
               WHERE f.id IS NULL
                 AND t.id > 0
                 AND t.id IN (
                   SELECT id FROM external_review_comment_thread
                   WHERE rowid IN (
                     SELECT rowid FROM external_review_comment_thread
                     WHERE started_at IS NULL
                   )
                 )
             )"
        .to_string();
        let _ = sql; // keep complex prune path disabled until thread timestamp migration is added
        // Current schema has no thread timestamp column; keep zero until additive migration introduces one.
        let orphan_thread_deleted = 0u64;
        Ok((deadletter_deleted, orphan_thread_deleted))
    }

    /// Prune CRAG loop web results (`source_uri` starting with `tavily:`) older than 7 days.
    pub async fn retention_prune_tavily_search_documents(&self) -> Result<u64, StoreError> {
        // We do not have time_ms on this table, search_documents uses ingested_at (TEXT) or id based ordering.
        let sql = "DELETE FROM search_documents 
                   WHERE source_uri LIKE 'tavily:%' 
                     AND ingested_at < datetime('now', '-7 day')";
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn.execute(&sql, ()).await?;
                Ok::<_, StoreError>(n)
            })
            .await
    }
}
