//! Best-effort mens local registry publish when MCP starts (mirrors `vox run`), plus optional HTTP
//! control-plane **join** and **heartbeat** when a client-suitable base URL is configured.

use crate::server_state::ServerState;
use vox_populi::http_lifecycle::{PopuliHttpJoinSpawnOutcome, populi_http_join_best_effort};

/// If **`VOX_MESH_ENABLED`**, write this process into the local mens registry; optional Codex row when
/// **`VOX_MESH_CODEX_TELEMETRY`** is set (uses [`vox_db::populi_registry_telemetry`]).
///
/// When **`VOX_MESH_HTTP_JOIN`** is not disabled and a URL from **`VOX_ORCHESTRATOR_MESH_CONTROL_URL`**
/// or **`VOX_MESH_CONTROL_ADDR`** normalizes to a non-bind-all HTTP(S) base, also **`POST /v1/populi/join`**
/// and (unless **`VOX_MESH_HTTP_HEARTBEAT_SECS=0`**) a background **`POST /v1/populi/heartbeat`** loop.
pub async fn publish_mesh_on_mcp_start(state: &ServerState) {
    if !vox_populi::populi_enabled_from_env() {
        return;
    }
    let node_id = vox_populi::populi_env().node_id.clone();
    let path = vox_populi::local_registry_path();
    match vox_populi::publish_local_registry_best_effort() {
        Ok(()) => {
            tracing::info!(
                target: "vox.populi",
                path = %path.display(),
                node_id = node_id.as_deref().unwrap_or("(generated)"),
                "mens registry publish (vox-mcp)"
            );
            vox_db::populi_registry_telemetry::record_local_registry_publish_opt(
                &state.repository.repository_id,
                &path,
                node_id.as_deref(),
            )
            .await;
        }
        Err(e) => {
            tracing::debug!(
                target: "vox.populi",
                error = %e,
                "mens registry publish failed (best-effort)"
            );
        }
    }

    let record = vox_populi::populi_registration_record_for_process();
    match populi_http_join_best_effort(record, "vox-mcp").await {
        PopuliHttpJoinSpawnOutcome::Skipped => {
            tracing::debug!(
                target: "vox.populi",
                "populi HTTP join skipped (disabled or no control base)"
            );
        }
        PopuliHttpJoinSpawnOutcome::Joined { base, node_id } => {
            tracing::info!(
                target: "vox.populi",
                control_base = %base,
                node_id = %node_id,
                "populi HTTP join established (vox-mcp)"
            );
            vox_db::populi_registry_telemetry::record_populi_http_join_opt(
                &state.repository.repository_id,
                true,
                &base,
                Some(node_id.as_str()),
                None,
            )
            .await;
        }
        PopuliHttpJoinSpawnOutcome::Failed { base, node_id, err } => {
            tracing::warn!(
                target: "vox.populi",
                control_base = %base,
                node_id = %node_id,
                error = %err,
                "populi HTTP join failed (vox-mcp)"
            );
            vox_db::populi_registry_telemetry::record_populi_http_join_opt(
                &state.repository.repository_id,
                false,
                &base,
                Some(node_id.as_str()),
                Some(&err.to_string()),
            )
            .await;
        }
    }
}
