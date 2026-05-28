//! Dashboard `/api/v2/*` handlers that need [`super::GatewayState`] (orchestrator + workspace).
//!
//! - **Mesh nodes:** live [`Orchestrator::topology_snapshot`](vox_orchestrator::Orchestrator::topology_snapshot)
//!   when `VOX_DASHBOARD_LIVE_MESH=1`, else deterministic fixture JSON (envelope-compatible).
//! - **Runs:** live task list from [`Orchestrator::all_tasks`](vox_orchestrator::Orchestrator::all_tasks)
//!   when `VOX_DASHBOARD_LIVE_RUNS=1`, else fixture.
//! - **Layout:** file-backed `dashboard_layout.v1` under `.vox/dashboard/layout.json` when workspace known.
//!   `PUT` requires a **write** bearer (same rules as `/v1/tools/call`) when authentication is enabled.
//! - **Routing viz:** read-only arm stats from the model registry when `VOX_DASHBOARD_ROUTING_VIZ=1`.

use axum::Json;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;

use super::{AccessRole, GatewayState, enforce_auth, enforce_https_requirement, enforce_rate_limit, request_identity, resolve_access_role};
use crate::services::routes::{err, ok};
use crate::sync_poison::poison_rw_read;

type GuardResult = std::result::Result<(), Json<Value>>;

fn enforce_dashboard_read(gs: &GatewayState, peer: &SocketAddr, headers: &HeaderMap) -> GuardResult {
    if let Err(e) = enforce_auth(gs, headers, Some(peer)) {
        return Err(err("unauthorized", &e));
    }
    if let Err(e) = enforce_https_requirement(gs, headers) {
        return Err(err("forbidden", &e));
    }
    let identity = request_identity(gs, peer, headers);
    if let Err(e) = enforce_rate_limit(gs, &identity) {
        return Err(err("rate_limited", &e));
    }
    Ok(())
}

fn enforce_dashboard_write(gs: &GatewayState, peer: &SocketAddr, headers: &HeaderMap) -> GuardResult {
    enforce_dashboard_read(gs, peer, headers)?;
    match resolve_access_role(gs, headers, Some(peer)) {
        Ok(AccessRole::Write) => Ok(()),
        Ok(AccessRole::Read) => Err(err(
            "forbidden",
            "write-capable bearer token required for this operation",
        )),
        Err(e) => Err(err("unauthorized", &e)),
    }
}

fn live_mesh_enabled() -> bool {
    matches!(
        std::env::var("VOX_DASHBOARD_LIVE_MESH")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true")),
        Ok(true)
    )
}

fn routing_viz_enabled() -> bool {
    matches!(
        std::env::var("VOX_DASHBOARD_ROUTING_VIZ")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true")),
        Ok(true)
    )
}

fn live_runs_enabled() -> bool {
    matches!(
        std::env::var("VOX_DASHBOARD_LIVE_RUNS")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true")),
        Ok(true)
    )
}

fn confidence_state_for_model(m: &vox_orchestrator::models::ModelSpec) -> &'static str {
    let pins = vox_config::load_model_pins_config().unwrap_or_default();
    if pins.retired_ids.iter().any(|id| id == &m.id) {
        return "deprecated";
    }
    match m.pricing_source {
        vox_orchestrator::models::spec::PricingSource::Telemetry
        | vox_orchestrator::models::spec::PricingSource::UserConfig => "confirmed",
        vox_orchestrator::models::spec::PricingSource::Unknown => "provisional",
        vox_orchestrator::models::spec::PricingSource::LiteLLM
        | vox_orchestrator::models::spec::PricingSource::OpenRouter
        | vox_orchestrator::models::spec::PricingSource::AnthropicDirect
        | vox_orchestrator::models::spec::PricingSource::Bootstrap => "shadowed",
    }
}

fn fixture_mesh_nodes() -> Value {
    json!({
        "source": "fixture",
        "nodes": [
            { "id": "stub-1", "name": "stub-worker-1", "status": "idle", "role": "worker" },
            { "id": "stub-2", "name": "stub-worker-2", "status": "idle", "role": "worker" }
        ],
        "hint": "Set VOX_DASHBOARD_LIVE_MESH=1 for orchestrator topology_snapshot()"
    })
}

fn fixture_runs() -> Value {
    json!({
        "source": "fixture",
        "runs": [],
        "hint": "Set VOX_DASHBOARD_LIVE_RUNS=1 for orchestrator all_tasks() snapshot"
    })
}

pub async fn get_mesh_nodes(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    if live_mesh_enabled() {
        let snap = gs.server_state.orchestrator.topology_snapshot();
        let nodes = serde_json::to_value(&snap.nodes).unwrap_or(json!([]));
        let edges = serde_json::to_value(&snap.delegation_edges).unwrap_or(json!([]));
        let gaps = serde_json::to_value(&snap.known_gaps).unwrap_or(json!([]));
        return ok(json!({
            "source": "orchestrator",
            "generated_at_ms": snap.generated_at_ms,
            "nodes": nodes,
            "edges": edges,
            "known_gaps": gaps,
        }));
    }
    ok(fixture_mesh_nodes())
}

pub async fn get_runs_recent(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    if !live_runs_enabled() {
        return ok(fixture_runs());
    }
    let tasks = gs.server_state.orchestrator.all_tasks();
    let mut slim: Vec<Value> = Vec::new();
    for t in tasks.into_iter().take(48) {
        slim.push(json!({
            "id": t.id.0,
            "description": t.description,
            "status": t.status,
            "priority": t.priority,
            "task_category": t.task_category,
            "model_override": t.model_override,
            "model_preference": t.model_preference,
        }));
    }
    ok(json!({
        "source": "orchestrator",
        "run_count": slim.len(),
        "runs": slim,
    }))
}

fn layout_path(gs: &GatewayState) -> Option<PathBuf> {
    let root = gs.server_state.workspace_root.as_ref()?;
    Some(root.join(".vox").join("dashboard").join("layout.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayoutV1 {
    pub version: u32,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub items: Vec<LayoutItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutItemV1 {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default)]
    pub props: Value,
}

fn default_classic_layout() -> DashboardLayoutV1 {
    DashboardLayoutV1 {
        version: 1,
        workspace_id: None,
        items: vec![
            LayoutItemV1 {
                id: "card-speak".into(),
                type_: "speak".into(),
                x: 0,
                y: 0,
                w: 6,
                h: 4,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-mesh".into(),
                type_: "mesh".into(),
                x: 6,
                y: 0,
                w: 6,
                h: 4,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-models".into(),
                type_: "models".into(),
                x: 0,
                y: 4,
                w: 4,
                h: 3,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-runs".into(),
                type_: "runs".into(),
                x: 4,
                y: 4,
                w: 4,
                h: 3,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-forge".into(),
                type_: "forge".into(),
                x: 8,
                y: 4,
                w: 4,
                h: 3,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-code".into(),
                type_: "code".into(),
                x: 0,
                y: 7,
                w: 6,
                h: 4,
                props: json!({}),
            },
            LayoutItemV1 {
                id: "card-settings".into(),
                type_: "settings".into(),
                x: 6,
                y: 7,
                w: 6,
                h: 4,
                props: json!({}),
            },
        ],
    }
}

pub async fn get_dashboard_layout(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    let Some(path) = layout_path(&gs) else {
        return ok(json!({ "layout": default_classic_layout(), "persisted": false }));
    };
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(layout) = serde_json::from_slice::<DashboardLayoutV1>(&bytes) {
            return ok(json!({ "layout": layout, "persisted": true, "path": path.display().to_string() }));
        }
    }
    ok(json!({ "layout": default_classic_layout(), "persisted": false, "path": path.display().to_string() }))
}

#[derive(Debug, Deserialize)]
pub struct PutLayoutBody {
    pub layout: DashboardLayoutV1,
}

pub async fn put_dashboard_layout(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<PutLayoutBody>,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_write(&gs, &connect.0, &headers) {
        return e;
    }
    if body.layout.version != 1 {
        return err("bad_version", "layout.version must be 1");
    }
    let Some(path) = layout_path(&gs) else {
        return err("no_workspace", "workspace_root is not set; cannot persist layout");
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return err("io", &format!("create_dir_all: {e}"));
        }
    }
    let bytes = match serde_json::to_vec_pretty(&body.layout) {
        Ok(b) => b,
        Err(e) => return err("serialize", &e.to_string()),
    };
    if let Err(e) = std::fs::write(&path, bytes) {
        return err("io", &format!("write: {e}"));
    }
    ok(json!({ "ok": true, "path": path.display().to_string() }))
}

pub async fn get_routing_summary(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    if !routing_viz_enabled() {
        return ok(json!({
            "source": "disabled",
            "hint": "Set VOX_DASHBOARD_ROUTING_VIZ=1 for arm_stats snapshot from ModelRegistry"
        }));
    }
    let models = gs.server_state.orchestrator.models_handle();
    let (arms, decision) = match poison_rw_read(models.read(), "model registry for routing viz") {
        Ok(guard) => {
            let arms = guard.arm_stats_snapshot().clone();
            let req = vox_orchestrator::models::ModelSelectionRequest::from_intent(
                vox_orchestrator::models::SelectionIntent::for_task(
                    vox_orchestrator::types::TaskCategory::CodeGen,
                ),
            );
            let decision = vox_orchestrator::models::decide(&req, &guard);
            (arms, decision)
        }
        Err(e) => return err("lock", &e.to_string()),
    };
    let arms_json: Value = serde_json::to_value(&arms).unwrap_or(json!({}));
    let decision_json = decision.map(|d| {
        json!({
            "selected_model": d.selected_model,
            "provider_route": format!("{:?}", d.provider_route),
            "alternatives": d.alternatives,
            "rejection_reasons": d.rejection_reasons,
            "discovery_state": d.discovery_state.as_str(),
            "score_breakdown": {
                "reason": format!("{:?}", d.score_breakdown.reason),
                "effective_axes": {
                    "efficiency": d.score_breakdown.effective_axes.efficiency,
                    "precision": d.score_breakdown.effective_axes.precision,
                    "latency": d.score_breakdown.effective_axes.latency,
                    "availability": d.score_breakdown.effective_axes.availability,
                    "balance": d.score_breakdown.effective_axes.balance,
                    "mobile": d.score_breakdown.effective_axes.mobile
                },
                "capability_match_count": d.score_breakdown.capability_match_count,
                "candidate_count": d.score_breakdown.candidate_count,
                "intelligence_score": d.score_breakdown.intelligence_score,
                "efficiency_score": d.score_breakdown.efficiency_score,
                "latency_score": d.score_breakdown.latency_score,
                "telemetry_quality_score": d.score_breakdown.telemetry_quality_score
            }
        })
    }).unwrap_or(json!(null));
    ok(json!({
        "source": "registry",
        "arm_stats": arms_json,
        "arm_count": arms.len(),
        "decision_preview": decision_json,
    }))
}

/// Documents that manual routing overrides are **not** applied through this HTTP surface;
/// operators use the same SSOT as `resolve.rs` (secrets, policy files, CLI).
pub async fn get_routing_manual_ssot(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    ok(json!({
        "mutation_supported": false,
        "ssot": "RoutingPolicy files, secrets, and MCP resolve path (vox-orchestrator-mcp model_route_policy::resolve)",
        "operator_docs": "docs/src/how-to/how-to-model-routing.md",
        "message": "Pin models and edit routing policy through the documented SSOT; this endpoint is read-only guidance for the dashboard."
    }))
}

pub async fn post_mesh_node_kill(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_write(&gs, &connect.0, &headers) {
        return e;
    }
    ok(json!({
        "acknowledged": false,
        "node_id": id,
        "message": "Mesh kill is not wired to orchestrator dispatch in this build; use MCP vox_cancel_task / vox_emergency_stop or enable future mesh driver integration.",
    }))
}

/// GET /api/v2/models/catalog — registry snapshot for dashboard / vox-gui parity.
pub async fn get_models_catalog(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) {
        return e;
    }
    let models = gs.server_state.orchestrator.models_handle();
    let snapshot = match poison_rw_read(models.read(), "model registry for catalog") {
        Ok(guard) => guard.list_models(),
        Err(e) => return err("lock", &e.to_string()),
    };
    let arm_stats = match poison_rw_read(models.read(), "arm stats for catalog") {
        Ok(guard) => guard.arm_stats_snapshot().clone(),
        Err(e) => return err("lock", &e.to_string()),
    };
    ok(json!({
        "source": "registry",
        "model_count": snapshot.len(),
        "models": snapshot,
        "confidence_state": snapshot
            .iter()
            .map(|m| (m.id.clone(), confidence_state_for_model(m)))
            .collect::<std::collections::HashMap<_, _>>(),
        "arm_stats": arm_stats,
    }))
}

/// Build the dashboard sub-router nested at `/api/v2`.
pub fn router() -> Router<GatewayState> {
    Router::new()
        .route("/mesh/nodes", get(get_mesh_nodes))
        .route("/runs/recent", get(get_runs_recent))
        .route(
            "/dashboard/layout",
            get(get_dashboard_layout).put(put_dashboard_layout),
        )
        .route("/routing/summary", get(get_routing_summary))
        .route("/routing/manual-ssot", get(get_routing_manual_ssot))
        .route("/models/catalog", get(get_models_catalog))
        .route("/mesh/nodes/:id/kill", post(post_mesh_node_kill))
}
