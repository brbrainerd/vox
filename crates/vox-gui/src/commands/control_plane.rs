use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;
use vox_orchestrator::{FileAffinity, TaskPriority};

use crate::commands::daemon::PersistentDaemon;

#[derive(Debug, Deserialize)]
pub struct SubmitTaskInput {
    pub description: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub priority: Option<String>,
    pub session_id: Option<String>,
    /// Interaction mode from the composer (plan|act|verify); forwarded as an enqueue hint.
    pub mode: Option<String>,
    /// Tier/model preference from the composer; forwarded as model_preference enqueue hint.
    pub tier: Option<String>,
    /// When false, the daemon refuses a near-duplicate (returns duplicate_of with
    /// a null task_id) so the GUI can offer merge/skip. Defaults true.
    pub allow_duplicate: Option<bool>,
    pub model_hint: Option<String>,
    pub dry_run: Option<bool>,
    pub active_skill: Option<String>,
    /// Drive Console clutch label (`free`|`efficiency`|`balanced`|`genius`); forwarded as enqueue hint.
    pub clutch: Option<String>,
    /// Drive Console risk label (`high`|`moderate`|`low`); forwarded as enqueue hint.
    pub risk: Option<String>,
    /// Explicit chat model pick from the ChatModelPicker; forwarded as the
    /// `model_override` enqueue hint (`TaskEnqueueHints.model_override`,
    /// tasks.rs:241-243 → `AgentTask::model_override` via apply_hints
    /// tasks.rs:861-862 → `StreamRoute::UserModelOverride` runtime.rs:408-421).
    pub model_override: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ControlPlaneResult {
    pub ok: bool,
    pub message: String,
    pub task_id: Option<String>,
    /// Set when an existing near-duplicate task was detected (its id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
}

async fn call_orchestrator_daemon(
    daemon: &PersistentDaemon,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => OrchDaemonClient::with_token(addr, token),
        None => OrchDaemonClient::new(addr),
    };
    client.call(method, params).await.map_err(|e| e.to_string())
}

/// Build the daemon SUBMIT_TASK params from the composer input. Extracted from
/// `submit_orchestrator_task` so the enqueue-hint wiring is unit-testable.
fn submit_task_params(input: SubmitTaskInput) -> serde_json::Value {
    let file_manifest: Vec<FileAffinity> = input.files.iter().map(FileAffinity::write).collect();
    let priority = match input.priority.as_deref() {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("normal") => Some(TaskPriority::Normal),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };
    let mut params = serde_json::json!({
        "description": input.description,
        "file_manifest": file_manifest,
        "priority": priority,
        "session_id": input.session_id.filter(|s| !s.trim().is_empty()),
        "allow_duplicate": input.allow_duplicate.unwrap_or(true),
        "model_hint": input.model_hint.filter(|s| !s.trim().is_empty()),
        "dry_run": input.dry_run,
        "active_skill": input.active_skill.filter(|s| !s.trim().is_empty()),
    });
    // Carry composer mode/tier/pick through as enqueue hints (tier →
    // model_preference; explicit pick → model_override). Only attach the key
    // when non-empty — the daemon rejects a null enqueue_hints.
    let mut enqueue_hints = serde_json::Map::new();
    if let Some(tier) = input.tier.as_deref().filter(|t| !t.trim().is_empty()) {
        enqueue_hints.insert("model_preference".into(), serde_json::json!(tier));
    }
    if let Some(model) = input
        .model_override
        .as_deref()
        .filter(|m| !m.trim().is_empty())
    {
        enqueue_hints.insert("model_override".into(), serde_json::json!(model));
    }
    if let Some(mode) = input.mode.as_deref().filter(|m| !m.trim().is_empty()) {
        enqueue_hints.insert("mode".into(), serde_json::json!(mode));
    }
    if let Some(clutch) = input.clutch.as_deref().filter(|c| !c.trim().is_empty()) {
        enqueue_hints.insert("clutch".into(), serde_json::json!(clutch));
    }
    if let Some(risk) = input.risk.as_deref().filter(|r| !r.trim().is_empty()) {
        enqueue_hints.insert("risk".into(), serde_json::json!(risk));
    }
    if !enqueue_hints.is_empty()
        && let Some(obj) = params.as_object_mut()
    {
        obj.insert(
            "enqueue_hints".into(),
            serde_json::Value::Object(enqueue_hints),
        );
    }
    params
}

#[tauri::command]
pub async fn submit_orchestrator_task(
    app_handle: tauri::AppHandle,
    input: SubmitTaskInput,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    let params = submit_task_params(input);
    let response =
        call_orchestrator_daemon(&daemon, orch_daemon_method::SUBMIT_TASK, params).await?;
    let task_id = response
        .get("task_id")
        .and_then(|v| v.as_u64())
        .map(|v| v.to_string());
    let duplicate_of = response
        .get("duplicate_of")
        .and_then(|v| v.as_u64())
        .map(|v| v.to_string());
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: if task_id.is_some() {
            "task submitted".to_string()
        } else {
            "duplicate skipped".to_string()
        },
        task_id,
        duplicate_of,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

#[tauri::command]
pub async fn pause_orchestrator_agent(
    agent_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::PAUSE_AGENT,
        serde_json::json!({ "agent_id": agent_id }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("agent {agent_id} paused"),
        task_id: None,
        duplicate_of: None,
    })
}

#[tauri::command]
pub async fn resume_orchestrator_agent(
    agent_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::RESUME_AGENT,
        serde_json::json!({ "agent_id": agent_id }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("agent {agent_id} resumed"),
        task_id: None,
        duplicate_of: None,
    })
}

#[tauri::command]
pub async fn doubt_orchestrator_task(
    task_id: u64,
    reason: Option<String>,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
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
        duplicate_of: None,
    })
}

#[tauri::command]
pub async fn overrule_orchestrator_task(
    task_id: u64,
    reason: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
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
        duplicate_of: None,
    })
}

#[derive(Debug, Serialize)]
pub struct TaskRowDto {
    pub id: u64,
    pub description: String,
    /// Normalized lowercase: urgent|normal|background.
    pub priority: String,
    /// Normalized snake_case: queued|in_progress|blocked|completed|unknown.
    pub lifecycle: String,
    pub agent_id: Option<u64>,
    pub session_id: Option<String>,
    pub estimated_complexity: u8,
    pub depends_on: Vec<u64>,
    pub write_files: Vec<String>,
    /// Mesh node that claimed this task via A2A (None when local).
    pub remote_node: Option<String>,
}

/// The daemon emits `TaskPriority` Capitalized ("Normal") and lifecycle labels
/// CamelCase ("InProgress"). Normalize once here so the frontend speaks one
/// dialect and `reorder_orchestrator_task` can round-trip lowercase priorities
/// (REORDER_TASK parses lowercase).
fn normalize_lifecycle(raw: &str) -> String {
    match raw {
        "InProgress" => "in_progress".to_string(),
        "Queued" => "queued".to_string(),
        "Blocked" => "blocked".to_string(),
        "Completed" => "completed".to_string(),
        other => other.to_lowercase(),
    }
}

#[tauri::command]
pub async fn list_orchestrator_tasks(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<Vec<TaskRowDto>, String> {
    let response = call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::LIST_TASKS,
        serde_json::json!({}),
    )
    .await?;
    let tasks = response
        .get("tasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(tasks
        .into_iter()
        .map(|t| TaskRowDto {
            id: t.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            priority: t
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("Normal")
                .to_lowercase(),
            lifecycle: normalize_lifecycle(
                t.get("lifecycle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
            ),
            agent_id: t.get("agent_id").and_then(|v| v.as_u64()),
            session_id: t
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            estimated_complexity: t
                .get("estimated_complexity")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u8,
            depends_on: t
                .get("depends_on")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default(),
            write_files: t
                .get("write_files")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(ToString::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            remote_node: t
                .get("remote_node")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
        })
        .collect())
}

#[tauri::command]
pub async fn edit_orchestrator_task(
    app_handle: tauri::AppHandle,
    task_id: u64,
    description: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::EDIT_TASK,
        serde_json::json!({ "task_id": task_id, "description": description }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} updated"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

#[tauri::command]
pub async fn cancel_orchestrator_task(
    app_handle: tauri::AppHandle,
    task_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::CANCEL_TASK,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} cancelled"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

#[tauri::command]
pub async fn interrupt_orchestrator_task(
    app_handle: tauri::AppHandle,
    task_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::INTERRUPT_TASK,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} interrupted"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

#[tauri::command]
pub async fn reorder_orchestrator_task(
    app_handle: tauri::AppHandle,
    task_id: u64,
    priority: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::REORDER_TASK,
        serde_json::json!({ "task_id": task_id, "priority": priority }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} → {priority}"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

// ── Plan/Act/Verify intervention commands (Track D) ─────────────────────────

/// Approve the plan phase of a task, advancing its PAV loop to Acting.
#[tauri::command]
pub async fn approve_orchestrator_task_plan(
    app_handle: tauri::AppHandle,
    task_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::APPROVE_PLAN,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} plan approved → acting"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

/// Skip the verification phase of a task (Acting → Done, bypassing Verifying).
#[tauri::command]
pub async fn skip_orchestrator_verify(
    app_handle: tauri::AppHandle,
    task_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::SKIP_VERIFY,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} verification skipped"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

/// Force a verification phase for a task (Acting → Verifying).
#[tauri::command]
pub async fn force_orchestrator_verify(
    app_handle: tauri::AppHandle,
    task_id: u64,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::FORCE_VERIFY,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} forced to verify"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}

#[cfg(test)]
mod tests {
    // Verify the module imports compile cleanly.
    use super::*;
    #[test]
    fn smoke_imports() {
        // If this test compiles, our imports are correct.
        let _ = std::mem::size_of::<SubmitTaskInput>();
        let _ = std::mem::size_of::<ControlPlaneResult>();
    }
}

#[cfg(test)]
mod submit_params_tests {
    use super::*;

    fn input(model_override: Option<&str>) -> SubmitTaskInput {
        SubmitTaskInput {
            description: "wire the picker".into(),
            files: vec![],
            priority: None,
            session_id: Some("gui-session-9".into()),
            mode: None,
            tier: None,
            allow_duplicate: None,
            model_hint: None,
            dry_run: None,
            active_skill: None,
            clutch: None,
            risk: None,
            model_override: model_override.map(str::to_string),
        }
    }

    #[test]
    fn submit_params_carry_model_override_enqueue_hint() {
        let params = submit_task_params(input(Some("anthropic/claude-opus-4.7")));
        assert_eq!(
            params["enqueue_hints"]["model_override"],
            "anthropic/claude-opus-4.7"
        );
    }

    #[test]
    fn no_pick_means_no_override_hint() {
        let params = submit_task_params(input(None));
        // No other hints set either ⇒ the enqueue_hints key is absent entirely
        // (the daemon rejects a null enqueue_hints).
        assert!(params.get("enqueue_hints").is_none());
    }
}
