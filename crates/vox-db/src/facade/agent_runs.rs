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
}
