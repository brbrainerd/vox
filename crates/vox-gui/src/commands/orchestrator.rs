use std::sync::Arc;

use serde::Serialize;
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
// toestub-ignore(skeleton/untested-pub-api) — spawns a background task bridging the orchestrator daemon to Tauri events; covered by integration
pub fn spawn_orchestrator_status_stream(
    app_handle: tauri::AppHandle,
    daemon: Arc<PersistentDaemon>,
) {
    tokio::spawn(async move {
        let addr = match daemon.ensure().await {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!("daemon unavailable: {e}");
                return;
            }
        };
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<serde_json::Value>(crate::config::ORCH_STATUS_CHANNEL_CAP);

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
// toestub-ignore(skeleton/untested-pub-api) — spawns a background task bridging the orchestrator daemon to Tauri events; covered by integration
pub fn spawn_agent_event_stream(app_handle: tauri::AppHandle, daemon: Arc<PersistentDaemon>) {
    tokio::spawn(async move {
        let addr = match daemon.ensure().await {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!("daemon unavailable: {e}");
                return;
            }
        };
        let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(
            crate::config::AGENT_EVENTS_CHANNEL_CAP,
        );

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

/// Tauri event emitted when the secretary auto-submits a task from chat.
/// Payload: `SecretaryProposedPayload`.
pub const SECRETARY_PROPOSED_EVENT: &str = "vox://secretary-proposed-task";

/// Payload for the [`SECRETARY_PROPOSED_EVENT`] Tauri event.
#[derive(Debug, serde::Serialize, Clone)]
pub struct SecretaryProposedPayload {
    /// Hopper item ID assigned to the submitted task.
    pub item_id: String,
    /// Cleaned intent text that was submitted as the task description.
    pub intent: String,
    /// Classifier confidence 0–100 (for UI display only; not a guarantee).
    pub confidence_pct: u8,
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

async fn enrich_mesh_from_tool(gui: &mut GuiOrchestratorStatus) {
    let mesh = call_daemon(
        "vox-orchestrator-d",
        orch_daemon_method::TOOL_CALL,
        serde_json::json!({ "name": "vox_mesh_nodes", "args": {} }),
        false,
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
pub async fn get_orchestrator_status() -> Result<serde_json::Value, String> {
    let status = daemon_status().await?;
    let mut gui = to_gui_status(status);
    if gui.peers.is_empty() {
        enrich_mesh_from_tool(&mut gui).await;
    }
    gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;
    serde_json::to_value(gui).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_orchestrator_status_bin() -> Result<tauri::ipc::Response, String> {
    let status = daemon_status().await?;
    let mut gui = to_gui_status(status);
    if gui.peers.is_empty() {
        enrich_mesh_from_tool(&mut gui).await;
    }
    gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;
    let bytes = rmp_serde::to_vec_named(&gui).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn set_orchestrator_config(
    app_handle: tauri::AppHandle,
    config: serde_json::Value,
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
}

/// Return the active context-window budget from the current compaction config.
///
/// Reads directly from the local in-memory config snapshot.
#[tauri::command]
pub async fn get_context_budget() -> Result<ContextBudgetPayload, String> {
    let cfg = vox_orchestrator::config::OrchestratorConfig::snapshot().compaction;

    Ok(ContextBudgetPayload {
        max_context_tokens: cfg.max_context_tokens,
        reserved_tokens: cfg.reserved_tokens,
        threshold_tokens: cfg.trigger_at(),
        usable_tokens: cfg.usable_budget(),
        strategy: cfg.strategy.to_string(),
    })
}

#[derive(Debug, serde::Serialize)]
pub struct HopperTaskDto {
    pub item_id: String,
    pub intent: String,
    pub priority: u8,
    pub state: String,
    pub task_id: u64,
}

fn hopper_item_to_dto(item: &vox_orchestrator::hopper::IntakeItem) -> HopperTaskDto {
    HopperTaskDto {
        item_id: item.item_id.0.clone(),
        intent: item.intent.clone(),
        priority: item.classified_priority as u8,
        state: item.state.kind().to_string(),
        task_id: vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0),
    }
}

#[tauri::command]
pub async fn hopper_list() -> Result<Vec<HopperTaskDto>, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let inbox = hopper.inbox().await;
    let assigned = hopper.assigned().await;
    let mut all = Vec::new();
    for item in inbox.iter().chain(assigned.iter()) {
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
        assert_eq!(dto.task_id, vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0));
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
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["item_id"], "abc123");
        assert_eq!(json["confidence_pct"], 85);
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
