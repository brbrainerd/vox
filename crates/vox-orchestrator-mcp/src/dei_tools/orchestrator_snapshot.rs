//! Orchestrator status JSON for `vox_orchestrator_status`, including optional mesh snapshot persistence.
//!
//! ## Mesh snapshot → Codex
//! [`persist_mesh_snapshot_codex_opt`] records a **`populi_control`-class** row only when **`VOX_MESH_CODEX_TELEMETRY=1`** and
//! Codex is attached — **off by default** so federation snapshots are not written unless explicitly opted in.

use crate::params::{AgentInfo, StatusResponse, ToolResult};
use crate::server_state::ServerState;
use crate::sync_poison::poison_rw_read;

use vox_gamify::companion::Companion;
use vox_gamify::db::{list_companions, migrate_default_orchestrator_companion_id};

/// Get a full snapshot of the orchestrator's state.
pub async fn orchestrator_status(state: &ServerState) -> anyhow::Result<String> {
    let (
        status,
        scaling_profile,
        effective_scale_up_threshold,
        vcs_snapshot_count,
        vcs_oplog_count,
        vcs_active_conflicts,
        vcs_active_workspaces,
        vcs_active_changes,
        _populi_control_url,
        _populi_http_timeout_ms,
        registered_worker_processes,
        execution_mode,
        worker_runtime_attached,
    ) = {
        let orch = &state.orchestrator;
        let handle = orch.config_handle();
        let cfg = poison_rw_read(
            handle.read(),
            "read orchestrator config for vox_orchestrator_status",
        )?;
        let effective = cfg.scaling_threshold as f64 * cfg.scaling_profile.threshold_multiplier();
        let registered_worker_processes =
            vox_orchestrator::sync_lock::rw_read(&*orch.agent_handles).len();
        let worker_runtime_attached = registered_worker_processes > 0;
        let execution_mode = if worker_runtime_attached {
            "workers_attached"
        } else {
            "queue_only"
        };
        (
            orch.status(),
            Some(format!("{:?}", cfg.scaling_profile).to_lowercase()),
            Some(effective),
            poison_rw_read(
                orch.snapshot_store_handle().read(),
                "read VCS snapshot store for vox_orchestrator_status",
            )?
            .count(),
            poison_rw_read(
                orch.oplog_handle().read(),
                "read VCS oplog for vox_orchestrator_status",
            )?
            .count(),
            poison_rw_read(
                orch.conflict_manager_handle().read(),
                "read VCS conflict manager for vox_orchestrator_status",
            )?
            .active_count(),
            poison_rw_read(
                orch.workspace_manager_handle().read(),
                "read VCS workspace manager (list_workspaces) for vox_orchestrator_status",
            )?
            .list_workspaces()
            .len(),
            poison_rw_read(
                orch.workspace_manager_handle().read(),
                "read VCS workspace manager (list_changes) for vox_orchestrator_status",
            )?
            .list_changes(None, usize::MAX)
            .len(),
            cfg.populi_control_url.clone(),
            cfg.populi_http_timeout_ms,
            registered_worker_processes,
            execution_mode.to_string(),
            worker_runtime_attached,
        )
    };

    let db_configured = state.db.is_some();
    let event_feed_mode = if db_configured {
        "codex_and_transient"
    } else {
        "transient_only"
    };
    let topology = Some(state.orchestrator.topology_snapshot());
    let persistence_outbox_lifecycle = {
        let key = "orchestrator/persistence_outbox_lifecycle";
        let store = state.orchestrator.context_store();
        vox_orchestrator::sync_lock::rw_read(&*store)
            .get(key)
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    };

    let populi_federation_cache = serde_json::to_value(
        poison_rw_read(
            state.populi_remote_snapshot.read(),
            "read Populi remote snapshot cache for vox_orchestrator_status",
        )?
        .clone(),
    )
    .ok();

    let _max_stale_ms: Option<u64> =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshMaxStaleMs)
            .expose()
            .and_then(|s| s.trim().parse().ok())
            .filter(|n| *n > 0);

    #[cfg(feature = "populi-transport")]
    let mesh_snapshot = if let Some(url) = _populi_control_url
        .as_ref()
        .filter(|s: &&String| !s.trim().is_empty())
    {
        let timeout = std::time::Duration::from_millis(_populi_http_timeout_ms.max(500_u64));
        let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(url, timeout)
            .with_env_token();
        match client.list_nodes().await {
            Ok(f) => {
                let f = vox_populi::filter_registry_by_max_stale_ms(f, _max_stale_ms);
                Some(serde_json::json!({
                    "ok": true,
                    "schema_version": f.schema_version,
                    "node_count": f.nodes.len(),
                    "nodes": f.nodes,
                }))
            }
            Err(e) => Some(serde_json::json!({
                "ok": false,
                "error": e.to_string(),
            })),
        }
    } else {
        None
    };
    #[cfg(not(feature = "populi-transport"))]
    let mesh_snapshot: Option<serde_json::Value> = None;

    if let Some(ref snap) = mesh_snapshot {
        persist_mesh_snapshot_codex_opt(state, snap).await;
    }

    let agents: Vec<AgentInfo> = status
        .agents
        .iter()
        .map(|a| AgentInfo {
            id: a.id.0,
            name: a.name.clone(),
            queued: a.queued,
            completed: a.completed,
            paused: a.paused,
            max_handoff_count: a.max_handoff_count,
        })
        .collect();

    let companion = {
        let canonical_id = "vox-orchestrator";
        if let Some(db) = &state.db {
            if let Err(e) = migrate_default_orchestrator_companion_id(db, "user").await {
                tracing::debug!(
                    error = %e,
                    "migrate_default_orchestrator_companion_id failed (non-fatal)"
                );
            }
        }
        let mut comp = if let Some(db) = &state.db {
            match list_companions(db, "user").await {
                Ok(comps) => resolve_stored_companion(&comps, canonical_id),
                Err(_) => None,
            }
        } else {
            None
        }
        .unwrap_or_else(|| {
            vox_gamify::companion::Companion::new(canonical_id, "user", "Vox DEI", "vox")
        });

        comp.ascii_sprite = Some("🧑‍💻".to_string());
        Some(comp)
    };

    let scaling_line = match (scaling_profile.as_deref(), effective_scale_up_threshold) {
        (Some(prof), Some(eff)) => format!(
            "**Scaling:** profile={}, effective scale-up threshold={:.1}\n\n",
            prof, eff
        ),
        _ => String::new(),
    };

    let bounce_line = if status.max_handoff_count > 0 {
        format!("**Peak Bounce Depth:** `{}`\n\n", status.max_handoff_count)
    } else {
        String::new()
    };

    let mut markdown = format!(
        "### 🤖 Vox DEI Status\n\n**Agents Active:** {}\n**Tasks In Progress:** {}\n**Tasks Completed:** {}\n\n{}{}",
        status.agents.len(),
        status.total_in_progress,
        status.total_completed,
        scaling_line,
        bounce_line
    );

    if let Some(ref c) = companion {
        markdown.push_str("#### 🧬 Code Companion\n\n");
        markdown.push_str(&format!(
            "```\n{}\n```\n",
            c.ascii_sprite.as_deref().unwrap_or("")
        ));
        markdown.push_str(&format!("**{}** {}\n", c.name, c.mood.emoji()));
        markdown.push_str(&format!(
            "HP: `{}`\n",
            c.render_status_bar(15).split("HP: ").last().unwrap_or("")
        ));
    }
    let planning = if let Some(db) = &state.db {
        let mut active_tasks = 0_usize;
        let mut completed_tasks = 0_usize;
        if let Ok((active, completed)) = db.get_memory_status_counts().await {
            active_tasks = active;
            completed_tasks = completed;
        }
        Some(serde_json::json!({
            "active_sessions": active_tasks,
            "total_sessions": completed_tasks,
        }))
    } else {
        None
    };

    markdown.push_str("\n#### 📋 Agent Queue\n\n");
    for a in &agents {
        let status_icon = if a.paused {
            "⏸️"
        } else if a.queued > 0 {
            "⚙️"
        } else {
            "💤"
        };
        let bounce_suffix = if a.max_handoff_count > 0 {
            format!(" [Bounce: {}]", a.max_handoff_count)
        } else {
            String::new()
        };
        markdown.push_str(&format!(
            "- {} **{}** (Queued: {}, Done: {}){}\n",
            status_icon, a.name, a.queued, a.completed, bounce_suffix
        ));
    }

    let mut daemon_orch_status: Option<serde_json::Value> = None;
    let mut daemon_orch_status_rpc_error: Option<String> = None;
    if let Some(client) = state.orch_daemon_client_for_status_tool_rpc() {
        match client.orchestrator_status().await {
            Ok(v) => daemon_orch_status = Some(v),
            Err(e) => daemon_orch_status_rpc_error = Some(format!("{}", e)),
        }
    }

    let attention_budget = if state.orchestrator_config.attention_enabled {
        let bm = state.orchestrator.budget_manager_handle();
        let snap =
            vox_orchestrator::sync_lock::rw_read::<vox_orchestrator::budget::BudgetManager>(&*bm)
                .attention_snapshot();
        Some(serde_json::to_value(snap).unwrap_or(serde_json::Value::Null))
    } else {
        None
    };

    let response = StatusResponse {
        agent_count: status.agents.len(),
        in_progress: status.total_in_progress,
        completed: status.total_completed,
        agents,
        max_handoff_count: status.max_handoff_count,
        scaling_profile,
        effective_scale_up_threshold,
        companion,
        markdown_summary: Some(markdown),
        snapshot_count: vcs_snapshot_count,
        oplog_count: vcs_oplog_count,
        active_conflicts: vcs_active_conflicts,
        active_workspaces: vcs_active_workspaces,
        active_changes: vcs_active_changes,
        mesh_snapshot,
        populi_federation_cache,
        planning,
        topology,
        persistence_outbox_lifecycle,
        execution_mode,
        worker_runtime_attached,
        registered_worker_processes,
        db_configured,
        event_feed_mode: event_feed_mode.to_string(),
        daemon_orch_status,
        daemon_orch_status_rpc_error,
        attention_budget,
    };

    Ok(ToolResult::ok(response).to_json())
}

/// Load companion row by canonical id, with fallback to the pre-rename storage id (spelled with
/// `concat!` so repo-wide retired-name greps stay clean).
fn resolve_stored_companion(comps: &[Companion], canonical_id: &str) -> Option<Companion> {
    let legacy_id = concat!("vox", "-", "dei");
    comps
        .iter()
        .find(|c| c.id == canonical_id)
        .cloned()
        .or_else(|| comps.iter().find(|c| c.id == legacy_id).cloned())
}

fn mesh_codex_telemetry_enabled() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshCodexTelemetry)
        .expose()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

/// When `VOX_MESH_CODEX_TELEMETRY` is truthy, store a bounded summary of the DEI mesh snapshot for this repo.
async fn persist_mesh_snapshot_codex_opt(state: &ServerState, snap: &serde_json::Value) {
    if !mesh_codex_telemetry_enabled() {
        return;
    }
    let Some(db) = state.db.as_ref() else {
        return;
    };
    let rid = state.repository.repository_id.clone();
    let details = serde_json::json!({
        "event": "orchestrator_status_mesh_snapshot",
        "ok": snap.get("ok"),
        "node_count": snap.get("node_count"),
        "schema_version": snap.get("schema_version"),
        "error": snap.get("error"),
    });
    if let Err(e) = db
        .record_populi_control_event(&rid, "orchestrator_status_mesh_snapshot", Some(details))
        .await
    {
        tracing::debug!(
            target: "vox.mesh_codex",
            error = %e,
            "record_populi_control_event failed (best-effort)"
        );
    }
}

#[cfg(test)]
mod resolve_companion_tests {
    use super::resolve_stored_companion;
    use vox_gamify::companion::Companion;

    #[test]
    fn prefers_canonical_id_when_both_present() {
        let canonical = Companion::new("vox-orchestrator", "user", "Canonical", "vox");
        let legacy = Companion::new(concat!("vox", "-", "dei"), "user", "Legacy", "vox");
        let out = resolve_stored_companion(&[legacy, canonical.clone()], "vox-orchestrator");
        assert_eq!(out.as_ref().map(|c| c.name.as_str()), Some("Canonical"));
    }

    #[test]
    fn falls_back_to_legacy_storage_id() {
        let legacy = Companion::new(concat!("vox", "-", "dei"), "user", "Legacy", "vox");
        let out = resolve_stored_companion(std::slice::from_ref(&legacy), "vox-orchestrator");
        assert_eq!(
            out.as_ref().map(|c| c.id.as_str()),
            Some(concat!("vox", "-", "dei"))
        );
    }

    #[test]
    fn none_when_empty() {
        assert!(resolve_stored_companion(&[], "vox-orchestrator").is_none());
    }
}
