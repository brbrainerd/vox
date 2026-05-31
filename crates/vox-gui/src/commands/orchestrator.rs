use std::sync::Arc;

use tauri::Emitter;
use vox_cli_core::daemon_ipc::dispatch::call_daemon;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;
use vox_package_types::manifest::VoxManifest;

use crate::commands::daemon::PersistentDaemon;

/// Tauri event channel carrying live orchestrator status snapshots to the UI.
pub const ORCH_STATUS_EVENT: &str = "vox://orch-status";

/// Spawn a background task that subscribes to the orchestrator daemon's status
/// stream and re-emits each snapshot as the [`ORCH_STATUS_EVENT`] Tauri event.
///
/// Resilient by design: if the daemon is unavailable or the stream ends, the task
/// simply exits without crashing the app. The emitted payload has the same shape
/// as [`get_orchestrator_status`] (the GUI-mapped status object with `agent_count`).
pub fn spawn_orchestrator_status_stream(
    app_handle: tauri::AppHandle,
    daemon: Arc<PersistentDaemon>,
) {
    tokio::spawn(async move {
        let addr = match daemon.ensure().await {
            Ok(a) => a,
            Err(_) => return,
        };
        let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);

        // Drive the subscription in its own task so we can drain `rx` concurrently.
        let producer = tokio::spawn(async move {
            let _ = OrchDaemonClient::new(addr).subscribe(tx).await;
        });

        while let Some(raw) = rx.recv().await {
            let gui_status = to_gui_status(raw);
            if let Ok(value) = serde_json::to_value(&gui_status) {
                let _ = app_handle.emit(ORCH_STATUS_EVENT, value);
            }
        }

        // Stream ended (daemon stopped or errored); let the producer wind down.
        let _ = producer.await;
    });
}

/// Tauri event channel carrying live agent events from the orchestrator daemon
/// to the UI. Payload is a serialized `AgentEvent` value
/// (`{ id, timestamp_ms, kind: { type, ..fields } }`).
pub const AGENT_EVENTS_EVENT: &str = "vox://agent-events";

/// Spawn a background task that subscribes to the orchestrator daemon's
/// agent-event stream and re-emits each event as the [`AGENT_EVENTS_EVENT`]
/// Tauri event.
///
/// Mirrors [`spawn_orchestrator_status_stream`] (the B1 pattern). Resilient by
/// design: if the daemon is unavailable or the stream ends, the task simply
/// exits without crashing the app. Each emitted payload is the raw serialized
/// `AgentEvent` forwarded verbatim from the daemon.
pub fn spawn_agent_event_stream(app_handle: tauri::AppHandle, daemon: Arc<PersistentDaemon>) {
    tokio::spawn(async move {
        let addr = match daemon.ensure().await {
            Ok(a) => a,
            Err(_) => return,
        };
        let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(256);

        // Drive the subscription in its own task so we can drain `rx` concurrently.
        let producer = tokio::spawn(async move {
            let _ = OrchDaemonClient::new(addr).subscribe_events(tx).await;
        });

        while let Some(value) = rx.recv().await {
            let _ = app_handle.emit(AGENT_EVENTS_EVENT, value);
        }

        // Stream ended (daemon stopped or errored); let the producer wind down.
        let _ = producer.await;
    });
}

#[derive(Debug, serde::Serialize)]
pub struct GuiAgentSummary {
    pub id: u64,
    pub codename: String,
    pub paused: bool,
    pub in_progress: bool,
    pub current_phase: Option<String>,
    pub active_skill: Option<String>,
    pub queued: usize,
    pub urgent_count: usize,
    pub normal_count: usize,
    pub background_count: usize,
    pub completed: usize,
    pub owned_files: usize,
    pub weighted_load: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct GuiOrchestratorStatus {
    pub agent_count: usize,
    pub total_queued: usize,
    pub total_in_progress: usize,
    pub total_completed: usize,
    pub total_doubted: usize,
    pub total_weighted_load: f64,
    pub predicted_load: f64,
    pub agents: Vec<GuiAgentSummary>,
    pub recent_events: Vec<serde_json::Value>,
    pub alerts: Vec<serde_json::Value>,
    pub peers: Vec<serde_json::Value>,
    pub total_cost: f64,
    pub budget_cap: f64,
    pub mesh_throughput: f64,
    pub total_vram_gb: f64,
}

fn get_u64(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn get_f64(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

async fn daemon_status() -> Result<serde_json::Value, String> {
    call_daemon(
        "vox-orchestrator-d",
        orch_daemon_method::STATUS,
        serde_json::json!({}),
        false,
    )
    .await
    .map_err(|e| e.to_string())
}

fn to_gui_status(status: serde_json::Value) -> GuiOrchestratorStatus {
    GuiOrchestratorStatus {
        agent_count: get_u64(&status, "agent_count") as usize,
        total_queued: get_u64(&status, "total_queued") as usize,
        total_in_progress: get_u64(&status, "total_in_progress") as usize,
        total_completed: get_u64(&status, "total_completed") as usize,
        total_doubted: get_u64(&status, "total_doubted") as usize,
        total_weighted_load: get_f64(&status, "total_weighted_load"),
        predicted_load: get_f64(&status, "predicted_load"),
        agents: status
            .get("agents")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|agent| GuiAgentSummary {
                id: agent.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
                codename: agent
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Agent")
                    .to_string(),
                paused: agent
                    .get("paused")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                in_progress: agent
                    .get("in_progress")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                current_phase: agent
                    .get("current_phase")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                active_skill: agent
                    .get("active_skill")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                queued: agent.get("queued").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                urgent_count: agent
                    .get("urgent_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                normal_count: agent
                    .get("normal_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                background_count: agent
                    .get("background_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                completed: agent.get("completed").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                owned_files: agent
                    .get("owned_files")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                weighted_load: agent
                    .get("weighted_load")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            })
            .collect(),
        recent_events: Vec::new(),
        alerts: Vec::new(),
        peers: Vec::new(),
        total_cost: 0.0,
        budget_cap: 50.0,
        mesh_throughput: 0.0,
        total_vram_gb: 0.0,
    }
}

#[tauri::command]
pub async fn get_orchestrator_status() -> Result<serde_json::Value, String> {
    let status = daemon_status().await?;
    serde_json::to_value(to_gui_status(status)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_orchestrator_status_bin() -> Result<tauri::ipc::Response, String> {
    let status = daemon_status().await?;
    let gui_status = to_gui_status(status);
    let bytes = rmp_serde::to_vec_named(&gui_status).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn set_orchestrator_config(config: serde_json::Value) -> Result<(), String> {
    // 1. Discover Vox.toml
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;

    // 2. Parse incoming JSON overrides into the orchestrator table
    let mut orch_table = manifest.orchestrator.unwrap_or_default();

    if let Some(c) = config.get("concurrency").and_then(|v| v.as_u64()) {
        orch_table.insert("max_agents".to_string(), toml::Value::Integer(c as i64));
    }
    if let Some(cap) = config.get("capUsd").and_then(|v| v.as_f64()) {
        // Convert USD to micros
        orch_table.insert(
            "financial_cost_budget_micros".to_string(),
            toml::Value::Integer((cap * 1_000_000.0) as i64),
        );
    }
    if let Some(doubt) = config.get("doubtThresh").and_then(|v| v.as_f64()) {
        orch_table.insert(
            "trust_auto_approve_min".to_string(),
            toml::Value::Float(doubt),
        );
    }
    if let Some(iso) = config.get("isolation").and_then(|v| v.as_str()) {
        let val = if iso == "wasm" {
            "Wasm"
        } else if iso == "ctr" {
            "Container"
        } else {
            "Native"
        };
        orch_table.insert(
            "scope_enforcement".to_string(),
            toml::Value::String(val.to_string()),
        );
    }
    if let Some(auto) = config.get("autobudget").and_then(|v| v.as_bool()) {
        orch_table.insert(
            "exec_time_budget_enabled".to_string(),
            toml::Value::Boolean(auto),
        );
    }
    if let Some(shadow) = config.get("doubt").and_then(|v| v.as_bool()) {
        orch_table.insert(
            "socrates_gate_shadow".to_string(),
            toml::Value::Boolean(!shadow),
        );
        orch_table.insert(
            "socrates_gate_enforce".to_string(),
            toml::Value::Boolean(shadow),
        );
    }

    // 3. Save it back
    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    // 4. Try to signal vox-orchestrator-d to hot-reload if it is running
    // We do this in a fire-and-forget manner to not block or fail the UI update.
    tokio::spawn(async move {
        let _ = vox_cli_core::daemon_ipc::dispatch::call_daemon(
            "vox-orchestrator-d",
            vox_foundation::protocol::orch_daemon_method::RELOAD_CONFIG,
            serde_json::json!({}),
            false,
        )
        .await;
    });

    Ok(())
}
