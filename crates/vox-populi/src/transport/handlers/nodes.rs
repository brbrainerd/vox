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
#[cfg(feature = "transport")]
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

pub(super) fn store_put_a2a(st: &PopuliTransportState, msg: A2AStoredMessage) {
    if let Some(ms) = st.mesh_store.clone() {
        tokio::spawn(async move {
            if let Err(e) = ms.put_a2a(&msg).await {
                tracing::warn!(error = %e, msg_id = msg.id, "mesh_store put_a2a failed");
            }
        });
    }
}

pub(super) fn store_ack_a2a(st: &PopuliTransportState, message_id: u64, acked_unix_ms: u64) {
    if let Some(ms) = st.mesh_store.clone() {
        tokio::spawn(async move {
            if let Err(e) = ms
                .ack_a2a(
                    message_id,
                    super::super::store::A2AAck {
                        acknowledged: true,
                        acked_unix_ms,
                    },
                )
                .await
            {
                tracing::warn!(error = %e, message_id, "mesh_store ack_a2a failed");
            }
        });
    }
}

pub(super) fn store_put_exec_lease(st: &PopuliTransportState, row: RemoteExecLeaseRow) {
    if let Some(ms) = st.mesh_store.clone() {
        tokio::spawn(async move {
            if let Err(e) = ms.put_exec_lease(&row).await {
                tracing::warn!(error = %e, lease_id = %row.lease_id, "mesh_store put_exec_lease failed");
            }
        });
    }
}

pub(super) fn store_revoke_exec_lease(st: &PopuliTransportState, lease_id: String) {
    if let Some(ms) = st.mesh_store.clone() {
        tokio::spawn(async move {
            if let Err(e) = ms.revoke_exec_lease(&lease_id).await {
                tracing::warn!(error = %e, lease_id, "mesh_store revoke_exec_lease failed");
            }
        });
    }
}

pub(crate) async fn registry_sweep_maintenance(st: &PopuliTransportState) {
    let now = crate::now_ms();
    let mut inner = st.inner.write().await;
    sweep_expired_maintenance_on_nodes(&mut inner.nodes, now);

    #[cfg(feature = "transport")]
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
    if src.gpu_vram_total_mb.is_some() {
        target.gpu_vram_total_mb = src.gpu_vram_total_mb;
    }
    if src.gpu_model_name.is_some() {
        target.gpu_model_name = src.gpu_model_name.clone();
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
    // This is the live copy — `vox populi serve` routes through it.
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

// ── Mesh resource summary (resource-aware orchestration) ───────────────────────

/// Aggregated capacity across all known mesh nodes — the single query an
/// orchestrator uses to answer "how many nodes are available and what are
/// their resources?".
#[derive(Debug, Clone, serde::Serialize)]
pub struct MeshResourceSummary {
    pub node_count: usize,
    /// Nodes accepting new work (not quarantined, not in maintenance drain).
    pub eligible_node_count: usize,
    pub gpu_total: u32,
    pub gpu_allocatable_total: u32,
    pub memory_free_bytes_total: u64,
    /// Mean of reported cpu_usage_pct across eligible nodes (0 when none report).
    pub cpu_usage_pct_avg: f32,
    pub nodes: Vec<MeshResourceNode>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MeshResourceNode {
    pub node_id: String,
    pub eligible: bool,
    pub cpu_usage_pct: Option<f32>,
    pub memory_free_bytes: Option<u64>,
    pub gpu_allocatable_count: Option<u32>,
    pub gpu_total_count: Option<u32>,
    pub loaded_llm_models: Vec<String>,
    pub labels: Vec<String>,
}

pub(crate) fn aggregate_resources(nodes: &[NodeRecord]) -> MeshResourceSummary {
    let now = crate::now_ms();
    let mut summary = MeshResourceSummary {
        node_count: nodes.len(),
        eligible_node_count: 0,
        gpu_total: 0,
        gpu_allocatable_total: 0,
        memory_free_bytes_total: 0,
        cpu_usage_pct_avg: 0.0,
        nodes: Vec::with_capacity(nodes.len()),
    };
    let mut cpu_sum = 0.0f64;
    let mut cpu_n = 0usize;
    for n in nodes {
        let eligible = n.quarantined != Some(true) && !node_maintenance_blocks_new_work(now, n);
        if eligible {
            summary.eligible_node_count += 1;
            summary.gpu_total += n.gpu_total_count.unwrap_or(0);
            summary.gpu_allocatable_total += n.gpu_allocatable_count.unwrap_or(0);
            summary.memory_free_bytes_total += n.memory_free_bytes.unwrap_or(0);
            if let Some(c) = n.cpu_usage_pct {
                cpu_sum += f64::from(c);
                cpu_n += 1;
            }
        }
        summary.nodes.push(MeshResourceNode {
            node_id: n.id.clone(),
            eligible,
            cpu_usage_pct: n.cpu_usage_pct,
            memory_free_bytes: n.memory_free_bytes,
            gpu_allocatable_count: n.gpu_allocatable_count,
            gpu_total_count: n.gpu_total_count,
            loaded_llm_models: n.loaded_llm_models.clone().unwrap_or_default(),
            labels: n.capabilities.labels.clone(),
        });
    }
    if cpu_n > 0 {
        summary.cpu_usage_pct_avg = (cpu_sum / cpu_n as f64) as f32;
    }
    summary
}

pub(crate) async fn resources_summary(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
) -> Result<Json<MeshResourceSummary>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for resource summary".into(),
        ));
    }
    let g = st.inner.read().await;
    Ok(Json(aggregate_resources(&g.nodes)))
}

#[cfg(test)]
mod resources_summary_tests {
    use super::*;

    fn node(
        id: &str,
        cpu: Option<f32>,
        mem_free: Option<u64>,
        gpus_alloc: Option<u32>,
    ) -> NodeRecord {
        let mut n: NodeRecord = serde_json::from_value(serde_json::json!({
            "id": id,
            "capabilities": {},
            "version": "test",
            "last_seen_unix_ms": 0,
        }))
        .expect("minimal NodeRecord");
        n.cpu_usage_pct = cpu;
        n.memory_free_bytes = mem_free;
        n.gpu_allocatable_count = gpus_alloc;
        n.gpu_total_count = gpus_alloc;
        n
    }

    #[test]
    fn aggregates_counts_and_capacity_excluding_quarantined() {
        let mut quarantined = node("q", Some(5.0), None, Some(4));
        quarantined.quarantined = Some(true);
        let nodes = vec![
            node("a", Some(10.0), Some(8 * 1024 * 1024 * 1024), Some(1)),
            node("b", Some(90.0), Some(2 * 1024 * 1024 * 1024), Some(0)),
            quarantined,
        ];
        let s = aggregate_resources(&nodes);
        assert_eq!(s.node_count, 3);
        assert_eq!(s.eligible_node_count, 2);
        assert_eq!(s.gpu_allocatable_total, 1); // quarantined node's 4 GPUs excluded
        assert_eq!(s.memory_free_bytes_total, 10 * 1024 * 1024 * 1024);
        assert!((s.cpu_usage_pct_avg - 50.0).abs() < 0.01);
        assert_eq!(s.nodes.len(), 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_record(id: &str) -> NodeRecord {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","capabilities":{{}},"version":"0.0.0","last_seen_unix_ms":0,"memory_free_bytes":null}}"#
        ))
        .expect("minimal NodeRecord JSON parses")
    }

    #[test]
    fn merge_carries_gpu_vram_and_model_from_heartbeat() {
        // Server-side existing record with no GPU VRAM/model advertised yet.
        let mut target = minimal_record("node-1");
        assert_eq!(target.gpu_vram_total_mb, None);
        assert_eq!(target.gpu_model_name, None);

        // Incoming heartbeat from the worker advertising probed VRAM + model.
        let mut src = minimal_record("node-1");
        src.gpu_vram_total_mb = Some(16376);
        src.gpu_model_name = Some("RTX 4080 SUPER".to_string());

        merge_optional_node_fields(&mut target, &src);

        assert_eq!(target.gpu_vram_total_mb, Some(16376));
        assert_eq!(target.gpu_model_name.as_deref(), Some("RTX 4080 SUPER"));
    }
}
