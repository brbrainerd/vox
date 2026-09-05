//! Node-registry handlers: health, list, join, heartbeat, leave, bootstrap.
//! Also contains shared write-through store helpers and small utilities used across submodules.

use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::{info, warn};

use crate::{NodeRecord, node_maintenance_blocks_new_work, sweep_expired_maintenance_on_nodes};

use super::super::auth::{
    PopuliAuthContext, auth_allows_worker_plane, populi_control_token_from_env,
};

use super::super::dispatch_results_sweep;
use super::super::store::scope_ok;
use super::super::{
    A2AStoredMessage, BootstrapExchangeRequest, BootstrapExchangeResponse, LeaveRequest,
    PopuliRegistryFile, PopuliTransportState, RemoteExecLeaseRow, server_stale_prune_ms,
};

// ─────────────────────────────────────────────────────────────────────────────
// Public surface
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok\n")
}

// ── write-through helpers ─────────────────────────────────────────────────────
// Each spawns a best-effort durable write; failures are logged but never returned
// to callers (matching the existing JSON persist semantics).

pub(super) fn store_put_a2a(_st: &PopuliTransportState, _msg: A2AStoredMessage) {
    // No durable mesh store attached; data lives in the in-memory cache only.
}

pub(super) fn store_ack_a2a(_st: &PopuliTransportState, _message_id: u64, _acked_unix_ms: u64) {
    // No durable mesh store attached.
}

pub(super) fn store_put_exec_lease(_st: &PopuliTransportState, _row: RemoteExecLeaseRow) {
    // No durable mesh store attached.
}

pub(super) fn store_revoke_exec_lease(_st: &PopuliTransportState, _lease_id: String) {
    // No durable mesh store attached.
}

pub(crate) async fn registry_sweep_maintenance(st: &PopuliTransportState) {
    let now = crate::now_ms();
    let mut inner = st.inner.write().await;
    sweep_expired_maintenance_on_nodes(&mut inner.nodes, now);

    dispatch_results_sweep(&st.dispatch_results, now);
}

pub(crate) async fn list_nodes(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
) -> Result<Json<PopuliRegistryFile>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for node list".into(),
        ));
    }
    registry_sweep_maintenance(&st).await;
    let mut g = st.inner.read().await.clone();
    if let Some(window) = server_stale_prune_ms() {
        let now = crate::now_ms();
        g.nodes
            .retain(|n| now.saturating_sub(n.last_seen_unix_ms) <= window);
    }

    let a2a = st.a2a_messages.read().await;
    let pending = a2a
        .iter()
        .filter(|m| !m.acknowledged && m.lease_holder_node_id.is_none())
        .count();
    g.queue_depth = Some(pending);

    Ok(Json(g))
}

pub(crate) async fn join_node(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(mut node): Json<NodeRecord>,
) -> Result<Json<NodeRecord>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for join".into(),
        ));
    }
    if !scope_ok(&st, &node) {
        warn!(node_id = %node.id, "join rejected: populi scope mismatch");
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi scope mismatch: set VOX_MESH_SCOPE_ID to match server".into(),
        ));
    }
    node.quarantined = None;
    node.last_seen_unix_ms = crate::now_ms();
    let mut g = st.inner.write().await;
    let now = crate::now_ms();
    sweep_expired_maintenance_on_nodes(&mut g.nodes, now);
    if let Some(i) = g.nodes.iter().position(|n| n.id == node.id) {
        let preserve_q = g.nodes[i].quarantined;
        let preserve_m = g.nodes[i].maintenance;
        let preserve_mu = g.nodes[i].maintenance_until_unix_ms;
        g.nodes[i] = node.clone();
        g.nodes[i].quarantined = preserve_q;
        g.nodes[i].maintenance = preserve_m;
        g.nodes[i].maintenance_until_unix_ms = preserve_mu;
    } else {
        g.nodes.push(node.clone());
    }
    let out = g
        .nodes
        .iter()
        .find(|n| n.id == node.id)
        .cloned()
        .expect("join upsert must leave node in registry");
    Ok(Json(out))
}

fn merge_optional_node_fields(target: &mut NodeRecord, src: &NodeRecord) {
    if src.listen_addr.is_some() {
        target.listen_addr = src.listen_addr.clone();
    }
    if src.scope_id.is_some() {
        target.scope_id = src.scope_id.clone();
    }
    if src.visibility.is_some() {
        target.visibility = src.visibility.clone();
    }
    if src.pool_id.is_some() {
        target.pool_id = src.pool_id.clone();
    }
    if src.trust_tier.is_some() {
        target.trust_tier = src.trust_tier.clone();
    }
    if src.workload_classes.is_some() {
        target.workload_classes = src.workload_classes.clone();
    }
    if src.privacy_class.is_some() {
        target.privacy_class = src.privacy_class.clone();
    }
    if src.maintenance_until_unix_ms.is_some() {
        target.maintenance_until_unix_ms = src.maintenance_until_unix_ms;
    }
    if src.maintenance.is_some() {
        target.maintenance = src.maintenance;
        if target.maintenance != Some(true) {
            target.maintenance_until_unix_ms = None;
        }
    }
    if src.provider.is_some() {
        target.provider = src.provider.clone();
    }
    if src.advertised_models.is_some() {
        target.advertised_models = src.advertised_models.clone();
    }
    if src.donation_policy.is_some() {
        target.donation_policy = src.donation_policy.clone();
    }
    if src.owner_vox_user_id.is_some() {
        target.owner_vox_user_id = src.owner_vox_user_id.clone();
    }
    if src.ed25519_pub_key_b64.is_some() {
        target.ed25519_pub_key_b64 = src.ed25519_pub_key_b64.clone();
    }
}

pub(crate) async fn heartbeat(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(mut node): Json<NodeRecord>,
) -> Result<Json<NodeRecord>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for heartbeat".into(),
        ));
    }
    if !scope_ok(&st, &node) {
        warn!(node_id = %node.id, "heartbeat rejected: populi scope mismatch");
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi scope mismatch: set VOX_MESH_SCOPE_ID to match server".into(),
        ));
    }
    node.quarantined = None;
    node.last_seen_unix_ms = crate::now_ms();
    let mut g = st.inner.write().await;
    let now = crate::now_ms();
    sweep_expired_maintenance_on_nodes(&mut g.nodes, now);
    if let Some(i) = g.nodes.iter().position(|n| n.id == node.id) {
        let preserve_q = g.nodes[i].quarantined;
        let preserve_m = g.nodes[i].maintenance;
        let preserve_mu = g.nodes[i].maintenance_until_unix_ms;
        g.nodes[i].last_seen_unix_ms = node.last_seen_unix_ms;
        merge_optional_node_fields(&mut g.nodes[i], &node);
        g.nodes[i].quarantined = preserve_q;
        g.nodes[i].maintenance = preserve_m;
        g.nodes[i].maintenance_until_unix_ms = preserve_mu;
        Ok(Json(g.nodes[i].clone()))
    } else {
        g.nodes.push(node.clone());
        Ok(Json(node))
    }
}

pub(crate) struct ResponseErr(pub(crate) StatusCode, pub(crate) String);

impl IntoResponse for ResponseErr {
    fn into_response(self) -> axum::response::Response {
        (self.0, self.1).into_response()
    }
}

pub(crate) async fn leave_node(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<LeaveRequest>,
) -> Result<StatusCode, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for leave".into(),
        ));
    }
    let mut g = st.inner.write().await;
    let before = g.nodes.len();
    g.nodes.retain(|n| n.id != req.id);
    if g.nodes.len() < before {
        Ok(StatusCode::NO_CONTENT)
    } else {
        warn!(node_id = %req.id, "leave requested for unknown node");
        Ok(StatusCode::NOT_FOUND)
    }
}

pub(crate) async fn bootstrap_exchange(
    State(st): State<PopuliTransportState>,
    Json(req): Json<BootstrapExchangeRequest>,
) -> Result<Json<BootstrapExchangeResponse>, ResponseErr> {
    let Some(expected) = st.bootstrap_token.as_ref() else {
        return Err(ResponseErr(
            StatusCode::NOT_FOUND,
            "bootstrap exchange is not enabled".into(),
        ));
    };
    // Cheap rejections first, and CRUCIALLY the token comparison before the
    // one-shot window is consumed. This previously swapped `bootstrap_used`
    // ahead of `bearer_token_eq`, so a single unauthenticated POST carrying any
    // wrong token permanently burned the window and locked the real peer out.
    if let Some(expires) = st.bootstrap_expires_unix_ms
        && crate::now_ms() > expires
    {
        warn!("bootstrap exchange rejected: token expired");
        return Err(ResponseErr(
            StatusCode::GONE,
            "bootstrap token expired".into(),
        ));
    }
    if !super::super::auth::bearer_token_eq(expected.as_ref(), req.bootstrap_token.trim()) {
        warn!("bootstrap exchange rejected: invalid token");
        return Err(ResponseErr(
            StatusCode::UNAUTHORIZED,
            "invalid bootstrap token".into(),
        ));
    }
    // Only a request that proved the token consumes the window. `swap` stays the
    // claim so two concurrent CORRECT requests cannot both be granted — the
    // loser sees `true` and is told the token is spent.
    if st.bootstrap_used.swap(true, Ordering::SeqCst) {
        warn!("bootstrap exchange rejected: token already used");
        return Err(ResponseErr(
            StatusCode::GONE,
            "bootstrap token already consumed".into(),
        ));
    }
    let mesh_token = populi_control_token_from_env().ok_or_else(|| {
        ResponseErr(
            StatusCode::SERVICE_UNAVAILABLE,
            "server missing VOX_MESH_TOKEN".into(),
        )
    })?;
    info!("bootstrap exchange granted");
    Ok(Json(BootstrapExchangeResponse {
        mesh_token,
        scope_id: crate::populi_scope_id_from_env(),
    }))
}

/// Mesh A2A wire ids: trim, non-empty, ASCII decimal digits only (orchestrator agent id JSON form).
pub(super) fn parse_a2a_mesh_agent_id(label: &str, raw: &str) -> Result<String, ResponseErr> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            format!("populi: {label} required (non-empty decimal digit string)"),
        ));
    }
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            format!(
                "populi: {label} must be a non-empty decimal digit string (orchestrator agent id)"
            ),
        ));
    }
    Ok(s.to_string())
}

pub(super) fn a2a_inbox_limit(requested: Option<usize>) -> usize {
    requested.unwrap_or(64).clamp(1, 256)
}

pub(crate) async fn require_claimer_worker_gate(
    st: &PopuliTransportState,
    claimer: &str,
) -> Result<(), ResponseErr> {
    if claimer.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            "populi: claimer_node_id required".into(),
        ));
    }
    registry_sweep_maintenance(st).await;
    let now = crate::now_ms();
    let worker = {
        let reg = st.inner.read().await;
        reg.nodes.iter().find(|n| n.id == claimer).cloned()
    };
    let Some(worker) = worker else {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: unknown claimer_node_id (join node first)".into(),
        ));
    };
    if worker.quarantined == Some(true) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: claimer node is quarantined".into(),
        ));
    }
    if node_maintenance_blocks_new_work(now, &worker) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: claimer node is in maintenance mode".into(),
        ));
    }
    Ok(())
}

/// Like [`require_claimer_worker_gate`] but only verifies the node is registered (join).
/// Used for **exec lease release** so holders can clear `scope_key` while in maintenance/quarantine.
pub(crate) async fn require_claimer_node_registered(
    st: &PopuliTransportState,
    claimer: &str,
) -> Result<(), ResponseErr> {
    if claimer.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            "populi: claimer_node_id required".into(),
        ));
    }
    let known = {
        let reg = st.inner.read().await;
        reg.nodes.iter().any(|n| n.id == claimer)
    };
    if !known {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: unknown claimer_node_id (join node first)".into(),
        ));
    }
    Ok(())
}
