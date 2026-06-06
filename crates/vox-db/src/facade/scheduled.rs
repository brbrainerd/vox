//! Phase 4.2: `scheduled_runs` durable state ops powering the
//! `vox_workflow_runtime::scheduled` runner.
//!
//! One row per scheduled function, keyed on `function_name`. The runner polls
//! [`VoxDb::scheduled_runs_due_now`] every second, fires registered callbacks,
//! and uses [`VoxDb::scheduled_runs_mark_started`] /
//! [`VoxDb::scheduled_runs_mark_completed`] to advance `next_due_at_ms`.
//! Process restarts pick up at the persisted `next_due_at_ms` — crash-safe.

use crate::StoreError;
use crate::VoxDb;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// One `scheduled_runs` row as consumed by the runner loop.
#[derive(Debug, Clone)]
pub struct ScheduledRunRow {
    /// Function name (primary key in `scheduled_runs`).
    pub function_name: String,
    /// Configured interval, in milliseconds.
    pub interval_ms: i64,
    /// Unix-ms timestamp of the next-due moment.
    pub next_due_at_ms: i64,
    /// `run_id` of the most-recently-started invocation, if any.
    pub last_run_id: Option<String>,
}

impl VoxDb {
    /// Upsert a scheduled function row.
    ///
    /// - If the row does not exist, inserts with
    ///   `next_due_at_ms = now() + interval_ms`.
    /// - If the row already exists, **preserves** the persisted
    ///   `next_due_at_ms` (don't reset on restart) and only updates
    ///   `interval_ms` if it changed.
    pub async fn upsert_scheduled_run(
        &self,
        name: &str,
        interval_ms: i64,
    ) -> Result<(), StoreError> {
        let _conn = self.conn.clone();
        let name = name.to_string();
        let now = now_ms();
        let next_due = now.saturating_add(interval_ms.max(0));
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO scheduled_runs
                       (function_name, interval_ms, next_due_at_ms, last_run_id, last_started_at_ms, last_completed_at_ms)
                     VALUES (?1, ?2, ?3, NULL, NULL, NULL)
                     ON CONFLICT(function_name) DO UPDATE SET
                       interval_ms = excluded.interval_ms",
                    (name, interval_ms, next_due),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Return the persisted `next_due_at_ms` for one scheduled function, or
    /// `None` if no row exists yet.
    ///
    /// Used by the `vox_workflow_runtime::scheduled` runner's restart-seed
    /// loop (ADR-041 §6(a)): the in-memory `Instant` deadline is derived from
    /// `clamp(persisted_next_due_at_ms - wall_now, 0, interval)` so a crash
    /// 23 hours into a `@scheduled("1d")` interval fires in ~1 hour, not in
    /// a fresh full day. The DB row is the crash-recovery anchor; this read
    /// is how the runner consults it.
    pub async fn scheduled_runs_next_due_at_ms(
        &self,
        name: &str,
    ) -> Result<Option<i64>, StoreError> {
        // Match the pattern used by `scheduled_runs_due_now` below.
        let conn = self.conn.clone();
        let mut cursor = conn
            .query(
                "SELECT next_due_at_ms FROM scheduled_runs WHERE function_name = ?1",
                (name.to_string(),),
            )
            .await?;
        if let Some(row) = cursor.next().await? {
            let value: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Return all scheduled rows whose `next_due_at_ms <= now()`.
    pub async fn scheduled_runs_due_now(&self) -> Result<Vec<ScheduledRunRow>, StoreError> {
        let now = now_ms();
        let conn = self.conn.clone();
        let mut cursor = conn
            .query(
                "SELECT function_name, interval_ms, next_due_at_ms, last_run_id
                 FROM scheduled_runs
                 WHERE next_due_at_ms <= ?1
                 ORDER BY next_due_at_ms ASC",
                (now,),
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = cursor.next().await? {
            let function_name: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let interval_ms: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let next_due_at_ms: i64 = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let last_run_id: Option<String> =
                row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(ScheduledRunRow {
                function_name,
                interval_ms,
                next_due_at_ms,
                last_run_id,
            });
        }
        Ok(out)
    }

    /// Mark a scheduled-function invocation as started: set `last_run_id` and
    /// `last_started_at_ms`.
    pub async fn scheduled_runs_mark_started(
        &self,
        name: &str,
        run_id: &str,
    ) -> Result<(), StoreError> {
        let _conn = self.conn.clone();
        let name = name.to_string();
        let run_id = run_id.to_string();
        let now = now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE scheduled_runs
                     SET last_run_id = ?2,
                         last_started_at_ms = ?3
                     WHERE function_name = ?1",
                    (name, run_id, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Mark a scheduled-function invocation as completed: set
    /// `last_completed_at_ms` and advance `next_due_at_ms` by `interval_ms`.
    ///
    /// `success` is currently captured implicitly via timestamp presence —
    /// future revisions may add a status column. Either way the next interval
    /// is scheduled (failures don't pause the timer).
    pub async fn scheduled_runs_mark_completed(
        &self,
        name: &str,
        run_id: &str,
        _success: bool,
    ) -> Result<(), StoreError> {
        let _conn = self.conn.clone();
        let name = name.to_string();
        let run_id = run_id.to_string();
        let now = now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE scheduled_runs
                     SET last_completed_at_ms = ?3,
                         next_due_at_ms = ?3 + interval_ms
                     WHERE function_name = ?1 AND last_run_id = ?2",
                    (name, run_id, now),
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}
