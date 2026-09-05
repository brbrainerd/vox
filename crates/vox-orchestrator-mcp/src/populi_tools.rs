//! Populi (Vox Populi distributed mesh) introspection MCP tools.
//!
//! - `mesh_local_status` — `vox_populi_local_status` (legacy; local registry + env dump).
//! - `mesh_nodes` — `vox_mesh_nodes`: node list (control plane preferred, local-registry fallback).
//! - `mesh_queue_stats` — `vox_mesh_queue_stats`: queue stats (control plane preferred, local fallback).
//! - `mesh_dispatch` — `vox_mesh_dispatch`: dispatch a script (control-plane only; clear error otherwise).
//!
//! The control plane is the `vox-populi` HTTP API (`GET /v1/populi/nodes`,
//! `GET /v1/populi/queue/stats`, `POST /v1/populi/dispatch`). Its base URL is resolved from
//! [`crate::server_state::ServerState`]'s `populi_control_url` config, falling back to the
//! `VOX_ORCHESTRATOR_MESH_CONTROL_URL` / `VOX_MESH_CONTROL_ADDR` secrets (same precedent as
//! `vox-orchestrator::catalog::discover_populi_mesh_models`). Bearer auth is picked up from
//! `VOX_MESH_TOKEN` via `with_env_token`.

use serde_json::{Value, json};

use crate::server_state::ServerState;

/// Return mens environment + on-disk registry as JSON text.
pub fn mesh_local_status(args: Value) -> anyhow::Result<String> {
    let path = args
        .get("registry_path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(vox_populi::local_registry_path);
    let reg = vox_populi::LocalRegistry::new(path.clone());
    let file = reg.load()?;
    let env = vox_populi::populi_env();
    let out = json!({
        "populi_env": env,
        "registry_path": reg.path().display().to_string(),
        "registry": file,
    });
    Ok(out.to_string())
}

/// Resolve the configured populi control-plane base URL, if any.
///
/// Order: orchestrator config `populi_control_url`, then the
/// `VOX_ORCHESTRATOR_MESH_CONTROL_URL` secret, then `VOX_MESH_CONTROL_ADDR`.
/// Returns a normalized (trimmed, non-empty) base URL.
#[allow(dead_code)]
fn resolve_control_url(state: &ServerState) -> Option<String> {
    if let Some(u) = state
        .orchestrator_config
        .populi_control_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(u.to_string());
    }
    let from_secret = |id| {
        vox_secrets::resolve_secret(id)
            .expose()
            .map(|s: &str| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    from_secret(vox_secrets::SecretId::VoxOrchestratorMeshControlUrl)
        .or_else(|| from_secret(vox_secrets::SecretId::VoxMeshControlAddr))
}

/// Build a bearer-authenticated control-plane client for `base`.
#[cfg(feature = "populi-transport")]
fn control_client(base: &str) -> vox_populi::http_client::PopuliHttpClient {
    vox_populi::http_client::PopuliHttpClient::new(base).with_env_token()
}

/// Condense a [`vox_populi::NodeRecord`] into a compact, GUI-friendly JSON row.
fn summarize_node(n: &vox_populi::NodeRecord) -> Value {
    // Derived status: explicit quarantine / maintenance take precedence; otherwise "online".
    let status = if n.quarantined == Some(true) {
        "quarantined"
    } else if n.maintenance == Some(true) {
        "maintenance"
    } else {
        "online"
    };

    let gpu_summary = match (n.gpu_total_count, n.gpu_allocatable_count) {
        (Some(total), Some(alloc)) => format!("{alloc}/{total} GPU"),
        (Some(total), None) => format!("{total} GPU"),
        _ => "—".to_string(),
    };

    let advertised_models: Vec<String> = n
        .advertised_models
        .as_ref()
        .map(|ms| ms.iter().map(|m| m.model_id.clone()).collect())
        .unwrap_or_default();

    json!({
        "id": n.id,
        "status": status,
        "host_triple": n.host_triple,
        "gpu_summary": gpu_summary,
        "gpu_total_count": n.gpu_total_count,
        "gpu_allocatable_count": n.gpu_allocatable_count,
        "gpu_truth_layer": n.gpu_truth_layer,
        "cpu_usage_pct": n.cpu_usage_pct,
        "memory_free_bytes": n.memory_free_bytes,
        "trust_tier": n.trust_tier,
        "ed25519_pub_key_b64": n.ed25519_pub_key_b64,
        "advertised_models": advertised_models,
        "last_seen_unix_ms": n.last_seen_unix_ms,
        "version": n.version,
        "listen_addr": n.listen_addr,
    })
}

fn registry_to_nodes_json(file: &vox_populi::PopuliRegistryFile) -> Vec<Value> {
    file.nodes.iter().map(summarize_node).collect()
}

/// `vox_mesh_nodes` — node list with status.
///
/// Prefers the control plane (`GET /v1/populi/nodes`) when a control URL is configured and
/// reachable; otherwise falls back to the on-disk [`vox_populi::LocalRegistry`] (the same source
/// `vox_populi_local_status` reads). The `source` field is always one of `control_plane` or
/// `local_registry`; a `control_plane_error` field is present when a configured control plane
/// could not be reached (so the fallback is observable, not silent).
#[cfg_attr(not(feature = "populi-transport"), allow(unused_variables))]
pub async fn mesh_nodes(state: &ServerState, _args: Value) -> anyhow::Result<String> {
    #[cfg(feature = "populi-transport")]
    if let Some(base) = resolve_control_url(state) {
        match control_client(&base).list_nodes().await {
            Ok(file) => {
                return Ok(json!({
                    "source": "control_plane",
                    "control_url": base,
                    "nodes": registry_to_nodes_json(&file),
                    "queue_depth": file.queue_depth,
                    "node_count": file.nodes.len(),
                })
                .to_string());
            }
            Err(e) => {
                // Fall through to the local registry, but make the failure visible.
                let path = vox_populi::local_registry_path();
                let reg = vox_populi::LocalRegistry::new(path.clone());
                let file = reg.load()?;
                return Ok(json!({
                    "source": "local_registry",
                    "control_plane_error": format!("{e}"),
                    "control_url": base,
                    "registry_path": reg.path().display().to_string(),
                    "nodes": registry_to_nodes_json(&file),
                    "queue_depth": file.queue_depth,
                    "node_count": file.nodes.len(),
                })
                .to_string());
            }
        }
    }

    let path = vox_populi::local_registry_path();
    let reg = vox_populi::LocalRegistry::new(path.clone());
    let file = reg.load()?;
    Ok(json!({
        "source": "local_registry",
        "registry_path": reg.path().display().to_string(),
        "nodes": registry_to_nodes_json(&file),
        "queue_depth": file.queue_depth,
        "node_count": file.nodes.len(),
    })
    .to_string())
}

/// `vox_mesh_queue_stats` — pending queue depth / breakdown.
///
/// Order: the **mesh** first (plan Task 3.3 — trusted peers probed just now),
/// then the control plane (`GET /v1/populi/queue/stats`, retained until Phase 6
/// deletes it), then the local registry's `queue_depth`.
///
/// The mesh is preferred because its numbers come from peers that answered,
/// not from a list a control plane was told about. They are still
/// *peer-asserted*: `peers_answered` is reported alongside so a reader can see
/// how many machines the total is made of. When no peer answers, the mesh is
/// skipped entirely rather than reporting a depth of zero over a real queue.
#[cfg_attr(not(feature = "populi-transport"), allow(unused_variables))]
pub async fn mesh_queue_stats(state: &ServerState, _args: Value) -> anyhow::Result<String> {
    #[cfg(feature = "populi-transport")]
    {
        let mesh = vox_orchestrator::models::mesh_directory::queue_stats().await;
        if mesh.peers_answered > 0 {
            let by_kind: serde_json::Map<String, Value> = mesh
                .pending_by_kind
                .iter()
                .map(|(k, n)| (k.as_str().to_string(), json!(n)))
                .collect();
            let by_priority: serde_json::Map<String, Value> = mesh
                .pending_by_priority
                .iter()
                .map(|(p, n)| (p.to_string(), json!(n)))
                .collect();
            return Ok(json!({
                "source": "mesh",
                "peers_answered": mesh.peers_answered,
                "pending_count": mesh.pending_count,
                "pending_by_kind": by_kind,
                "pending_by_priority": by_priority,
            })
            .to_string());
        }
    }

    #[cfg(feature = "populi-transport")]
    if let Some(base) = resolve_control_url(state) {
        match control_client(&base).queue_stats().await {
            Ok(stats) => {
                return Ok(json!({
                    "source": "control_plane",
                    "control_url": base,
                    "pending_count": stats.pending_count,
                    "pending_by_kind": stats.pending_by_kind,
                    "pending_by_priority": stats.pending_by_priority,
                })
                .to_string());
            }
            Err(e) => {
                let path = vox_populi::local_registry_path();
                let file = vox_populi::LocalRegistry::new(path).load()?;
                return Ok(json!({
                    "source": "local_registry",
                    "control_plane_error": format!("{e}"),
                    "control_url": base,
                    "pending_count": file.queue_depth,
                })
                .to_string());
            }
        }
    }

    let path = vox_populi::local_registry_path();
    let file = vox_populi::LocalRegistry::new(path).load()?;
    Ok(json!({
        "source": "local_registry",
        "pending_count": file.queue_depth,
    })
    .to_string())
}

/// `vox_mesh_dispatch` — submit a script to the control plane for remote execution.
///
/// Requires a configured control URL: dispatch is a *write* against live mesh workers, so when no
/// control plane is configured this returns a clear `is_error` envelope rather than faking success.
/// Args: `source`/`script` (required), optional `node_id`, `task_kind`, `min_vram_mb`.
#[cfg(not(feature = "populi-transport"))]
pub async fn mesh_dispatch(_state: &ServerState, _args: Value) -> anyhow::Result<String> {
    Ok(crate::params::ToolResult::<()>::err(
        "Mesh dispatch is unavailable: this build was compiled without the `populi-transport` \
         feature, so no control-plane HTTP client is linked. Rebuild with \
         `--features populi-transport` and configure a control URL to dispatch.",
    )
    .to_json_compact())
}

#[cfg(feature = "populi-transport")]
pub async fn mesh_dispatch(state: &ServerState, args: Value) -> anyhow::Result<String> {
    let Some(base) = resolve_control_url(state) else {
        return Ok(crate::params::ToolResult::<()>::err(
            "Mesh dispatch is not configured: no populi control URL set. \
             Set `populi_control_url` (TOML `[orchestrator].populi_control_url` / `[mens].control_url`) \
             or the VOX_ORCHESTRATOR_MESH_CONTROL_URL env var, then retry.",
        )
        .to_json_compact());
    };

    let source = args
        .get("source")
        .or_else(|| args.get("script"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(source) = source else {
        return Ok(crate::params::ToolResult::<()>::err(
            "Mesh dispatch requires a non-empty `source` (or `script`) field with .vox source code.",
        )
        .to_json_compact());
    };

    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let task_kind = args
        .get("task_kind")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let min_vram_mb = args
        .get("min_vram_mb")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    let req = vox_populi::transport::DispatchRequest {
        source: source.to_string(),
        node_id,
        timeout_secs: 30,
        is_bundle: false,
        source_blake3_hex: None,
        required_labels: None,
        is_detached: false,
        priority: 128,
        task_kind,
        model_id: None,
        min_vram_mb,
    };

    match control_client(&base).dispatch(&req).await {
        Ok(resp) => Ok(crate::params::ToolResult::ok(json!({
            "control_url": base,
            "success": resp.success,
            "output": resp.output,
            "is_truncated": resp.is_truncated,
            "duration_ms": resp.duration_ms,
            "exit_code": resp.exit_code,
            "error": resp.error,
            "node_id": resp.node_id,
        }))
        .to_json_compact()),
        Err(e) => Ok(crate::params::ToolResult::<()>::err(format!(
            "Mesh dispatch to {base} failed: {e}"
        ))
        .to_json_compact()),
    }
}

#[cfg(test)]
mod summarize_node_tests {
    use super::*;

    #[test]
    fn exposes_cpu_and_free_memory_for_resource_card() {
        let mut n: vox_populi::NodeRecord = serde_json::from_value(serde_json::json!({
            "id": "node-a",
            "capabilities": {},
            "version": "test",
            "last_seen_unix_ms": 0,
        }))
        .expect("minimal NodeRecord");
        n.cpu_usage_pct = Some(42.5);
        n.memory_free_bytes = Some(8 * 1024 * 1024 * 1024);
        let v = summarize_node(&n);
        assert_eq!(v["cpu_usage_pct"].as_f64(), Some(42.5));
        assert_eq!(
            v["memory_free_bytes"].as_u64(),
            Some(8 * 1024 * 1024 * 1024)
        );
    }
}
