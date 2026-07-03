//! Canonical `agent_runs` ledger ops (B2 GUI harness).
//!
//! One row per agent/CLI invocation, keyed on `run_id`. Purpose-built for the
//! desktop GUI's Runs surface — distinct from the workflow runtime's
//! `workflow_run_log`. Crash-safe: rows persist across restarts, are queryable
//! by id (replay), and as a recent list. See
//! `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md` task B2.

use crate::StoreError;
use crate::VoxDb;

/// One `agent_runs` row.
#[derive(Debug, Clone)]
pub struct AgentRunRow {
    /// Run id (primary key).
    pub run_id: String,
    /// Display label for the run (kept as an alias for the existing Runs UI).
    pub workflow_name: String,
    /// Command line / invocation string.
    pub command: Option<String>,
    /// Repository identifier or path.
    pub repo: Option<String>,
    /// Worktree path.
    pub worktree: Option<String>,
    /// Model id used for the run.
    pub model: Option<String>,
    /// Lifecycle status label (`running` | `completed` | `failed` | ...).
    pub status: String,
    /// Planned step count, if known.
    pub planned_steps: i64,
    /// Completed step count.
    pub completed_steps: i64,
    /// Accrued cost in USD.
    pub cost_usd: f64,
    /// Input token count.
    pub tokens_in: i64,
    /// Output token count.
    pub tokens_out: i64,
    /// Opaque reference to a log location, if any.
    pub logs_ref: Option<String>,
    /// Serialized artifacts list (JSON array string).
    pub artifacts_json: String,
    /// Reference to a HITL approval record (wired by B3), if any.
    pub approval_ref: Option<String>,
    /// Start timestamp (unix-ms).
    pub started_at_ms: i64,
    /// Last-updated timestamp (unix-ms).
    pub updated_at_ms: i64,
    /// Finish timestamp (unix-ms); `None` while running.
    pub completed_at_ms: Option<i64>,
    /// Error text when the run failed.
    pub last_error: Option<String>,
}

const SELECT_COLS: &str = "run_id, workflow_name, command, repo, worktree, model, status, \
     planned_steps, completed_steps, cost_usd, tokens_in, tokens_out, \
     logs_ref, artifacts_json, approval_ref, \
     started_at_ms, updated_at_ms, completed_at_ms, last_error";

fn map_row(row: &turso::Row) -> Result<AgentRunRow, StoreError> {
    let e = |err: turso::Error| StoreError::Db(err.to_string());
    Ok(AgentRunRow {
        run_id: row.get(0).map_err(e)?,
        workflow_name: row.get(1).map_err(e)?,
        command: row.get(2).map_err(e)?,
        repo: row.get(3).map_err(e)?,
        worktree: row.get(4).map_err(e)?,
        model: row.get(5).map_err(e)?,
        status: row.get(6).map_err(e)?,
        planned_steps: row.get(7).map_err(e)?,
        completed_steps: row.get(8).map_err(e)?,
        cost_usd: row.get(9).map_err(e)?,
        tokens_in: row.get(10).map_err(e)?,
        tokens_out: row.get(11).map_err(e)?,
        logs_ref: row.get(12).map_err(e)?,
        artifacts_json: row.get(13).map_err(e)?,
        approval_ref: row.get(14).map_err(e)?,
        started_at_ms: row.get(15).map_err(e)?,
        updated_at_ms: row.get(16).map_err(e)?,
        completed_at_ms: row.get(17).map_err(e)?,
        last_error: row.get(18).map_err(e)?,
    })
}

impl VoxDb {
    /// Insert or update an agent run keyed on `run_id` (latest upsert wins).
    /// `run_id` and `started_at_ms` are immutable on conflict; everything else
    /// advances with the new row.
    pub async fn agent_runs_upsert(&self, row: &AgentRunRow) -> Result<(), StoreError> {
        let row = row.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_runs
                       (run_id, workflow_name, command, repo, worktree, model, status,
                        planned_steps, completed_steps, cost_usd, tokens_in, tokens_out,
                        logs_ref, artifacts_json, approval_ref,
                        started_at_ms, updated_at_ms, completed_at_ms, last_error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                     ON CONFLICT(run_id) DO UPDATE SET
                        workflow_name = excluded.workflow_name,
                        command = excluded.command,
                        repo = excluded.repo,
                        worktree = excluded.worktree,
                        model = excluded.model,
                        status = excluded.status,
                        planned_steps = excluded.planned_steps,
                        completed_steps = excluded.completed_steps,
                        cost_usd = excluded.cost_usd,
                        tokens_in = excluded.tokens_in,
                        tokens_out = excluded.tokens_out,
                        logs_ref = excluded.logs_ref,
                        artifacts_json = excluded.artifacts_json,
                        approval_ref = excluded.approval_ref,
                        updated_at_ms = excluded.updated_at_ms,
                        completed_at_ms = excluded.completed_at_ms,
                        last_error = excluded.last_error",
                    turso::params![
                        row.run_id,
                        row.workflow_name,
                        row.command,
                        row.repo,
                        row.worktree,
                        row.model,
                        row.status,
                        row.planned_steps,
                        row.completed_steps,
                        row.cost_usd,
                        row.tokens_in,
                        row.tokens_out,
                        row.logs_ref,
                        row.artifacts_json,
                        row.approval_ref,
                        row.started_at_ms,
                        row.updated_at_ms,
                        row.completed_at_ms,
                        row.last_error
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch one agent run by id (replay), or `None` if absent.
    pub async fn agent_runs_get(&self, run_id: &str) -> Result<Option<AgentRunRow>, StoreError> {
        let conn = self.conn.clone();
        let sql = format!("SELECT {SELECT_COLS} FROM agent_runs WHERE run_id = ?1");
        let mut cursor = conn.query(&sql, turso::params![run_id.to_string()]).await?;
        match cursor.next().await? {
            Some(row) => Ok(Some(map_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Most-recently-updated agent runs, newest first.
    pub async fn agent_runs_recent(&self, limit: i64) -> Result<Vec<AgentRunRow>, StoreError> {
        let conn = self.conn.clone();
        let sql =
            format!("SELECT {SELECT_COLS} FROM agent_runs ORDER BY updated_at_ms DESC LIMIT ?1");
        let mut cursor = conn.query(&sql, turso::params![limit]).await?;
        let mut out = Vec::new();
        while let Some(row) = cursor.next().await? {
            out.push(map_row(&row)?);
        }
        Ok(out)
    }

    /// T1.5: find the `approval_id` of the most recent `ApprovalRequested`
    /// op-log entry (see `vox_orchestrator_queue::oplog::OperationKind`)
    /// whose `run_id` field matches `run_id`.
    ///
    /// `run_id` on `ApprovalRequested` is populated from
    /// `crates/vox-orchestrator-mcp/src/dispatch.rs`'s `run_id_for_approval`
    /// (explicit `trace_id`/`correlation_id` arg, falling back to the numeric
    /// `task_id` when present — see the T1.5 comment at that call site). GUI
    /// callers should therefore pass the orchestrator's numeric `task_id`
    /// (stringified) here, not the GUI-minted `gui-<uuid>` run id — the two
    /// are distinct id spaces and there is no reliable join on the latter
    /// today. Returns `None` on no match, decode failure, or DB error
    /// (best-effort — never fails the caller's finalize path).
    pub async fn find_approval_id_for_run(&self, run_id: &str) -> Option<String> {
        if run_id.trim().is_empty() {
            return None;
        }
        let conn = self.conn.clone();
        let pattern = format!("%\"ApprovalRequested\"%\"run_id\":\"{run_id}\"%");
        let mut cursor = conn
            .query(
                "SELECT kind_json FROM convergence_op_log \
                 WHERE kind_json LIKE ?1 \
                 ORDER BY op_id DESC LIMIT 20",
                turso::params![pattern],
            )
            .await
            .ok()?;
        while let Ok(Some(row)) = cursor.next().await {
            let kind_json: String = row.get(0).ok()?;
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&kind_json) else {
                continue;
            };
            let Some(obj) = value.get("ApprovalRequested") else {
                continue;
            };
            let matches_run = obj.get("run_id").and_then(|v| v.as_str()) == Some(run_id);
            if !matches_run {
                continue;
            }
            if let Some(approval_id) = obj.get("approval_id").and_then(|v| v.as_str()) {
                return Some(approval_id.to_string());
            }
        }
        None
    }

    /// T1.5: look up the cost/token totals from the `vox-telemetry`
    /// `TaskRootSummaryEvent` for `task_id` (see
    /// `crates/vox-db/src/telemetry_sink.rs`, which writes these under
    /// `session_id = "task:{task_id}"`, `metric_type = "task.root_summary"`).
    ///
    /// Returns `(total_cost_usd, total_input_tokens, total_output_tokens)` from
    /// the newest matching row, or `None` if no telemetry has been recorded yet
    /// for this task (e.g. race with the async telemetry sink, or the run never
    /// went through the orchestrator task pipeline). Best-effort: DB errors and
    /// decode failures also yield `None` rather than propagating, so callers can
    /// fall back to placeholder 0 values without erroring the whole finalize
    /// path.
    pub async fn find_task_root_summary_totals(&self, task_id: &str) -> Option<(f64, i64, i64)> {
        if task_id.trim().is_empty() {
            return None;
        }
        let session_id = format!("task:{task_id}");
        let rows = self
            .list_research_metrics_by_session(&session_id, Some("task.root_summary"), 1)
            .await
            .ok()?;
        let (_, _, _, metadata_json) = rows.into_iter().next()?;
        let metadata_json = metadata_json?;
        let event: serde_json::Value = serde_json::from_str(&metadata_json).ok()?;
        let cost = event.get("total_cost_usd").and_then(|v| v.as_f64())?;
        let tokens_in = event
            .get("total_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i64;
        let tokens_out = event
            .get("total_output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as i64;
        Some((cost, tokens_in, tokens_out))
    }
}
