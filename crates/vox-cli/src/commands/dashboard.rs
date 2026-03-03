use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::{self, Stream};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use tower_http::cors::CorsLayer;
use vox_orchestrator::{AgentId, Orchestrator, OrchestratorConfig, TaskId};

#[derive(RustEmbed)]
#[folder = "../../docs/dashboard/"]
struct Assets;

pub(crate) struct AppState {
    orchestrator: Mutex<Orchestrator>,
    #[allow(dead_code)]
    db: Option<Arc<vox_db::VoxDb>>,
    /// Recent scope violations and safety-related events for Trust and Safety panel (max 100).
    safety_events: Arc<Mutex<std::collections::VecDeque<SafetyEvent>>>,
}

#[derive(Clone, Serialize)]
struct SafetyEvent {
    kind: String,
    agent_id: u64,
    path: String,
    reason: String,
    timestamp_ms: u64,
}

fn lock_orchestrator(
    state: &AppState,
) -> Result<MutexGuard<'_, Orchestrator>, (StatusCode, &'static str)> {
    state.orchestrator.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Orchestrator lock poisoned",
        )
    })
}

/// Build the dashboard Router with all API routes and static fallback. Used by run() and tests.
pub(crate) fn dashboard_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/agents", get(list_agents))
        .route("/api/tasks", get(list_tasks))
        .route("/api/tasks/{id}/trace", get(get_task_trace))
        .route("/api/events", get(list_events))
        .route("/api/costs", get(get_costs))
        .route("/api/locks", get(get_locks))
        .route("/api/status", get(get_system_status))
        .route("/api/safety", get(get_safety))
        .route("/api/a2a/history", get(get_a2a_history))
        .route("/api/stream", get(event_stream))
        .route("/api/continue", post(trigger_continue))
        .route("/api/assess", post(trigger_assess))
        .route("/api/rebalance", post(trigger_rebalance))
        .route("/api/pause/{id}", post(pause_agent))
        .route("/api/resume/{id}", post(resume_agent))
        .route("/api/tune", post(tune_formula))
        .route("/api/skills", get(list_skills))
        .route("/api/skills/install", post(install_skill))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run() -> Result<()> {
    let config = load_config();
    let orchestrator = Orchestrator::new(config);
    let event_bus = orchestrator.event_bus().clone();

    let db_config = vox_db::DbConfig::from_env().unwrap_or_else(|_| vox_db::DbConfig::Local {
        path: "vox.db".to_string(),
    });

    let db = match vox_db::VoxDb::connect(db_config).await {
        Ok(db) => {
            let db_arc = Arc::new(db);
            let db_for_task = db_arc.clone();
            let mut rx = event_bus.subscribe();

            // Stream orchestrator events to gamify database
            tokio::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let (agent_id, event_type): (u64, &str) = match &event.kind {
                        vox_orchestrator::AgentEventKind::AgentSpawned { agent_id, .. } => {
                            (agent_id.0, "AgentSpawned")
                        }
                        vox_orchestrator::AgentEventKind::AgentRetired { agent_id } => {
                            (agent_id.0, "AgentRetired")
                        }
                        vox_orchestrator::AgentEventKind::ActivityChanged { agent_id, .. } => {
                            (agent_id.0, "ActivityChanged")
                        }
                        vox_orchestrator::AgentEventKind::TaskSubmitted { agent_id, .. } => {
                            (agent_id.0, "TaskSubmitted")
                        }
                        vox_orchestrator::AgentEventKind::TaskStarted { agent_id, .. } => {
                            (agent_id.0, "TaskStarted")
                        }
                        vox_orchestrator::AgentEventKind::TaskCompleted { agent_id, .. } => {
                            (agent_id.0, "TaskCompleted")
                        }
                        vox_orchestrator::AgentEventKind::TaskFailed { agent_id, .. } => {
                            (agent_id.0, "TaskFailed")
                        }
                        vox_orchestrator::AgentEventKind::LockAcquired { agent_id, .. } => {
                            (agent_id.0, "LockAcquired")
                        }
                        vox_orchestrator::AgentEventKind::LockReleased { agent_id, .. } => {
                            (agent_id.0, "LockReleased")
                        }
                        vox_orchestrator::AgentEventKind::AgentIdle { agent_id } => {
                            (agent_id.0, "AgentIdle")
                        }
                        vox_orchestrator::AgentEventKind::AgentBusy { agent_id } => {
                            (agent_id.0, "AgentBusy")
                        }
                        vox_orchestrator::AgentEventKind::MessageSent { from, .. } => {
                            (from.0, "MessageSent")
                        }
                        vox_orchestrator::AgentEventKind::CostIncurred { agent_id, .. } => {
                            (agent_id.0, "CostIncurred")
                        }
                        vox_orchestrator::AgentEventKind::ContinuationTriggered {
                            agent_id,
                            ..
                        } => (agent_id.0, "ContinuationTriggered"),
                        vox_orchestrator::AgentEventKind::PlanHandoff { from, .. } => {
                            (from.0, "PlanHandoff")
                        }
                        vox_orchestrator::AgentEventKind::ScopeViolation { agent_id, .. } => {
                            (agent_id.0, "ScopeViolation")
                        }
                        vox_orchestrator::AgentEventKind::PromptConflictDetected { .. } => {
                            (0, "PromptConflictDetected")
                        }
                        vox_orchestrator::AgentEventKind::InjectionDetected { .. } => {
                            (0, "InjectionDetected")
                        }
                        vox_orchestrator::AgentEventKind::CompactionTriggered {
                            agent_id, ..
                        } => (agent_id.0, "CompactionTriggered"),
                        vox_orchestrator::AgentEventKind::MemoryFlushed { agent_id, .. } => {
                            (agent_id.0, "MemoryFlushed")
                        }
                        vox_orchestrator::AgentEventKind::SessionCreated { agent_id, .. } => {
                            (agent_id.0, "SessionCreated")
                        }
                        vox_orchestrator::AgentEventKind::SessionReset { agent_id, .. } => {
                            (agent_id.0, "SessionReset")
                        }
                        vox_orchestrator::AgentEventKind::WorkflowStarted { .. } => {
                            (0, "WorkflowStarted")
                        }
                        vox_orchestrator::AgentEventKind::WorkflowCompleted { .. } => {
                            (0, "WorkflowCompleted")
                        }
                        vox_orchestrator::AgentEventKind::WorkflowFailed { .. } => {
                            (0, "WorkflowFailed")
                        }
                        vox_orchestrator::AgentEventKind::ActivityStarted { .. } => {
                            (0, "ActivityStarted")
                        }
                        vox_orchestrator::AgentEventKind::ActivityCompleted { .. } => {
                            (0, "ActivityCompleted")
                        }
                        vox_orchestrator::AgentEventKind::ActivityRetried { .. } => {
                            (0, "ActivityRetried")
                        }
                        // JJ-inspired VCS events
                        vox_orchestrator::AgentEventKind::SnapshotCaptured { agent_id, .. } => {
                            (agent_id.0, "SnapshotCaptured")
                        }
                        vox_orchestrator::AgentEventKind::OperationUndone { agent_id, .. } => {
                            (agent_id.0, "OperationUndone")
                        }
                        vox_orchestrator::AgentEventKind::ConflictDetected {
                            agent_ids, ..
                        } => (
                            agent_ids.first().map(|a| a.0).unwrap_or(0),
                            "ConflictDetected",
                        ),
                        vox_orchestrator::AgentEventKind::ConflictResolved { .. } => {
                            (0, "ConflictResolved")
                        }
                        vox_orchestrator::AgentEventKind::WorkspaceCreated { agent_id, .. } => {
                            (agent_id.0, "WorkspaceCreated")
                        }
                        vox_orchestrator::AgentEventKind::UrgentRebalanceTriggered { .. } => {
                            (0, "UrgentRebalanceTriggered")
                        }
                        _ => (0, "Other"),
                    };

                    let payload = serde_json::to_string(&event.kind).unwrap_or_default();
                    let _ = vox_gamify::db::insert_event(
                        &db_for_task,
                        &agent_id.to_string(),
                        event_type,
                        Some(&payload),
                    )
                    .await;

                    // Process rewards
                    let kind_json = serde_json::to_value(&event.kind).unwrap_or_default();
                    let _ = vox_gamify::db::process_event_rewards(
                        &db_for_task,
                        vox_gamify::util::DEFAULT_USER_ID,
                        &kind_json,
                    )
                    .await;
                }
            });
            Some(db_arc)
        }
        Err(e) => {
            tracing::warn!("failed to connect to database in dashboard: {}", e);
            None
        }
    };

    let safety_events = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let safety_rx = event_bus.subscribe();
    let safety_events_clone = safety_events.clone();
    tokio::spawn(async move {
        let mut recv = safety_rx;
        while let Ok(ev) = recv.recv().await {
            let entry = match &ev.kind {
                vox_orchestrator::AgentEventKind::ScopeViolation {
                    agent_id,
                    path,
                    reason,
                } => Some(SafetyEvent {
                    kind: "scope_violation".to_string(),
                    agent_id: agent_id.0,
                    path: path.display().to_string(),
                    reason: reason.clone(),
                    timestamp_ms: ev.timestamp_ms,
                }),
                vox_orchestrator::AgentEventKind::PromptConflictDetected { warnings, .. } => {
                    Some(SafetyEvent {
                        kind: "prompt_conflict".to_string(),
                        agent_id: 0,
                        path: String::new(),
                        reason: format!("Prompt conflict: {}", warnings.join("; ")),
                        timestamp_ms: ev.timestamp_ms,
                    })
                }
                vox_orchestrator::AgentEventKind::InjectionDetected { detail } => {
                    Some(SafetyEvent {
                        kind: "injection".to_string(),
                        agent_id: 0,
                        path: String::new(),
                        reason: format!("Injection detected: {}", detail),
                        timestamp_ms: ev.timestamp_ms,
                    })
                }
                _ => None,
            };
            if let Some(entry) = entry {
                if let Ok(mut g) = safety_events_clone.lock() {
                    g.push_back(entry);
                    while g.len() > 100 {
                        g.pop_front();
                    }
                }
            }
        }
    });

    let state = Arc::new(AppState {
        orchestrator: Mutex::new(orchestrator),
        db,
        safety_events,
    });

    let app = dashboard_app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3847));
    println!("🚀 Vox Dashboard running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn static_handler(req: axum::extract::Request) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let body = axum::body::Body::from(content.data);
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            axum::response::Response::builder()
                .header("content-type", mime.as_ref())
                .body(body)
                .expect("valid response")
        }
        None => {
            // Fallback to index.html for SPA-like behavior if needed, or 404
            match Assets::get("index.html") {
                Some(content) => axum::response::Response::builder()
                    .header("content-type", "text/html")
                    .body(axum::body::Body::from(content.data))
                    .expect("valid response"),
                None => axum::response::Response::builder()
                    .status(axum::http::StatusCode::NOT_FOUND)
                    .body(axum::body::Body::from("404 Not Found"))
                    .expect("valid response"),
            }
        }
    }
}

// ── Handlers ──────────────────────────────────────────────

#[derive(Serialize)]
struct AgentJson {
    id: u64,
    name: String,
    activity: String,
    queue_depth: usize,
    completed: usize,
    paused: bool,
    dynamic: bool,
    weighted_load: f64,
    urgent_count: usize,
    normal_count: usize,
    background_count: usize,
    cost: f64,
}

async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AgentJson>>, (StatusCode, &'static str)> {
    let orch = lock_orchestrator(&state)?;
    let status = orch.status();

    let agents = status
        .agents
        .into_iter()
        .map(|a| {
            let budget = orch.budget().check_budget(a.id);
            AgentJson {
                id: a.id.0,
                name: a.name,
                activity: if a.in_progress { "working" } else { "idle" }.to_string(),
                queue_depth: a.queued,
                completed: a.completed,
                paused: a.paused,
                dynamic: a.dynamic,
                weighted_load: a.weighted_load,
                urgent_count: a.urgent_count,
                normal_count: a.normal_count,
                background_count: a.background_count,
                cost: budget.map(|b| b.cost_usd).unwrap_or(0.0),
            }
        })
        .collect();

    Ok(Json(agents))
}

#[derive(Serialize)]
struct TaskJson {
    id: u64,
    description: String,
    agent_id: Option<u64>,
    status: String,
    priority: String,
}

async fn list_tasks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TaskJson>>, (StatusCode, &'static str)> {
    let orch = lock_orchestrator(&state)?;
    let mut tasks = Vec::new();

    for agent_id in orch.agent_ids() {
        if let Some(queue) = orch.agent_queue(agent_id) {
            // Pending/Current tasks
            if let Some(t) = queue.current_task() {
                tasks.push(TaskJson {
                    id: t.id.0,
                    description: t.description.clone(),
                    agent_id: Some(agent_id.0),
                    status: "active".to_string(),
                    priority: format!("{:?}", t.priority).to_lowercase(),
                });
            }
            // Queued tasks
            // Note: AgentQueue doesn't easily expose the full vec, so we might need to add a method
        }
    }

    Ok(Json(tasks))
}

async fn list_events(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, &'static str)> {
    if let Some(db) = &state.db {
        let mut all_events = Vec::new();

        let agent_ids = {
            let orch = lock_orchestrator(&state)?;
            orch.agent_ids()
        };
        for id in agent_ids {
            if let Ok(records) = vox_gamify::db::get_events(db, &id.0.to_string(), Some(50)).await {
                for r in records {
                    if let Some(p) = r.payload {
                        if let Ok(kind) =
                            serde_json::from_str::<vox_orchestrator::AgentEventKind>(&p)
                        {
                            // Parse SQLite datetime string "YYYY-MM-DD HH:MM:SS" to unix ms
                            let timestamp_ms = chrono::NaiveDateTime::parse_from_str(
                                &r.timestamp,
                                "%Y-%m-%d %H:%M:%S",
                            )
                            .map(|dt: chrono::NaiveDateTime| dt.and_utc().timestamp_millis() as u64)
                            .unwrap_or(0);

                            all_events.push(serde_json::json!({
                                "id": r.id,
                                "timestamp_ms": timestamp_ms,
                                "kind": kind
                            }));
                        }
                    }
                }
            }
        }

        // Sort by timestamp descending
        all_events.sort_by(|a, b| {
            let ts_a = a.get("timestamp_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            let ts_b = b.get("timestamp_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            ts_b.cmp(&ts_a)
        });

        Ok(Json(all_events))
    } else {
        Ok(Json(vec![]))
    }
}

#[derive(Serialize)]
struct CostSummary {
    total: f64,
    by_agent: std::collections::HashMap<u64, f64>,
}

async fn get_costs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CostSummary>, (StatusCode, &'static str)> {
    let orch = lock_orchestrator(&state)?;
    let mut total = 0.0;
    let mut by_agent = std::collections::HashMap::new();

    for id in orch.agent_ids() {
        if let Some(b) = orch.budget().check_budget(id) {
            total += b.cost_usd;
            by_agent.insert(id.0, b.cost_usd);
        }
    }

    Ok(Json(CostSummary { total, by_agent }))
}

#[derive(Serialize)]
struct SystemStatusJson {
    enabled: bool,
    agent_count: usize,
    reserved_agents: usize,
    dynamic_agents: usize,
    total_queued: usize,
    total_weighted_load: f64,
    predicted_load: f64,
    locked_files: usize,
    total_contention: usize,
    scaling_profile: String,
    effective_scale_up_threshold: f64,
    resource_cpu_multiplier: f64,
    resource_mem_multiplier: f64,
    resource_exponent: f64,
    resource_weight: f64,
}

async fn get_system_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SystemStatusJson>, (StatusCode, &'static str)> {
    let orch = lock_orchestrator(&state)?;
    let status = orch.status();

    let cfg = orch.config();
    let effective = cfg.scaling_threshold as f64 * cfg.scaling_profile.threshold_multiplier();
    Ok(Json(SystemStatusJson {
        enabled: status.enabled,
        agent_count: status.agent_count,
        reserved_agents: status.reserved_agents,
        dynamic_agents: status.dynamic_agents,
        total_queued: status.total_queued,
        total_weighted_load: status.total_weighted_load,
        predicted_load: status.predicted_load,
        locked_files: status.locked_files,
        total_contention: status.total_contention,
        scaling_profile: format!("{:?}", cfg.scaling_profile).to_lowercase(),
        effective_scale_up_threshold: effective,
        resource_cpu_multiplier: cfg.resource_cpu_multiplier,
        resource_mem_multiplier: cfg.resource_mem_multiplier,
        resource_exponent: cfg.resource_exponent,
        resource_weight: cfg.resource_weight,
    }))
}

#[derive(Serialize)]
struct LockInfo {
    path: String,
    agent_id: u64,
    exclusive: bool,
}

async fn get_locks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<LockInfo>>, (StatusCode, &'static str)> {
    let orch = lock_orchestrator(&state)?;
    let list = orch.lock_manager().list_locks();
    let out: Vec<LockInfo> = list
        .into_iter()
        .map(|(path, agent_id, exclusive)| LockInfo {
            path: path.to_string_lossy().into_owned(),
            agent_id: agent_id.0,
            exclusive,
        })
        .collect();
    Ok(Json(out))
}

async fn get_safety(State(state): State<Arc<AppState>>) -> Json<Vec<SafetyEvent>> {
    let g = state
        .safety_events
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    Json(g.iter().cloned().collect::<Vec<_>>())
}

async fn get_task_trace(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<vox_orchestrator::TaskTraceStep>>, StatusCode> {
    let orch = lock_orchestrator(&state).map_err(|(code, _)| code)?;
    match orch.task_trace(TaskId(id)) {
        Some(steps) => Ok(Json(steps.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn event_stream(
    State(state): State<Arc<AppState>>,
) -> Result<
    Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, &'static str),
> {
    let orch = lock_orchestrator(&state)?;
    let rx = orch.event_bus().subscribe();

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => {
                let sse_event =
                    Event::default().data(serde_json::to_string(&event).unwrap_or_default());
                Some((Ok(sse_event), rx))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ── Action Handlers ───────────────────────────────────────

async fn trigger_continue(
    State(state): State<Arc<AppState>>,
) -> Result<Json<bool>, (StatusCode, &'static str)> {
    let mut orch = lock_orchestrator(&state)?;
    orch.tick(); // Triggers idle check
    Ok(Json(true))
}

async fn trigger_assess(State(_state): State<Arc<AppState>>) -> Json<bool> {
    // Currently no direct assess-all; would need logic
    Json(true)
}

async fn trigger_rebalance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<usize>, (StatusCode, &'static str)> {
    let mut orch = lock_orchestrator(&state)?;
    Ok(Json(orch.rebalance()))
}

async fn pause_agent(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<bool>, (StatusCode, &'static str)> {
    let mut orch = lock_orchestrator(&state)?;
    let _ = orch.pause_agent(AgentId(id));
    Ok(Json(true))
}

async fn resume_agent(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<bool>, (StatusCode, &'static str)> {
    let mut orch = lock_orchestrator(&state)?;
    let _ = orch.resume_agent(AgentId(id));
    Ok(Json(true))
}

#[derive(serde::Deserialize)]
struct TuneFormulaRequest {
    resource_cpu_multiplier: Option<f64>,
    resource_mem_multiplier: Option<f64>,
    resource_exponent: Option<f64>,
    resource_weight: Option<f64>,
}

async fn tune_formula(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TuneFormulaRequest>,
) -> Result<Json<bool>, (StatusCode, &'static str)> {
    let mut orch = lock_orchestrator(&state)?;
    let cfg = orch.config_mut();
    if let Some(v) = payload.resource_cpu_multiplier {
        cfg.resource_cpu_multiplier = v;
    }
    if let Some(v) = payload.resource_mem_multiplier {
        cfg.resource_mem_multiplier = v;
    }
    if let Some(v) = payload.resource_exponent {
        cfg.resource_exponent = v;
    }
    if let Some(v) = payload.resource_weight {
        cfg.resource_weight = v;
    }
    Ok(Json(true))
}

fn load_config() -> OrchestratorConfig {
    let mut config = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            let toml_path = cwd.join("Vox.toml");
            OrchestratorConfig::load_from_toml(&toml_path).ok()
        })
        .unwrap_or_default();
    config.merge_env_overrides();
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    use vox_orchestrator::FileAffinity;

    #[tokio::test]
    async fn api_tasks_trace_returns_trace_steps() {
        let config = OrchestratorConfig::for_testing();
        let mut orch = Orchestrator::new(config);
        let task_id = orch
            .submit_task("test", vec![FileAffinity::write("a.rs")], None)
            .await
            .unwrap();
        let state = Arc::new(AppState {
            orchestrator: Mutex::new(orch),
            db: None,
            safety_events: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        });
        let app = dashboard_app(state);
        let req = Request::builder()
            .uri(format!("/api/tasks/{}/trace", task_id.0))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let steps: Vec<vox_orchestrator::TaskTraceStep> = serde_json::from_slice(&body).unwrap();
        assert!(!steps.is_empty());
        assert_eq!(steps[0].stage, "ingress");
    }

    #[tokio::test]
    async fn api_locks_returns_json_array() {
        let config = OrchestratorConfig::for_testing();
        let orch = Orchestrator::new(config);
        let state = Arc::new(AppState {
            orchestrator: Mutex::new(orch),
            db: None,
            safety_events: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        });
        let app = dashboard_app(state);
        let req = Request::builder()
            .uri("/api/locks")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let locks: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(locks.is_empty());
    }

    #[tokio::test]
    async fn api_status_returns_system_status() {
        let config = OrchestratorConfig::for_testing();
        let orch = Orchestrator::new(config);
        let state = Arc::new(AppState {
            orchestrator: Mutex::new(orch),
            db: None,
            safety_events: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        });
        let app = dashboard_app(state);
        let req = Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(status.get("enabled").is_some());
        assert!(status.get("agent_count").is_some());
        assert!(status.get("scaling_profile").is_some());
        assert!(status.get("effective_scale_up_threshold").is_some());
    }

    #[tokio::test]
    async fn api_safety_returns_json_array() {
        let config = OrchestratorConfig::for_testing();
        let orch = Orchestrator::new(config);
        let state = Arc::new(AppState {
            orchestrator: Mutex::new(orch),
            db: None,
            safety_events: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        });
        let app = dashboard_app(state);
        let req = Request::builder()
            .uri("/api/safety")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let events: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(events.is_empty());
    }
}
async fn list_skills(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB not available").into_response(),
    };

    match db.store().list_skill_manifests().await {
        Ok(skills) => (StatusCode::OK, Json(skills)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_a2a_history(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB not available").into_response(),
    };

    match vox_gamify::db::list_a2a_messages(db, 50).await {
        Ok(msgs) => (StatusCode::OK, Json(msgs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct InstallSkillRequest {
    skill_md: String,
}

async fn install_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallSkillRequest>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB not available").into_response(),
    };

    let bundle = match vox_skills::parser::parse_skill_md(&payload.skill_md) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    let manifest_json = serde_json::to_string(&bundle.manifest).unwrap_or_default();
    match db
        .store()
        .publish_skill(
            &bundle.manifest.id,
            &bundle.manifest.version,
            &manifest_json,
            &bundle.skill_md,
        )
        .await
    {
        Ok(_) => (StatusCode::OK, "Skill installed successfully".to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
