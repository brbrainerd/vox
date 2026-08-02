//! `VoxDb` methods for `vox harness eval --live` persistence. See
//! `crates/vox-db/src/schema/domains/harness_eval.rs` for the schema and
//! `crates/vox-db-types/src/store_types/harness_eval.rs` for the record types.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

pub use vox_db_types::store_types::harness_eval::*;

impl VoxDb {
    pub async fn record_harness_eval_run(
        &self,
        rec: &HarnessEvalRunRecord,
    ) -> Result<i64, StoreError> {
        let changed_files_json = serde_json::to_string(&rec.changed_files)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO harness_eval_run (
                        run_id, triggered_by, git_sha, git_branch, changed_files_json,
                        config_version, samples_per_task, task_count, pass_count, fail_count,
                        skip_count, total_cost_usd, started_at_ms, finished_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        rec.run_id,
                        rec.triggered_by,
                        rec.git_sha,
                        rec.git_branch,
                        changed_files_json,
                        rec.config_version,
                        rec.samples_per_task,
                        rec.task_count,
                        rec.pass_count,
                        rec.fail_count,
                        rec.skip_count,
                        rec.total_cost_usd,
                        rec.started_at_ms,
                        rec.finished_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn record_harness_eval_task_result(
        &self,
        rec: &HarnessEvalTaskResultRecord,
    ) -> Result<i64, StoreError> {
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO harness_eval_task_result (
                        run_id, task_id, category, checker_kind, status, pass_samples,
                        total_samples, latency_p50_ms, cost_usd, failure_detail, recorded_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        rec.run_id,
                        rec.task_id,
                        rec.category,
                        rec.checker_kind,
                        rec.status,
                        rec.pass_samples,
                        rec.total_samples,
                        rec.latency_p50_ms,
                        rec.cost_usd,
                        rec.failure_detail,
                        rec.recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn record_model_selection_event(
        &self,
        rec: &ModelSelectionEventRecord,
    ) -> Result<i64, StoreError> {
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO model_selection_event (
                        run_id, task_id, model_id, cost_tier, selection_reason,
                        was_privacy_gated, recorded_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        rec.run_id,
                        rec.task_id,
                        rec.model_id,
                        rec.cost_tier,
                        rec.selection_reason,
                        rec.was_privacy_gated as i64,
                        rec.recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn list_harness_eval_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<HarnessEvalRunRecord>, StoreError> {
        let lim = limit.max(1) as i64;
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, triggered_by, git_sha, git_branch, changed_files_json,
                        config_version, samples_per_task, task_count, pass_count, fail_count,
                        skip_count, total_cost_usd, started_at_ms, finished_at_ms
                 FROM harness_eval_run
                 ORDER BY started_at_ms DESC
                 LIMIT ?1",
                params![lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let changed_files_json: Option<String> = row.get(4)?;
            let changed_files = changed_files_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            out.push(HarnessEvalRunRecord {
                run_id: row.get(0)?,
                triggered_by: row.get(1)?,
                git_sha: row.get(2)?,
                git_branch: row.get(3)?,
                changed_files,
                config_version: row.get(5)?,
                samples_per_task: row.get(6)?,
                task_count: row.get(7)?,
                pass_count: row.get(8)?,
                fail_count: row.get(9)?,
                skip_count: row.get(10)?,
                total_cost_usd: row.get(11)?,
                started_at_ms: row.get(12)?,
                finished_at_ms: row.get(13)?,
            });
        }
        Ok(out)
    }

    pub async fn get_harness_eval_task_results(
        &self,
        run_id: &str,
    ) -> Result<Vec<HarnessEvalTaskResultRecord>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, task_id, category, checker_kind, status, pass_samples,
                        total_samples, latency_p50_ms, cost_usd, failure_detail, recorded_at_ms
                 FROM harness_eval_task_result
                 WHERE run_id = ?1
                 ORDER BY id ASC",
                params![run_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(HarnessEvalTaskResultRecord {
                run_id: row.get(0)?,
                task_id: row.get(1)?,
                category: row.get(2)?,
                checker_kind: row.get(3)?,
                status: row.get(4)?,
                pass_samples: row.get(5)?,
                total_samples: row.get(6)?,
                latency_p50_ms: row.get(7)?,
                cost_usd: row.get(8)?,
                failure_detail: row.get(9)?,
                recorded_at_ms: row.get(10)?,
            });
        }
        Ok(out)
    }

    /// Batched sibling of [`get_harness_eval_task_results`](Self::get_harness_eval_task_results)
    /// for callers (e.g. the GUI's `harness_eval_history` command) that need task results for
    /// many runs at once — issues a single `WHERE run_id IN (...)` query instead of one round
    /// trip per run. Runs with no task results are simply absent from the returned map (never an
    /// empty-vec entry), so callers should use `.get(run_id).map_or(&[], Vec::as_slice)` or
    /// similar rather than indexing.
    pub async fn get_harness_eval_task_results_for_runs(
        &self,
        run_ids: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<HarnessEvalTaskResultRecord>>, StoreError>
    {
        if run_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = (0..run_ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT run_id, task_id, category, checker_kind, status, pass_samples,
                    total_samples, latency_p50_ms, cost_usd, failure_detail, recorded_at_ms
             FROM harness_eval_task_result
             WHERE run_id IN ({placeholders})
             ORDER BY run_id ASC, id ASC"
        );
        let bound: Vec<turso::Value> = run_ids
            .iter()
            .map(|id| turso::Value::from(id.as_str()))
            .collect();
        let mut rows = self.connection().query(&sql, bound).await?;
        let mut out: std::collections::HashMap<String, Vec<HarnessEvalTaskResultRecord>> =
            std::collections::HashMap::new();
        while let Some(row) = rows.next().await? {
            let rec = HarnessEvalTaskResultRecord {
                run_id: row.get(0)?,
                task_id: row.get(1)?,
                category: row.get(2)?,
                checker_kind: row.get(3)?,
                status: row.get(4)?,
                pass_samples: row.get(5)?,
                total_samples: row.get(6)?,
                latency_p50_ms: row.get(7)?,
                cost_usd: row.get(8)?,
                failure_detail: row.get(9)?,
                recorded_at_ms: row.get(10)?,
            };
            out.entry(rec.run_id.clone()).or_default().push(rec);
        }
        Ok(out)
    }

    pub async fn get_model_selection_events(
        &self,
        run_id: &str,
    ) -> Result<Vec<ModelSelectionEventRecord>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, task_id, model_id, cost_tier, selection_reason,
                        was_privacy_gated, recorded_at_ms
                 FROM model_selection_event
                 WHERE run_id = ?1
                 ORDER BY id ASC",
                params![run_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let was_privacy_gated: i64 = row.get(5)?;
            out.push(ModelSelectionEventRecord {
                run_id: row.get(0)?,
                task_id: row.get(1)?,
                model_id: row.get(2)?,
                cost_tier: row.get(3)?,
                selection_reason: row.get(4)?,
                was_privacy_gated: was_privacy_gated != 0,
                recorded_at_ms: row.get(6)?,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbConfig, VoxDb};

    /// `harness_eval_history` needs task results for up to 50 runs per call;
    /// this exercises the batched `WHERE run_id IN (...)` fetch that replaces
    /// an N-round-trip loop, confirming it groups per-run results correctly
    /// (including a run with zero task results, which must still round-trip
    /// as "absent from the map" rather than erroring).
    #[tokio::test]
    async fn get_harness_eval_task_results_for_runs_batches_into_one_query() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        for (run_id, task_id) in [("r1", "task-a"), ("r2", "task-b")] {
            db.record_harness_eval_run(&HarnessEvalRunRecord {
                run_id: run_id.to_string(),
                triggered_by: "local".to_string(),
                git_sha: "abc1234".to_string(),
                git_branch: "main".to_string(),
                changed_files: vec![],
                config_version: None,
                samples_per_task: 1,
                task_count: 1,
                pass_count: 1,
                fail_count: 0,
                skip_count: 0,
                total_cost_usd: 0.0,
                started_at_ms: 1000,
                finished_at_ms: 2000,
            })
            .await
            .expect("record run");
            db.record_harness_eval_task_result(&HarnessEvalTaskResultRecord {
                run_id: run_id.to_string(),
                task_id: task_id.to_string(),
                category: "chat".to_string(),
                checker_kind: "deterministic".to_string(),
                status: "pass".to_string(),
                pass_samples: 1,
                total_samples: 1,
                latency_p50_ms: None,
                cost_usd: None,
                failure_detail: None,
                recorded_at_ms: 1500,
            })
            .await
            .expect("record task result");
        }
        // r3 has a run but no task results — must not appear as an empty-vec
        // entry (or a spurious error) in the batched map.
        db.record_harness_eval_run(&HarnessEvalRunRecord {
            run_id: "r3".to_string(),
            triggered_by: "local".to_string(),
            git_sha: "abc1234".to_string(),
            git_branch: "main".to_string(),
            changed_files: vec![],
            config_version: None,
            samples_per_task: 1,
            task_count: 0,
            pass_count: 0,
            fail_count: 0,
            skip_count: 0,
            total_cost_usd: 0.0,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        })
        .await
        .expect("record run");

        let run_ids = vec!["r1".to_string(), "r2".to_string(), "r3".to_string()];
        let by_run = db
            .get_harness_eval_task_results_for_runs(&run_ids)
            .await
            .expect("batched fetch");

        assert_eq!(
            by_run.len(),
            2,
            "only runs with task results should have entries"
        );
        assert_eq!(by_run["r1"].len(), 1);
        assert_eq!(by_run["r1"][0].task_id, "task-a");
        assert_eq!(by_run["r2"].len(), 1);
        assert_eq!(by_run["r2"][0].task_id, "task-b");
        assert!(!by_run.contains_key("r3"));
    }

    #[tokio::test]
    async fn get_harness_eval_task_results_for_runs_empty_input_returns_empty_map() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let by_run = db
            .get_harness_eval_task_results_for_runs(&[])
            .await
            .expect("batched fetch on empty input");
        assert!(by_run.is_empty());
    }

    #[tokio::test]
    async fn harness_eval_run_and_task_results_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        let run = HarnessEvalRunRecord {
            run_id: "abc1234-1000".to_string(),
            triggered_by: "local".to_string(),
            git_sha: "abc1234".to_string(),
            git_branch: "claude/axis-chat-fixes".to_string(),
            changed_files: vec!["crates/vox-orchestrator/src/runtime.rs".to_string()],
            config_version: Some("routing.v1-2026-08-01".to_string()),
            samples_per_task: 3,
            task_count: 2,
            pass_count: 1,
            fail_count: 1,
            skip_count: 0,
            total_cost_usd: 0.002,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        };
        db.record_harness_eval_run(&run).await.expect("record run");

        let task_result = HarnessEvalTaskResultRecord {
            run_id: run.run_id.clone(),
            task_id: "plain-chat-2plus2".to_string(),
            category: "chat".to_string(),
            checker_kind: "deterministic".to_string(),
            status: "pass".to_string(),
            pass_samples: 3,
            total_samples: 3,
            latency_p50_ms: Some(420),
            cost_usd: Some(0.0004),
            failure_detail: None,
            recorded_at_ms: 1500,
        };
        db.record_harness_eval_task_result(&task_result)
            .await
            .expect("record task result");

        let selection_event = ModelSelectionEventRecord {
            run_id: run.run_id.clone(),
            task_id: task_result.task_id.clone(),
            model_id: "deepseek/deepseek-v4-flash".to_string(),
            cost_tier: "free".to_string(),
            selection_reason: "highest score (0.82)".to_string(),
            was_privacy_gated: false,
            recorded_at_ms: 1450,
        };
        db.record_model_selection_event(&selection_event)
            .await
            .expect("record selection event");

        let runs = db.list_harness_eval_runs(10).await.expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, run.run_id);
        assert_eq!(runs[0].pass_count, 1);

        let results = db
            .get_harness_eval_task_results(&run.run_id)
            .await
            .expect("get task results");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].task_id, "plain-chat-2plus2");

        let events = db
            .get_model_selection_events(&run.run_id)
            .await
            .expect("get selection events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cost_tier, "free");
    }
}
