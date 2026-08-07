use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use vox_config::timeouts::D_2S;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;
use vox_package_types::manifest::VoxManifest;

use crate::commands::daemon::PersistentDaemon;

/// Tauri event channel carrying live orchestrator status snapshots to the UI.
pub const ORCH_STATUS_EVENT: &str = "vox://orch-status";

/// Backoff between reconnect attempts for the status/event streams (T3.1).
/// Short enough that a reconnect is not user-visible for a transient blip,
/// long enough not to hammer a daemon that is still coming up after a spawn.
const STREAM_RECONNECT_BACKOFF: std::time::Duration = D_2S;

/// Spawn a background task that subscribes to the orchestrator daemon's status
/// stream and re-emits each snapshot as the [`ORCH_STATUS_EVENT`] Tauri event.
///
/// T3.1: this is a **reconnect loop**, not a one-shot subscription — when the
/// stream ends for any reason (daemon death, network blip, subscribe error),
/// it waits [`STREAM_RECONNECT_BACKOFF`], re-resolves the daemon via
/// [`PersistentDaemon::ensure_live`] (which detects and replaces a dead
/// cached daemon rather than reusing a stale address), and resubscribes. The
/// loop only exits if the `AppHandle` itself is gone (the app is shutting
/// down), so a mid-session daemon death self-heals instead of producing
/// permanent silent staleness. The emitted payload has the same shape as
/// [`get_orchestrator_status`] (the GUI-mapped status object with `agent_count`).
// toestub-ignore(skeleton/untested-pub-api) — spawns a background task bridging the orchestrator daemon to Tauri events; covered by integration
pub fn spawn_orchestrator_status_stream(
    app_handle: tauri::AppHandle,
    daemon: Arc<PersistentDaemon>,
) {
    tokio::spawn(async move {
        loop {
            let addr = match daemon.ensure_live().await {
                Ok(a) => a,
                Err(e) => {
                    tracing::debug!("daemon unavailable: {e}; retrying");
                    tokio::time::sleep(STREAM_RECONNECT_BACKOFF).await;
                    continue;
                }
            };
            let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(
                crate::config::ORCH_STATUS_CHANNEL_CAP,
            );

            let token = daemon.token().await;
            // Drive the subscription in its own task so we can drain `rx` concurrently.
            let producer = tokio::spawn(async move {
                let client = match token {
                    Some(t) => OrchDaemonClient::with_token(addr, t),
                    None => OrchDaemonClient::new(addr),
                };
                client.subscribe(tx).await
            });

            while let Some(raw) = rx.recv().await {
                let gui_status = to_gui_status(raw);
                if let Ok(value) = serde_json::to_value(&gui_status) {
                    let _ = app_handle.emit(ORCH_STATUS_EVENT, value);
                }
            }

            // Stream ended (daemon stopped or errored). Invalidate the cached
            // daemon so the next loop iteration's `ensure_live` re-resolves
            // rather than trusting a connection we just watched die, then
            // back off before reconnecting.
            let _ = producer.await;
            daemon.invalidate().await;
            tokio::time::sleep(STREAM_RECONNECT_BACKOFF).await;
        }
    });
}

/// Tauri event channel carrying live agent events from the orchestrator daemon
/// to the UI. Payload is a serialized `AgentEvent` value
/// (`{ id, timestamp_ms, kind: { type, ..fields } }`).
pub const AGENT_EVENTS_EVENT: &str = "vox://agent-events";

/// Extract the durable-op offset (`op_id`) to resume from after a reconnect,
/// from either a live `AgentEvent` frame's `id` field or a T1.3
/// replay-envelope frame's `op_id` field (`{ replay: true, op_id, ... }`,
/// see `orch_daemon::replay_frame_value`). Falls back to leaving the offset
/// unchanged (`None`) if the frame has neither — a best-effort forward
/// cursor, not a correctness-critical one (a missed bump just means the next
/// reconnect replays slightly more than strictly necessary, never less).
fn extract_offset(value: &serde_json::Value) -> Option<u64> {
    value
        .get("op_id")
        .and_then(|v| v.as_u64())
        .or_else(|| value.get("id").and_then(|v| v.as_u64()))
}

/// Spawn a background task that subscribes to the orchestrator daemon's
/// agent-event stream and re-emits each event as the [`AGENT_EVENTS_EVENT`]
/// Tauri event.
///
/// Mirrors [`spawn_orchestrator_status_stream`] (the B1 pattern) as a
/// **reconnect loop** (T3.1): the first connection uses plain
/// `subscribe_events`; every reconnect after a stream drop uses
/// [`OrchDaemonClient::subscribe_events_from_offset`] with the last-seen
/// event's offset (tracked via [`extract_offset`]) so events emitted during
/// the outage are replayed from Tier-A durable history instead of silently
/// skipped. If the daemon is unavailable, the task backs off and retries
/// rather than exiting. Each emitted payload is the raw serialized
/// `AgentEvent` (or T1.3 replay envelope) forwarded verbatim from the daemon.
// toestub-ignore(skeleton/untested-pub-api) — spawns a background task bridging the orchestrator daemon to Tauri events; covered by integration
pub fn spawn_agent_event_stream(app_handle: tauri::AppHandle, daemon: Arc<PersistentDaemon>) {
    tokio::spawn(async move {
        let mut last_offset: Option<u64> = None;
        loop {
            let addr = match daemon.ensure_live().await {
                Ok(a) => a,
                Err(e) => {
                    tracing::debug!("daemon unavailable: {e}; retrying");
                    tokio::time::sleep(STREAM_RECONNECT_BACKOFF).await;
                    continue;
                }
            };
            let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(
                crate::config::AGENT_EVENTS_CHANNEL_CAP,
            );

            let token = daemon.token().await;
            let from_offset = last_offset;
            // Drive the subscription in its own task so we can drain `rx` concurrently.
            let producer = tokio::spawn(async move {
                let client = match token {
                    Some(t) => OrchDaemonClient::with_token(addr, t),
                    None => OrchDaemonClient::new(addr),
                };
                match from_offset {
                    // Reconnect: replay everything since the last event we saw.
                    Some(offset) => client.subscribe_events_from_offset(offset, tx).await,
                    // First connection: plain live-tail, no replay needed.
                    None => client.subscribe_events(tx).await,
                }
            });

            while let Some(value) = rx.recv().await {
                if let Some(offset) = extract_offset(&value) {
                    last_offset = Some(offset);
                }
                let _ = app_handle.emit(AGENT_EVENTS_EVENT, value);
            }

            // Stream ended (daemon stopped or errored). Invalidate the cached
            // daemon so the next loop iteration's `ensure_live` re-resolves,
            // then back off before reconnecting (with replay-from-offset).
            let _ = producer.await;
            daemon.invalidate().await;
            tokio::time::sleep(STREAM_RECONNECT_BACKOFF).await;
        }
    });
}

/// Tauri event emitted every time any orchestrator task changes state
/// (created, updated, reordered, cancelled). Frontend subscribers should
/// call their refresh function on receipt.
pub const TASKS_CHANGED_EVENT: &str = "vox://tasks-changed";

/// Emit [`TASKS_CHANGED_EVENT`] to all webview windows.
///
/// Call this after any mutation to orchestrator task state. The frontend
/// `TasksView` subscribes to this event to refresh its task list without
/// polling.
pub fn emit_tasks_changed<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    // `emit` broadcasts to all windows. A unit payload `()` serialises to `null`.
    let _ = app_handle.emit(TASKS_CHANGED_EVENT, ());
}

/// Tauri event emitted when the secretary detects actionable intent in a
/// chat message and *proposes* a task. Task 0.2 (harness parity plan): the
/// secretary is propose-only — this event does not mean a task exists yet.
/// The frontend must call `secretary_confirm_task` (passing back
/// `session_id`/`intent`) to actually submit it. Payload: `SecretaryProposedPayload`.
pub const SECRETARY_PROPOSED_EVENT: &str = "vox://secretary-proposed-task";

/// Payload for the [`SECRETARY_PROPOSED_EVENT`] Tauri event.
#[derive(Debug, serde::Serialize, Clone)]
pub struct SecretaryProposedPayload {
    /// Client-side proposal id (NOT a hopper/task id — no task has been
    /// submitted yet). Only used to correlate the toast with a confirm/dismiss
    /// action in the frontend.
    pub item_id: String,
    /// Cleaned intent text that would be submitted as the task description
    /// if the user confirms.
    pub intent: String,
    /// Classifier confidence 0–100 (for UI display only; not a guarantee).
    pub confidence_pct: u8,
    /// Chat session id the message came from — passed back to
    /// `secretary_confirm_task` on confirm.
    pub session_id: String,
}

/// Emit [`SECRETARY_PROPOSED_EVENT`] to all webview windows.
pub fn emit_secretary_proposed<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    payload: SecretaryProposedPayload,
) {
    let _ = app_handle.emit(SECRETARY_PROPOSED_EVENT, payload);
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
    pub cost: Option<f64>,
    pub budget: Option<f64>,
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
    /// Live attention-budget snapshot passed through verbatim from the daemon (Track D). May be null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention_budget: Option<serde_json::Value>,
}

fn get_u64(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn get_f64(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

/// Read a numeric field only if present, leaving `None` (unknown) otherwise.
/// Never fabricates a value when the daemon did not report one.
fn get_opt_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
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

async fn daemon_status(daemon: &PersistentDaemon) -> Result<serde_json::Value, String> {
    call_orchestrator_daemon(daemon, orch_daemon_method::STATUS, serde_json::json!({})).await
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
                // Per-agent financial cost from the daemon (USD). Absent → unknown.
                cost: get_opt_f64(&agent, "cost_usd"),
                budget: get_opt_f64(&agent, "budget_usd"),
            })
            .collect(),
        recent_events: status
            .get("recent_events")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        alerts: Vec::new(),
        peers: status
            .get("peers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        total_cost: get_f64(&status, "total_cost_usd"),
        budget_cap: get_f64(&status, "budget_cap_usd"),
        mesh_throughput: get_f64(&status, "mesh_throughput_mb_s"),
        total_vram_gb: get_f64(&status, "total_vram_gb"),
        attention_budget: status
            .get("attention_budget")
            .cloned()
            .filter(|v| !v.is_null()),
    }
}

async fn enrich_mesh_from_tool(gui: &mut GuiOrchestratorStatus, daemon: &PersistentDaemon) {
    let mesh = call_orchestrator_daemon(
        daemon,
        orch_daemon_method::TOOL_CALL,
        serde_json::json!({ "name": "vox_mesh_nodes", "args": {} }),
    )
    .await;
    if let Ok(value) = mesh {
        let nodes = value
            .get("result")
            .or(Some(&value))
            .and_then(|r| r.get("nodes"))
            .and_then(|n| n.as_array());
        if let Some(nodes) = nodes {
            gui.peers = nodes.clone();
            gui.total_vram_gb = nodes
                .iter()
                .filter_map(|n| n.get("vram_gb").and_then(|v| v.as_f64()))
                .sum();
        }
    }
}

#[tauri::command]
pub async fn get_orchestrator_status(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<serde_json::Value, String> {
    let status = daemon_status(&daemon).await?;
    let mut gui = to_gui_status(status);
    if gui.peers.is_empty() {
        enrich_mesh_from_tool(&mut gui, &daemon).await;
    }
    gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;
    serde_json::to_value(gui).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_orchestrator_status_bin(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<tauri::ipc::Response, String> {
    let status = daemon_status(&daemon).await?;
    let mut gui = to_gui_status(status);
    if gui.peers.is_empty() {
        enrich_mesh_from_tool(&mut gui, &daemon).await;
    }
    gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;
    let bytes = rmp_serde::to_vec_named(&gui).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn set_orchestrator_config(
    app_handle: tauri::AppHandle,
    config: serde_json::Value,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<(), String> {
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
    // Scaling section (Track D): local-resource-aware auto-scaling controls.
    if let Some(v) = config.get("scalingEnabled").and_then(|v| v.as_bool()) {
        orch_table.insert("scaling_enabled".to_string(), toml::Value::Boolean(v));
    }
    if let Some(v) = config.get("minAgents").and_then(|v| v.as_u64()) {
        orch_table.insert("min_agents".to_string(), toml::Value::Integer(v as i64));
    }
    if let Some(v) = config.get("scalingThreshold").and_then(|v| v.as_u64()) {
        orch_table.insert(
            "scaling_threshold".to_string(),
            toml::Value::Integer(v as i64),
        );
    }
    if let Some(v) = config.get("scaleCpuCeilingPct").and_then(|v| v.as_f64()) {
        orch_table.insert("scale_cpu_ceiling_pct".to_string(), toml::Value::Float(v));
    }
    if let Some(v) = config.get("scaleMemFloorMb").and_then(|v| v.as_u64()) {
        orch_table.insert(
            "scale_mem_floor_mb".to_string(),
            toml::Value::Integer(v as i64),
        );
    }

    // 3. Save it back
    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    // 3b. Bump the vox-config snapshot so caches are invalidated and the
    //     `vox://orchestrator-config-changed` listener fires on the GUI side.
    vox_config::snapshot::bump(&[
        "max_agents",
        "financial_cost_budget_micros",
        "trust_auto_approve_min",
        "scope_enforcement",
        "exec_time_budget_enabled",
        "socrates_gate_enforce",
        "scaling_enabled",
        "min_agents",
        "scaling_threshold",
        "scale_cpu_ceiling_pct",
        "scale_mem_floor_mb",
    ]);
    // Also emit the event directly so the GUI updates immediately even if the
    // snapshot listener fires before the Tauri event loop processes the callback.
    let _ = app_handle.emit(
        ORCH_CONFIG_CHANGED_EVENT,
        OrchestratorConfigChanged {
            rev: vox_config::snapshot::current_rev(),
        },
    );

    // 4. Try to signal vox-orchestrator-d to hot-reload if it is running
    // We do this in a fire-and-forget manner to not block or fail the UI update.
    let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
    tokio::spawn(async move {
        let _ = call_orchestrator_daemon(
            &daemon,
            orch_daemon_method::RELOAD_CONFIG,
            serde_json::json!({}),
        )
        .await;
    });

    Ok(())
}

/// Tauri event channel the frontend subscribes to for reactive Orchestrator
/// settings refresh — emitted whenever the orchestrator config snapshot is bumped.
pub const ORCH_CONFIG_CHANGED_EVENT: &str = "vox://orchestrator-config-changed";

/// Payload for [`ORCH_CONFIG_CHANGED_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfigChanged {
    /// Monotonic snapshot revision at the time of the bump.
    pub rev: u64,
}

/// Spawn once at GUI startup: forward `vox-config` snapshot bumps that affect
/// orchestrator config keys to the webview as [`ORCH_CONFIG_CHANGED_EVENT`],
/// so the Orchestrator settings surface refreshes reactively when config
/// changes — whether from this GUI, an env reload, or mesh sync.
///
/// Mirrors [`crate::commands::user_config::spawn_llm_config_bridge`] (the B2
/// pattern). Listeners are cheap synchronous callbacks; the Tauri emit is
/// non-blocking so this satisfies the `vox-config` snapshot contract.
// toestub-ignore(skeleton/untested-pub-api) — thin bridge from vox-config snapshot bumps to Tauri events; snapshot invalidation logic is tested in vox-config
pub fn spawn_orchestrator_config_watch(app: tauri::AppHandle) {
    vox_config::snapshot::on_change(move |change| {
        // Only forward bumps that are a general reload (empty keys) or that
        // contain at least one orchestrator key. This prevents spurious
        // full-catalog refetches triggered by unrelated bumps such as LLM
        // config changes or EnvScratch::drop.
        let is_orch = change.changed.is_empty()
            || change.changed.iter().any(|k| {
                k.starts_with("VOX_ORCHESTRATOR_") || k.starts_with("VOX_CIRCUIT_BREAKER_")
            });
        if is_orch {
            let _ = app.emit(
                ORCH_CONFIG_CHANGED_EVENT,
                OrchestratorConfigChanged { rev: change.rev },
            );
        }
    });
}

/// Return all orchestrator config fields as structured metadata so the frontend
/// can render the settings dynamically without hardcoded lists (Band B.3).
///
/// Each entry carries: key, label, field_type, current_value, default_value,
/// group, and description — everything the UI needs to build a settings form.
#[tauri::command]
pub async fn get_orchestrator_config_catalog() -> Vec<vox_orchestrator::OrchestratorConfigField> {
    vox_orchestrator::config::OrchestratorConfig::snapshot().to_catalog()
}

/// Current per-task-type policy overrides, straight from the effective
/// `OrchestratorConfig` snapshot (env/project/user-merged, matching every
/// other orchestrator-settings read in this file).
#[tauri::command]
pub fn get_task_policy_overrides() -> vox_orchestrator::config::TaskPolicyOverrides {
    vox_orchestrator::config::OrchestratorConfig::snapshot().task_policy
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DefaultTaskPolicyDto {
    pub clutch: String,
    pub risk: String,
}

fn clutch_label(c: vox_orchestrator::mode::ClutchProfile) -> &'static str {
    match c {
        vox_orchestrator::mode::ClutchProfile::Free => "free",
        vox_orchestrator::mode::ClutchProfile::Efficiency => "efficiency",
        vox_orchestrator::mode::ClutchProfile::Balanced => "balanced",
        vox_orchestrator::mode::ClutchProfile::Genius => "genius",
    }
}

fn risk_label(r: vox_orchestrator::mode::RiskPosture) -> &'static str {
    match r {
        vox_orchestrator::mode::RiskPosture::High => "high",
        vox_orchestrator::mode::RiskPosture::Moderate => "moderate",
        vox_orchestrator::mode::RiskPosture::Low => "low",
    }
}

/// The composer's real starting clutch/risk for `task_category` — the same
/// precedence chain (`resolve_task_policy`) the backend uses when actually
/// executing a task with no explicit hint, so the GUI's shown default can
/// never drift from what would actually happen. `category` is any string
/// `TaskCategory::from_str` accepts (case-insensitive, never errors — falls
/// back to `General`); `source` is a `TriggerSource::from_label` string
/// (`"interactive"`/`"automated"`/`"subagent"`/`"mesh"`, case-insensitive) —
/// an unrecognized `source` falls back to `Interactive` via the same
/// `unwrap_or(TriggerSource::Interactive)` pattern `resolved_policy()` uses
/// elsewhere, not a parse error.
#[tauri::command]
pub fn resolve_default_task_policy(category: String, source: String) -> DefaultTaskPolicyDto {
    use vox_orchestrator::mode::TriggerSource;
    use vox_orchestrator::types::TaskCategory;

    let overrides = vox_orchestrator::config::OrchestratorConfig::snapshot().task_policy;
    let category: TaskCategory = category.parse().unwrap_or_default();
    let source = TriggerSource::from_label(&source).unwrap_or(TriggerSource::Interactive);

    let (category_clutch, category_risk) =
        vox_orchestrator::mode::effective_category_policy(&overrides, category);
    let (source_clutch, source_risk) =
        vox_orchestrator::mode::effective_source_policy(&overrides, source);
    let (clutch, risk) = vox_orchestrator::mode::resolve_task_policy(
        None,
        None,
        category_clutch,
        category_risk,
        source_clutch,
        source_risk,
    );
    DefaultTaskPolicyDto {
        clutch: clutch_label(clutch).to_string(),
        risk: risk_label(risk).to_string(),
    }
}

#[cfg(test)]
mod default_policy_tests {
    use super::*;

    #[test]
    fn chat_default_matches_resolve_task_policy_with_no_overrides() {
        let dto = resolve_default_task_policy("Chat".to_string(), "interactive".to_string());
        // No overrides configured in a fresh test environment ⇒ falls all the
        // way to the global default, exactly like resolve_task_policy(None, None, None, None, None, None).
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }

    #[test]
    fn unknown_category_or_source_labels_fall_back_to_global_default() {
        // TaskCategory::from_str never errors (falls back to General for an
        // unrecognized string), so this exercises "recognized category with no
        // configured policy," not a parse failure — still must land on the
        // same global default since nothing is configured for General either.
        let dto = resolve_default_task_policy(
            "NotARealCategory".to_string(),
            "not_a_real_source".to_string(),
        );
        assert_eq!(dto.clutch, "balanced");
        assert_eq!(dto.risk, "moderate");
    }
}

/// Reject unparseable clutch/risk labels before touching Vox.toml. `None`
/// values are always accepted (that axis just isn't being set/changed).
fn validate_task_policy_labels(clutch: Option<&str>, risk: Option<&str>) -> Result<(), String> {
    if let Some(c) = clutch {
        vox_orchestrator::mode::ClutchProfile::from_label(c)
            .ok_or_else(|| format!("unknown clutch label: {c}"))?;
    }
    if let Some(r) = risk {
        vox_orchestrator::mode::RiskPosture::from_label(r)
            .ok_or_else(|| format!("unknown risk label: {r}"))?;
    }
    Ok(())
}

/// `scope_kind` is `"category"` or `"source"`; `scope_key` is a `TaskCategory`/
/// `TriggerSource` Debug name (e.g. `"CodeGen"`, `"Automated"`). Signals the
/// running orchestrator daemon to reload afterward (fire-and-forget), exactly
/// like `set_orchestrator_config` does — without this, the write only affects
/// what a future daemon restart picks up, not the live process.
#[tauri::command]
pub async fn set_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
    clutch: Option<String>,
    risk: Option<String>,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<(), String> {
    validate_task_policy_labels(clutch.as_deref(), risk.as_deref())?;

    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    let mut orch_table = manifest.orchestrator.unwrap_or_default();

    let mut task_policy_table = orch_table
        .get("task_policy")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut scope_table = task_policy_table
        .get(&scope_kind)
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    let mut entry = toml::map::Map::new();
    if let Some(c) = &clutch {
        entry.insert("clutch".to_string(), toml::Value::String(c.clone()));
    }
    if let Some(r) = &risk {
        entry.insert("risk".to_string(), toml::Value::String(r.clone()));
    }
    scope_table.insert(scope_key.clone(), toml::Value::Table(entry));
    task_policy_table.insert(scope_kind, toml::Value::Table(scope_table));
    orch_table.insert(
        "task_policy".to_string(),
        toml::Value::Table(task_policy_table),
    );

    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    vox_config::snapshot::bump(&["task_policy"]);
    let _ = app_handle.emit(
        ORCH_CONFIG_CHANGED_EVENT,
        OrchestratorConfigChanged {
            rev: vox_config::snapshot::current_rev(),
        },
    );

    // Fire-and-forget: tell the running daemon to reload so this override
    // affects real task execution immediately, not just after a restart.
    let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
    tokio::spawn(async move {
        let _ = call_orchestrator_daemon(
            &daemon,
            orch_daemon_method::RELOAD_CONFIG,
            serde_json::json!({}),
        )
        .await;
    });

    Ok(())
}

/// Remove one override (`scope_kind`/`scope_key` as in [`set_task_policy_override`]).
/// Same daemon-reload signal as `set_task_policy_override`.
#[tauri::command]
pub async fn clear_task_policy_override(
    app_handle: tauri::AppHandle,
    scope_kind: String,
    scope_key: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    let mut orch_table = manifest.orchestrator.unwrap_or_default();

    if let Some(mut task_policy_table) = orch_table
        .get("task_policy")
        .and_then(|v| v.as_table())
        .cloned()
    {
        if let Some(mut scope_table) = task_policy_table
            .get(&scope_kind)
            .and_then(|v| v.as_table())
            .cloned()
        {
            scope_table.remove(&scope_key);
            task_policy_table.insert(scope_kind, toml::Value::Table(scope_table));
            orch_table.insert(
                "task_policy".to_string(),
                toml::Value::Table(task_policy_table),
            );
        }
    }

    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;

    vox_config::snapshot::bump(&["task_policy"]);
    let _ = app_handle.emit(
        ORCH_CONFIG_CHANGED_EVENT,
        OrchestratorConfigChanged {
            rev: vox_config::snapshot::current_rev(),
        },
    );

    let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
    tokio::spawn(async move {
        let _ = call_orchestrator_daemon(
            &daemon,
            orch_daemon_method::RELOAD_CONFIG,
            serde_json::json!({}),
        )
        .await;
    });

    Ok(())
}

#[cfg(test)]
mod task_policy_tests {
    use super::*;

    #[test]
    fn get_task_policy_overrides_reflects_snapshot() {
        let overrides = get_task_policy_overrides();
        // Fresh default config has no overrides yet.
        assert!(overrides.category.is_empty());
        assert!(overrides.source.is_empty());
    }

    #[test]
    fn set_task_policy_override_rejects_unparseable_labels() {
        let result = validate_task_policy_labels(Some("turbo"), Some("high"));
        assert!(
            result.is_err(),
            "an unparseable clutch label must be rejected before writing Vox.toml"
        );
    }

    #[test]
    fn set_task_policy_override_accepts_valid_labels() {
        let result = validate_task_policy_labels(Some("free"), Some("high"));
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod catalog_tests {
    use vox_orchestrator::config::OrchestratorConfig;

    #[test]
    fn catalog_len_matches_config_field_count() {
        let catalog = OrchestratorConfig::default().to_catalog();
        // Exact parity test: catalog must have exactly 106 entries — one per
        // field! macro invocation in to_catalog(). Using an exact count rather
        // than a floor catches catalog shrinkage as well as unintentional growth.
        // If you intentionally add or remove fields, update this count to match.
        assert_eq!(
            catalog.len(),
            106,
            "catalog field count changed — update this test if fields were intentionally added/removed"
        );
    }

    #[test]
    fn catalog_keys_are_unique() {
        let catalog = OrchestratorConfig::default().to_catalog();
        let mut keys: Vec<&str> = catalog.iter().map(|f| f.key.as_str()).collect();
        keys.sort_unstable();
        let orig_len = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), orig_len, "catalog contains duplicate keys");
    }

    #[test]
    fn catalog_default_matches_config_default() {
        let catalog = OrchestratorConfig::default().to_catalog();
        let max_agents_field = catalog.iter().find(|f| f.key == "max_agents").unwrap();
        let default_cfg = OrchestratorConfig::default();
        let expected = serde_json::json!(default_cfg.max_agents);
        assert_eq!(
            max_agents_field.current_value, expected,
            "current_value for max_agents must match OrchestratorConfig::default().max_agents"
        );
        assert_eq!(
            max_agents_field.default_value, expected,
            "default_value for max_agents must match OrchestratorConfig::default().max_agents"
        );
    }
}

/// Read the effective orchestrator settings (Vox.toml + env overrides) via
/// [`OrchestratorConfig::snapshot`] so the GUI always reflects the live effective
/// value rather than only the Vox.toml defaults (fixes the inert-sliders bug).
#[tauri::command]
pub async fn get_orchestrator_config() -> Result<serde_json::Value, String> {
    let cfg = vox_orchestrator::config::OrchestratorConfig::snapshot();
    let isolation = match cfg.scope_enforcement {
        vox_orchestrator::scope::ScopeEnforcement::Strict => "strict",
        vox_orchestrator::scope::ScopeEnforcement::Warn => "warn",
        vox_orchestrator::scope::ScopeEnforcement::Disabled => "disabled",
    };
    Ok(serde_json::json!({
        "concurrency": cfg.max_agents,
        "capUsd": cfg.financial_cost_budget_micros as f64 / 1_000_000.0,
        "doubtThresh": cfg.trust_auto_approve_min,
        "isolation": isolation,
        "autobudget": cfg.exec_time_budget_enabled,
        "doubt": cfg.socrates_gate_enforce,
        "scalingEnabled": cfg.scaling_enabled,
        "minAgents": cfg.min_agents,
        "scalingThreshold": cfg.scaling_threshold,
        "scaleCpuCeilingPct": cfg.scale_cpu_ceiling_pct,
        "scaleMemFloorMb": cfg.scale_mem_floor_mb,
    }))
}

/// Token budget snapshot returned to the frontend.
#[derive(Debug, serde::Serialize)]
pub struct ContextBudgetPayload {
    /// Maximum tokens the model's context can hold (from `CompactionConfig`).
    pub max_context_tokens: usize,
    /// Tokens reserved for the model's response (subtracted from usable budget).
    pub reserved_tokens: usize,
    /// Token count at which compaction triggers (`max * compaction_threshold`).
    pub threshold_tokens: usize,
    /// Usable token budget (`max - reserved`).
    pub usable_tokens: usize,
    /// Human-readable compaction strategy name: "aggressive", "balanced", or "conservative".
    pub strategy: String,
    /// Tokens currently occupying the context window for the given session:
    /// input + output of the most recent llm_interactions row. Zero when no
    /// session is provided or no rows exist.
    pub used_tokens: usize,
}

/// Return the active context-window budget from the current compaction config.
///
/// Reads directly from the local in-memory config snapshot. When `session_id`
/// is provided, also queries `llm_interactions` for the latest input token count
/// so the frontend can render a real context-window fill percentage.
#[tauri::command]
pub async fn get_context_budget(
    session_id: Option<String>,
) -> Result<ContextBudgetPayload, String> {
    let cfg = vox_orchestrator::config::OrchestratorConfig::snapshot().compaction;

    let used_tokens: usize = if let Some(sid) = session_id.as_deref() {
        match vox_db::VoxDb::connect_canonical().await {
            Ok(db) => {
                // Most recent interaction only: input + output of the latest call is
                // what currently occupies the context window. A SUM over all rows would
                // grow unbounded across turns and peg the meter at 100% — dishonest.
                let rows = db
                    .query_all(
                        "SELECT COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) \
                         FROM llm_interactions \
                         WHERE session_id = ?1 \
                         ORDER BY rowid DESC LIMIT 1",
                        turso::params![sid],
                    )
                    .await
                    .unwrap_or_default();
                rows.into_iter()
                    .next()
                    .and_then(|row| row.get::<i64>(0).ok())
                    .map(|n| n.max(0) as usize)
                    .unwrap_or(0)
            }
            Err(_) => 0,
        }
    } else {
        0
    };

    Ok(ContextBudgetPayload {
        max_context_tokens: cfg.max_context_tokens,
        reserved_tokens: cfg.reserved_tokens,
        threshold_tokens: cfg.trigger_at(),
        usable_tokens: cfg.usable_budget(),
        strategy: cfg.strategy.to_string(),
        used_tokens,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct HopperTaskDto {
    pub item_id: String,
    pub intent: String,
    pub priority: u8,
    pub state: String,
    pub task_id: u64,
    /// Chat/CLI session the item was submitted from (persisted column).
    pub session_id: Option<String>,
    /// Agent bound to the item while `state == "assigned"` (from state JSON).
    pub agent_id: Option<String>,
    /// Origin daemon for mesh-replicated items (from source JSON).
    pub remote_node: Option<String>,
}

fn hopper_item_to_dto(item: &vox_orchestrator::hopper::IntakeItem) -> HopperTaskDto {
    let agent_id = match &item.state {
        vox_orchestrator::hopper::ItemState::Assigned { agent_id } => Some(agent_id.clone()),
        _ => None,
    };
    let remote_node = match &item.source {
        vox_orchestrator::hopper::IntakeSource::Mesh { node_id } => Some(node_id.clone()),
        _ => None,
    };
    HopperTaskDto {
        item_id: item.item_id.0.clone(),
        intent: item.intent.clone(),
        priority: item.classified_priority as u8,
        state: item.state.kind().to_string(),
        task_id: vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0),
        session_id: item.session_id.clone(),
        agent_id,
        remote_node,
    }
}

/// Most-recent bound on completed items chained into `hopper_list` (F7): the
/// command is re-polled on every tasks-changed event, so the done read must
/// not grow with all-time history. Pinned by
/// `done_history_limit_is_the_agreed_bound` below and exercised by
/// `history_recent_is_bounded_and_newest_first` in sqlite_store.rs.
const DONE_HISTORY_LIMIT: u32 = 50;

#[tauri::command]
pub async fn hopper_list() -> Result<Vec<HopperTaskDto>, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let inbox = hopper.inbox().await;
    let assigned = hopper.assigned().await;
    // Bounded, newest-first; the recent-window read is already scoped to
    // `done` at the SQL layer (ops_orchestrator::hopper_history_list_recent),
    // this filter is defense-in-depth against that changing silently.
    let done: Vec<_> = hopper
        .history_recent(DONE_HISTORY_LIMIT)
        .await
        .into_iter()
        .filter(|i| matches!(i.state, vox_orchestrator::hopper::ItemState::Done))
        .collect();
    let mut all = Vec::new();
    for item in inbox.iter().chain(assigned.iter()).chain(done.iter()) {
        all.push(hopper_item_to_dto(item));
    }
    Ok(all)
}

#[tauri::command]
pub async fn hopper_submit(
    app_handle: tauri::AppHandle,
    intent: String,
    affinity: Vec<String>,
) -> Result<HopperTaskDto, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let item = hopper
        .submit(
            intent,
            affinity,
            vox_orchestrator::hopper::PriorityHint::Normal,
            vox_orchestrator::hopper::IntakeSource::Developer,
            None,
        )
        .await;
    emit_tasks_changed(&app_handle);
    Ok(hopper_item_to_dto(&item))
}

#[tauri::command]
pub async fn hopper_reprioritize(
    app_handle: tauri::AppHandle,
    item_id: String,
    priority: u8,
) -> Result<HopperTaskDto, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let hid = vox_orchestrator::hopper::HopperItemId(item_id);
    let new_priority = vox_orchestrator::types::TaskPriority::from_u8(priority);
    let cap = vox_orchestrator::hopper::capability::DeveloperOverrideMint::new().mint(
        "Tauri GUI",
        "GUI Reprioritization",
        "gui-override",
    );
    let item = hopper
        .reprioritize(&hid, new_priority, cap)
        .await
        .map_err(|e| e.to_string())?;
    emit_tasks_changed(&app_handle);
    Ok(hopper_item_to_dto(&item))
}

#[tauri::command]
pub async fn hopper_cancel(
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<HopperTaskDto, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let hid = vox_orchestrator::hopper::HopperItemId(item_id);
    let item = hopper.cancel(&hid).await.map_err(|e| e.to_string())?;
    emit_tasks_changed(&app_handle);
    Ok(hopper_item_to_dto(&item))
}

#[tauri::command]
pub async fn hopper_mark_done(
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<HopperTaskDto, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let hid = vox_orchestrator::hopper::HopperItemId(item_id);
    let item = hopper.complete(&hid).await.map_err(|e| e.to_string())?;
    emit_tasks_changed(&app_handle);
    Ok(hopper_item_to_dto(&item))
}

#[cfg(test)]
mod hopper_tests {
    use super::*;
    use vox_orchestrator::hopper::IntakeItem;
    use vox_orchestrator::hopper::types::{IntakeSource, PriorityHint};

    #[test]
    fn test_hopper_item_to_dto() {
        let item = IntakeItem::new(
            "test intent".to_string(),
            vec![],
            PriorityHint::Normal,
            IntakeSource::Developer,
            None,
        );
        let dto = hopper_item_to_dto(&item);
        assert_eq!(dto.item_id, item.item_id.0);
        assert_eq!(dto.intent, "test intent");
        assert_eq!(dto.state, "inbox");
        assert_eq!(
            dto.task_id,
            vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0)
        );
    }

    #[test]
    fn hopper_dto_carries_persisted_session_agent_and_mesh_fields() {
        use vox_orchestrator::hopper::types::ItemState;
        let mut item = IntakeItem::new(
            "wired intent".to_string(),
            vec![],
            PriorityHint::Normal,
            IntakeSource::Mesh {
                node_id: "did:vox:peer-1".into(),
            },
            Some("gui-session-9".to_string()),
        );
        item.state = ItemState::Assigned {
            agent_id: "agent-42".into(),
        };
        let dto = hopper_item_to_dto(&item);
        assert_eq!(dto.session_id.as_deref(), Some("gui-session-9"));
        assert_eq!(dto.agent_id.as_deref(), Some("agent-42"));
        assert_eq!(dto.remote_node.as_deref(), Some("did:vox:peer-1"));
        // Inbox developer items carry none of the three.
        let plain = IntakeItem::new(
            "p".into(),
            vec![],
            PriorityHint::Normal,
            IntakeSource::Developer,
            None,
        );
        let dto2 = hopper_item_to_dto(&plain);
        assert!(dto2.session_id.is_none() && dto2.agent_id.is_none() && dto2.remote_node.is_none());
    }

    #[test]
    fn done_history_limit_is_the_agreed_bound() {
        // Spec Phase 2 item 6 records "bounded, most-recent-N" for the done
        // read; changing N is a product decision, not a drive-by.
        assert_eq!(DONE_HISTORY_LIMIT, 50);
    }

    #[test]
    fn task_priority_wire_values_match_frontend_constants() {
        // Mirror of crates/vox-gui/ui/src/lib/taskPriority.ts (TASK_PRIORITY_WIRE).
        // If either side changes, both tests must be updated together.
        use vox_orchestrator::types::TaskPriority;
        assert_eq!(TaskPriority::Background as u8, 0);
        assert_eq!(TaskPriority::Normal as u8, 1);
        assert_eq!(TaskPriority::Urgent as u8, 2);
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn context_budget_payload_serializes() {
        let payload = ContextBudgetPayload {
            max_context_tokens: 128_000,
            reserved_tokens: 10_000,
            threshold_tokens: 102_400,
            usable_tokens: 118_000,
            strategy: "balanced".to_string(),
            used_tokens: 0,
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["max_context_tokens"], 128_000);
        assert_eq!(json["strategy"], "balanced");
        assert_eq!(json["threshold_tokens"], 102_400);
    }

    #[test]
    fn threshold_tokens_matches_trigger_at() {
        // CompactionConfig::trigger_at() = max * threshold_fraction
        // Default: 128_000 * 0.80 = 102_400
        let cfg = vox_orchestrator::compaction::CompactionConfig::default();
        assert_eq!(cfg.trigger_at(), 102_400);
        assert_eq!(cfg.usable_budget(), 118_000);
    }
}

#[cfg(test)]
mod secretary_tests {
    use super::*;

    #[test]
    fn secretary_proposed_payload_serializes() {
        let payload = SecretaryProposedPayload {
            item_id: "abc123".to_string(),
            intent: "Fix the auth bug in login module".to_string(),
            confidence_pct: 85,
            session_id: "session-1".to_string(),
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["item_id"], "abc123");
        assert_eq!(json["confidence_pct"], 85);
        assert_eq!(json["session_id"], "session-1");
    }
}

#[cfg(test)]
mod gui_status_tests {
    use super::*;

    #[test]
    fn gui_status_passes_through_attention_budget() {
        let raw = serde_json::json!({
            "agent_count": 0, "total_queued": 0, "total_in_progress": 0,
            "total_completed": 0, "total_doubted": 0,
            "attention_budget": { "max_attention_ms": 3_600_000, "spent_ms": 1_800_000,
                "total_requests": 0, "auto_approved": 0, "rejected": 0,
                "interrupt_freq_per_hour": 9.0, "last_interrupt_ms": 0, "inbox_suppressed_count": 0 }
        });
        let gui = to_gui_status(raw);
        let ab = gui.attention_budget.expect("budget passed through");
        assert_eq!(ab["spent_ms"], 1_800_000);
    }
}
