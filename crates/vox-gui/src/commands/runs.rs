use serde::{Deserialize, Serialize};
use turso::params;
use vox_db::{connect_workspace_journey_optional, DbConnectSurface};

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartGuiRunInput {
    pub run_id: String,
    pub workflow_name: String,
    pub planned_steps: Option<i64>,
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
    let conn = db.connection();
    let ts = now_ms();
    conn.execute(
        "INSERT INTO workflow_run_log
        (run_id, workflow_name, status, planned_steps, completed_steps, started_at_ms, updated_at_ms)
        VALUES (?1, ?2, 'running', ?3, 0, ?4, ?4)
        ON CONFLICT(run_id) DO UPDATE SET
          workflow_name = excluded.workflow_name,
          status = 'running',
          planned_steps = excluded.planned_steps,
          updated_at_ms = excluded.updated_at_ms,
          last_error = NULL",
        params![
            input.run_id,
            input.workflow_name,
            input.planned_steps.unwrap_or(1),
            ts
        ],
    )
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
) -> Result<(), String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let conn = db.connection();
    let ts = now_ms();
    let status = if success { "completed" } else { "failed" };
    conn.execute(
        "UPDATE workflow_run_log
        SET status = ?2,
            completed_steps = COALESCE(?3, completed_steps),
            updated_at_ms = ?4,
            completed_at_ms = ?4,
            last_error = ?5
        WHERE run_id = ?1",
        params![run_id, status, completed_steps, ts, error],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_gui_runs(limit: Option<u32>) -> Result<Vec<GuiRunRecord>, String> {
    let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;
    let conn = db.connection();
    let mut rows = conn
        .query(
            "SELECT run_id, workflow_name, status, planned_steps, completed_steps, started_at_ms, updated_at_ms, completed_at_ms, last_error
             FROM workflow_run_log
             ORDER BY updated_at_ms DESC
             LIMIT ?1",
            params![i64::from(limit.unwrap_or(100))],
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        out.push(GuiRunRecord {
            run_id: row.get(0).map_err(|e| e.to_string())?,
            workflow_name: row.get(1).map_err(|e| e.to_string())?,
            status: row.get(2).map_err(|e| e.to_string())?,
            planned_steps: row.get(3).map_err(|e| e.to_string())?,
            completed_steps: row.get(4).map_err(|e| e.to_string())?,
            started_at_ms: row.get(5).map_err(|e| e.to_string())?,
            updated_at_ms: row.get(6).map_err(|e| e.to_string())?,
            completed_at_ms: row.get(7).map_err(|e| e.to_string())?,
            last_error: row.get(8).map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}
