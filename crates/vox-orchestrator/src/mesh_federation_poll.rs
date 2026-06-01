//! Populi control-plane poll loop shared by MCP and `vox-orchestrator-d`.
//!
//! ## `mesh_exec_lease_reconcile` telemetry
//! When **`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_RECONCILE`** is truthy, stale exec leases are evaluated each tick.
//! A matching Codex row (`populi_control` / `record_populi_control_event`) with event name **`mesh_exec_lease_reconcile`**
//! is written only if **`VOX_MESH_CODEX_TELEMETRY=1`** and a [`vox_db::VoxDb`] handle is wired — default off.
//! Env SSOT: `docs/src/reference/env-vars.md`.

use std::sync::{Arc, Mutex, RwLock};

use tokio::task::JoinHandle;
use vox_db::VoxDb;

use crate::config::OrchestratorConfig;
use crate::orchestrator::Orchestrator;
use crate::populi_federation::{PopuliNodeBrief, RemotePopuliRoutingHint, RemotePopuliSnapshot};

/// Background poll of populi control plane when `populi_control_url` is set and `populi_poll_interval_secs` > 0.
pub fn spawn_populi_federation_poller(
    orchestrator_config: &OrchestratorConfig,
    repository_id: String,
    db: Option<Arc<VoxDb>>,
    orchestrator: Arc<Orchestrator>,
    snapshot: Arc<RwLock<RemotePopuliSnapshot>>,
    join_slot: Arc<Mutex<Option<JoinHandle<()>>>>,
) {
    let url = match orchestrator_config
        .populi_control_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(u) => u.to_string(),
        None => return,
    };
    if orchestrator_config.populi_poll_interval_secs == 0 {
        return;
    }
    let interval_secs = orchestrator_config.populi_poll_interval_secs.max(1);
    let timeout_ms = orchestrator_config.populi_http_timeout_ms.max(500);
    let populi_heartbeat_stale_ms = orchestrator_config.stale_threshold_ms;
    let populi_rebalance_on_remote_schedulable_drop =
        orchestrator_config.populi_rebalance_on_remote_schedulable_drop;
    let populi_replay_queued_routes_on_remote_schedulable_drop =
        orchestrator_config.populi_replay_queued_routes_on_remote_schedulable_drop;
    let reconcile_exec_leases =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorMeshExecLeaseReconcile)
            .expose()
            .map(|v: &str| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);
    let auto_revoke_exec_leases =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorMeshExecLeaseAutoRevoke)
            .expose()
            .map(|v: &str| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);
    let codex_mesh_telemetry =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshCodexTelemetry)
            .expose()
            .map(|v: &str| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);
    let db_reconcile = db;
    let repo_id_reconcile = repository_id;
    let snap = snapshot;
    let orch = orchestrator;
    let mut guard = join_slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = guard.take() {
        h.abort();
    }
    let handle = tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let timeout = std::time::Duration::from_millis(timeout_ms);
            let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(&url, timeout)
                .with_env_token();
            let now = vox_populi::wall_clock_unix_ms();
            match client.list_nodes().await {
                Ok(f) => {
                    // Opt-in (default OFF): persist the control-plane node list into
                    // the on-disk LocalRegistry (merge by last-seen). Off by default
                    // so the poller stays read-only unless an operator enables it.
                    if std::env::var("VOX_POPULI_PERSIST_LOCAL_REGISTRY")
                        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
                        .unwrap_or(false)
                    {
                        let reg = vox_populi::LocalRegistry::new(
                            vox_populi::LocalRegistry::resolved_default_path(),
                        );
                        if let Err(e) = client.sync_node_registry(&reg).await {
                            tracing::debug!(error = %e, "populi: local-registry sync failed");
                        }
                    }
                    let brief: Vec<PopuliNodeBrief> = f
                        .nodes
                        .iter()
                        .map(|n| PopuliNodeBrief {
                            id: n.id.clone(),
                            last_seen_unix_ms: n.last_seen_unix_ms,
                        })
                        .collect();
                    let routing_hints: Vec<RemotePopuliRoutingHint> = f
                        .nodes
                        .iter()
                        .map(|n| {
                            let heartbeat_stale = populi_heartbeat_stale_ms > 0
                                && now.saturating_sub(n.last_seen_unix_ms)
                                    > populi_heartbeat_stale_ms;
                            RemotePopuliRoutingHint {
                                node_id: n.id.clone(),
                                capabilities: n.capabilities.clone(),
                                labels: n.capabilities.labels.clone(),
                                gpu_cuda: n.capabilities.gpu_cuda,
                                gpu_metal: n.capabilities.gpu_metal,
                                min_vram_mb: n.capabilities.min_vram_mb,
                                gpu_total_count: n.gpu_total_count,
                                gpu_healthy_count: n.gpu_healthy_count,
                                gpu_allocatable_count: n.gpu_allocatable_count,
                                gpu_inventory_source: n.gpu_inventory_source.clone(),
                                gpu_truth_layer: n.gpu_truth_layer.clone(),
                                nvidia_driver_version: n.nvidia_driver_version.clone(),
                                cuda_driver_version: n.cuda_driver_version.clone(),
                                gpu_readiness_ok: n.gpu_readiness_ok,
                                gpu_readiness_reason: n.gpu_readiness_reason.clone(),
                                gpu_readiness_checked_unix_ms: n.gpu_readiness_checked_unix_ms,
                                training_labels: n
                                    .capabilities
                                    .labels
                                    .iter()
                                    .filter(|s| {
                                        s.starts_with("workload=") || s.starts_with("pool=")
                                    })
                                    .cloned()
                                    .collect(),
                                maintenance: vox_populi::node_maintenance_blocks_new_work(now, n),
                                quarantined: n.quarantined.unwrap_or(false),
                                heartbeat_stale,
                            }
                        })
                        .collect();

                    let hint_update = orch.set_remote_populi_routing_hints(routing_hints);
                    if hint_update.prev_schedulable > hint_update.new_schedulable {
                        if populi_rebalance_on_remote_schedulable_drop {
                            let n = orch.rebalance();
                            if n > 0 {
                                tracing::debug!(
                                    target: "vox.mcp.populi",
                                    rebalanced = n,
                                    "mesh federation: rebalanced after remote schedulable drop"
                                );
                            }
                        }
                        if populi_replay_queued_routes_on_remote_schedulable_drop {
                            let replayed = orch
                                .replay_queued_routes_after_populi_schedulable_drop()
                                .await;
                            if replayed > 0 {
                                tracing::info!(
                                    target: "vox.mcp.populi",
                                    replayed,
                                    "mesh federation: replayed queued routes after remote schedulable drop"
                                );
                            }
                        }
                    }

                    if reconcile_exec_leases {
                        match client.list_exec_leases().await {
                            Ok(list) => {
                                for row in list.leases {
                                    let holder = row.holder_node_id.as_str();
                                    let node = f.nodes.iter().find(|n| n.id == holder);
                                    let heartbeat_stale = populi_heartbeat_stale_ms > 0
                                        && node.is_some_and(|n| {
                                            now.saturating_sub(n.last_seen_unix_ms)
                                                > populi_heartbeat_stale_ms
                                        });
                                    let reason: Option<&'static str> = if node.is_none() {
                                        Some("holder_not_in_registry")
                                    } else if heartbeat_stale {
                                        Some("holder_heartbeat_stale")
                                    } else if node.is_some_and(|n| {
                                        vox_populi::node_maintenance_blocks_new_work(now, n)
                                    }) {
                                        Some("holder_in_maintenance")
                                    } else if node.is_some_and(|n| n.quarantined == Some(true)) {
                                        Some("holder_quarantined")
                                    } else {
                                        None
                                    };
                                    if let Some(reason) = reason {
                                        let warn = matches!(
                                            reason,
                                            "holder_not_in_registry" | "holder_heartbeat_stale"
                                        );
                                        if warn {
                                            tracing::warn!(
                                                target: "vox.mcp.populi_reconcile",
                                                lease_id = %row.lease_id,
                                                holder_node_id = %row.holder_node_id,
                                                scope_key = %row.scope_key,
                                                reason,
                                                expires_unix_ms = row.expires_unix_ms,
                                                "mesh exec lease holder may need operator attention"
                                            );
                                        } else {
                                            tracing::debug!(
                                                target: "vox.mcp.populi_reconcile",
                                                lease_id = %row.lease_id,
                                                holder_node_id = %row.holder_node_id,
                                                scope_key = %row.scope_key,
                                                reason,
                                                expires_unix_ms = row.expires_unix_ms,
                                                "mesh exec lease holder state"
                                            );
                                        }
                                        let mut auto_revoke_ok: Option<bool> = None;
                                        if auto_revoke_exec_leases {
                                            match client
                                                .admin_exec_lease_revoke(
                                                    &vox_populi::transport::AdminExecLeaseRevokeRequest {
                                                        lease_id: row.lease_id.clone(),
                                                    },
                                                )
                                                .await
                                            {
                                                Ok(()) => {
                                                    auto_revoke_ok = Some(true);
                                                    tracing::warn!(
                                                        target: "vox.mcp.populi_reconcile",
                                                        lease_id = %row.lease_id,
                                                        holder_node_id = %row.holder_node_id,
                                                        scope_key = %row.scope_key,
                                                        reason,
                                                        "mesh exec lease auto-revoked",
                                                    );
                                                }
                                                Err(e) => {
                                                    auto_revoke_ok = Some(false);
                                                    tracing::warn!(
                                                        target: "vox.mcp.populi_reconcile",
                                                        lease_id = %row.lease_id,
                                                        holder_node_id = %row.holder_node_id,
                                                        error = %e,
                                                        reason,
                                                        "mesh exec lease auto-revoke failed",
                                                    );
                                                }
                                            }
                                        }
                                        if codex_mesh_telemetry {
                                            if let Some(db) = db_reconcile.as_ref() {
                                                let db = db.clone();
                                                let rid = repo_id_reconcile.clone();
                                                let lease_id = row.lease_id.clone();
                                                let holder_node_id = row.holder_node_id.clone();
                                                let scope_key = row.scope_key.clone();
                                                let expires_unix_ms = row.expires_unix_ms;
                                                let reason_owned = reason.to_string();
                                                let attempted = auto_revoke_exec_leases;
                                                let ok = auto_revoke_ok;
                                                tokio::spawn(async move {
                                                    let details = serde_json::json!({
                                                        "reason": reason_owned,
                                                        "lease_id": lease_id,
                                                        "holder_node_id": holder_node_id,
                                                        "scope_key": scope_key,
                                                        "expires_unix_ms": expires_unix_ms,
                                                        "auto_revoke_attempted": attempted,
                                                        "auto_revoke_ok": ok,
                                                    });
                                                    if let Err(e) = db
                                                        .record_populi_control_event(
                                                            &rid,
                                                            "mesh_exec_lease_reconcile",
                                                            Some(details),
                                                        )
                                                        .await
                                                    {
                                                        tracing::debug!(
                                                            error = %e,
                                                            "record_populi_control_event (mesh_exec_lease_reconcile) failed"
                                                        );
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    error = %e,
                                    "populi exec lease list skipped (unsupported or transport error)"
                                );
                            }
                        }
                    }

                    match snap.write() {
                        Ok(mut w) => {
                            *w = RemotePopuliSnapshot::success(now, f.schema_version, brief);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "populi poll: snapshot lock poisoned")
                        }
                    }
                }
                Err(e) => {
                    let hint_update = orch.set_remote_populi_routing_hints(Vec::new());
                    if hint_update.prev_schedulable > hint_update.new_schedulable {
                        if populi_rebalance_on_remote_schedulable_drop {
                            let n = orch.rebalance();
                            if n > 0 {
                                tracing::debug!(
                                    target: "vox.mcp.populi",
                                    rebalanced = n,
                                    "mesh federation: rebalanced after remote schedulable drop (poll failure)"
                                );
                            }
                        }
                        if populi_replay_queued_routes_on_remote_schedulable_drop {
                            let replayed = orch
                                .replay_queued_routes_after_populi_schedulable_drop()
                                .await;
                            if replayed > 0 {
                                tracing::info!(
                                    target: "vox.mcp.populi",
                                    replayed,
                                    "mesh federation: replayed queued routes after remote schedulable drop (poll failure)"
                                );
                            }
                        }
                    }
                    match snap.write() {
                        Ok(mut w) => {
                            *w = RemotePopuliSnapshot::failure(now, e.to_string());
                        }
                        Err(pe) => {
                            tracing::error!(error = %pe, "populi poll: snapshot lock poisoned")
                        }
                    }
                }
            }
        }
    });
    *guard = Some(handle);
}
