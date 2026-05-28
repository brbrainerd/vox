use serde::{Deserialize, Serialize};
use vox_cli_core::daemon_ipc::dispatch::call_daemon;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::{FileAffinity, TaskPriority};

#[derive(Debug, Deserialize)]
pub struct SubmitTaskInput {
    pub description: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub priority: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ControlPlaneResult {
    pub ok: bool,
    pub message: String,
    pub task_id: Option<String>,
}

async fn call_orchestrator_daemon(
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    call_daemon("vox-orchestrator-d", method, params, false)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_orchestrator_task(input: SubmitTaskInput) -> Result<ControlPlaneResult, String> {
    let file_manifest: Vec<FileAffinity> = input.files.iter().map(FileAffinity::write).collect();
    let priority = match input.priority.as_deref() {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("normal") => Some(TaskPriority::Normal),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };
    let response = call_orchestrator_daemon(
        orch_daemon_method::SUBMIT_TASK,
        serde_json::json!({
            "description": input.description,
            "file_manifest": file_manifest,
            "priority": priority,
            "session_id": input.session_id.filter(|s| !s.trim().is_empty()),
        }),
    )
    .await?;
    let task_id = response.get("task_id").and_then(|v| v.as_u64()).map(|v| v.to_string());
    Ok(ControlPlaneResult {
        ok: true,
        message: "task submitted".to_string(),
        task_id,
    })
}

#[tauri::command]
pub async fn pause_orchestrator_agent(agent_id: u64) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::PAUSE_AGENT,
        serde_json::json!({ "agent_id": agent_id }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("agent {agent_id} paused"),
        task_id: None,
    })
}

#[tauri::command]
pub async fn resume_orchestrator_agent(agent_id: u64) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::RESUME_AGENT,
        serde_json::json!({ "agent_id": agent_id }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("agent {agent_id} resumed"),
        task_id: None,
    })
}

#[tauri::command]
pub async fn doubt_orchestrator_task(
    task_id: u64,
    reason: Option<String>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::DOUBT_TASK,
        serde_json::json!({
            "task_id": task_id,
            "reason": reason,
        }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} marked as suspect"),
        task_id: Some(task_id.to_string()),
    })
}

#[tauri::command]
pub async fn overrule_orchestrator_task(
    task_id: u64,
    reason: String,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::OVERRULE_TASK,
        serde_json::json!({
            "task_id": task_id,
            "reason": reason,
        }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} overruled"),
        task_id: Some(task_id.to_string()),
    })
}
