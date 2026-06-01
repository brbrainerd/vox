use serde::{Deserialize, Serialize};
use vox_db::{AgentRunRow, DbConnectSurface, connect_workspace_journey_optional};

#[derive(Debug, Clone, Serialize)]
pub struct GuiRunRecord {
    pub run_id: String,
    pub workflow_name: String,
    pub status: String,
    pub planned_steps: i64,
    pub completed_steps: i64,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub last_error: Option<String>,
    // New fields surfaced from agent_runs (Option so older rows are fine).
    pub command: Option<String>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
}

impl From<AgentRunRow> for GuiRunRecord {
    fn from(row: AgentRunRow) -> Self {
        GuiRunRecord {
            run_id: row.run_id,
            workflow_name: row.workflow_name,
            status: row.status,
            planned_steps: row.planned_steps,
            completed_steps: row.completed_steps,
            started_at_ms: row.started_at_ms,
            updated_at_ms: row.updated_at_ms,
            completed_at_ms: row.completed_at_ms,
            last_error: row.last_error,
            command: row.command,
            model: row.model,
            cost_usd: Some(row.cost_usd),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartGuiRunInput {
    pub run_id: String,
    pub workflow_name: String,
    pub planned_steps: Option<i64>,
    pub command: Option<String>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[tauri::command]
pub async fn start_gui_run(input: StartGuiRunInput) -> Result<(), String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let ts = now_ms();
    let row = AgentRunRow {
        run_id: input.run_id,
        workflow_name: input.workflow_name,
        command: input.command,
        repo: input.repo,
        worktree: input.worktree,
        model: input.model,
        status: "running".to_string(),
        planned_steps: input.planned_steps.unwrap_or(1),
        completed_steps: 0,
        cost_usd: 0.0,
        tokens_in: 0,
        tokens_out: 0,
        logs_ref: None,
        artifacts_json: "[]".to_string(),
        approval_ref: None,
        started_at_ms: ts,
        updated_at_ms: ts,
        completed_at_ms: None,
        last_error: None,
    };
    db.agent_runs_upsert(&row)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn finish_gui_run(
    run_id: String,
    success: bool,
    completed_steps: Option<i64>,
    error: Option<String>,
    cost_usd: Option<f64>,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
) -> Result<(), String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let ts = now_ms();
    let status = if success { "completed" } else { "failed" };

    // Load the existing row so we preserve start metadata; build a minimal
    // running-row fallback if this run was never started via start_gui_run.
    let mut row = match db
        .agent_runs_get(&run_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(existing) => existing,
        None => AgentRunRow {
            run_id: run_id.clone(),
            workflow_name: run_id.clone(),
            command: None,
            repo: None,
            worktree: None,
            model: None,
            status: status.to_string(),
            planned_steps: completed_steps.unwrap_or(0),
            completed_steps: 0,
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
            logs_ref: None,
            artifacts_json: "[]".to_string(),
            approval_ref: None,
            started_at_ms: ts,
            updated_at_ms: ts,
            completed_at_ms: None,
            last_error: None,
        },
    };

    row.status = status.to_string();
    if let Some(steps) = completed_steps {
        row.completed_steps = steps;
    }
    if let Some(cost) = cost_usd {
        row.cost_usd = cost;
    }
    if let Some(tin) = tokens_in {
        row.tokens_in = tin;
    }
    if let Some(tout) = tokens_out {
        row.tokens_out = tout;
    }
    row.updated_at_ms = ts;
    row.completed_at_ms = Some(ts);
    row.last_error = error;

    db.agent_runs_upsert(&row)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_gui_runs(limit: Option<u32>) -> Result<Vec<GuiRunRecord>, String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let rows = db
        .agent_runs_recent(i64::from(limit.unwrap_or(100)))
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(GuiRunRecord::from).collect())
}

#[tauri::command]
pub async fn get_gui_run(run_id: String) -> Result<Option<GuiRunRecord>, String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let row = db
        .agent_runs_get(&run_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(GuiRunRecord::from))
}
