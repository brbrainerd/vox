//! JSON payloads shared by MCP VCS tools and `vox dei` CLI (parity surface).

use serde_json::{Value, json};

use crate::Orchestrator;
use crate::snapshot::SnapshotId;
use crate::types::AgentId;

/// List recent snapshots (same shape as MCP `vox_snapshot_list`).
pub fn snapshot_list_json(orch: &Orchestrator, agent_id: Option<u64>, limit: usize) -> Value {
    let agent = agent_id.map(AgentId);
    let handle = orch.snapshot_store_handle();
    let store = crate::sync_lock::rw_read(&*handle);
    let snaps = store.list(agent, limit);
    let items: Vec<Value> = snaps
        .iter()
        .map(|s| {
            json!({
                "id": s.id.to_string(),
                "agent_id": s.agent_id.0.to_string(),
                "timestamp_ms": s.timestamp_ms,
                "description": s.description,
                "file_count": s.files.len(),
            })
        })
        .collect();
    json!({ "snapshots": items })
}

/// Diff two snapshots by numeric id (same shape as MCP `vox_snapshot_diff`).
pub fn snapshot_diff_json(orch: &Orchestrator, before_id: u64, after_id: u64) -> Value {
    let handle = orch.snapshot_store_handle();
    let store = crate::sync_lock::rw_read(&*handle);
    let before = store.get(SnapshotId(before_id)).cloned();
    let after = store.get(SnapshotId(after_id)).cloned();
    match (before, after) {
        (Some(b), Some(a)) => {
            let diffs = crate::snapshot::SnapshotStore::diff(&b, &a);
            let items: Vec<Value> = diffs
                .iter()
                .map(|d| {
                    json!({
                        "path": d.path.display().to_string(),
                        "kind": format!("{:?}", d.kind),
                    })
                })
                .collect();
            json!({ "diffs": items })
        }
        _ => json!({
            "error": "one_or_both_snapshots_missing",
            "before": before_id,
            "after": after_id,
        }),
    }
}

/// Restore filesystem state from a snapshot (`S-` prefix optional in `snapshot_id_str`).
pub async fn snapshot_restore_json(
    orch: &Orchestrator,
    snapshot_id_str: &str,
) -> Result<Value, String> {
    let sid = snapshot_id_str
        .strip_prefix("S-")
        .unwrap_or(snapshot_id_str)
        .parse::<u64>()
        .map(SnapshotId)
        .map_err(|_| "invalid snapshot_id: expected numeric or S-<digits>".to_string())?;
    orch.restore_fs_snapshot(sid)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "restored": true,
        "snapshot_id": sid.to_string(),
    }))
}

/// Create agent workspace (MCP `vox_workspace_create`).
pub fn workspace_create_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let snap_handle = orch.snapshot_store_handle();
    let base_id = {
        let mut store = crate::sync_lock::rw_write(&*snap_handle);
        store.take_snapshot(AgentId(agent_id), &[], "workspace base".to_string())
    };
    let ws_handle = orch.workspace_manager_handle();
    let mut mgr = crate::sync_lock::rw_write(&*ws_handle);
    let ws = mgr.create_workspace(AgentId(agent_id), base_id).clone();
    json!({
        "workspace_created": true,
        "agent_id": ws.agent_id.to_string(),
        "base_snapshot": base_id.to_string(),
    })
}

/// Workspace status (MCP `vox_workspace_status`).
pub fn workspace_status_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let ws_handle = orch.workspace_manager_handle();
    let mgr = crate::sync_lock::rw_read(&*ws_handle);
    match mgr.get_workspace(AgentId(agent_id)) {
        Some(ws) => {
            let paths: Vec<String> = ws
                .modified_paths()
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            json!({
                "has_workspace": true,
                "modified_files": paths,
                "modified_count": ws.modified_count(),
                "base_snapshot": ws.base_snapshot.to_string(),
                "active_change": ws.active_change.map(|c| c.to_string()),
            })
        }
        None => json!({ "has_workspace": false }),
    }
}

/// Merge workspace into mainline (MCP `vox_workspace_merge`).
/// Records overlap conflicts (as data) before destroying the workspace.
pub fn workspace_merge_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let merging = AgentId(agent_id);
    let ws_handle = orch.workspace_manager_handle();
    let mut mgr = crate::sync_lock::rw_write(&*ws_handle);

    // Detect overlaps against every other active workspace, before mutating.
    let merging_base = mgr.get_workspace(merging).map(|w| w.base_snapshot);
    let others: Vec<(AgentId, SnapshotId, Vec<std::path::PathBuf>)> = mgr
        .list_workspaces()
        .iter()
        .filter(|w| w.agent_id != merging)
        .map(|w| {
            (
                w.agent_id,
                w.base_snapshot,
                mgr.overlapping_paths(merging, w.agent_id),
            )
        })
        .filter(|(_, _, paths)| !paths.is_empty())
        .collect();

    let conflicts_recorded = if let Some(base) = merging_base {
        // LOCK ORDER: workspace_manager → conflict_manager (always acquire in this
        // order; the `mgr` write lock above is still held here). Any future code that
        // touches both locks MUST follow this order to avoid an ABBA deadlock.
        let mut cm = crate::sync_lock::rw_write(&*orch.conflict_manager);
        crate::merge_conflicts::record_overlap_conflicts(&mut cm, (merging, base), &others).len()
    } else {
        0
    };

    match mgr.destroy_workspace(merging) {
        Some(ws) => {
            let count = ws.modified_count();
            json!({
                "merged": true,
                "files_merged": count,
                "conflicts_recorded": conflicts_recorded,
            })
        }
        None => json!({
            "merged": false,
            "error": "no_active_workspace",
            "conflicts_recorded": 0,
        }),
    }
}

/// Recent oplog entries (MCP `vox_oplog`).
pub async fn oplog_list_json(orch: &Orchestrator, agent_id: Option<u64>, limit: usize) -> Value {
    let agent = agent_id.map(AgentId);
    let ops = orch.list_recent_operations(agent, limit).await;
    let items: Vec<Value> = ops
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id.to_string(),
                "agent_id": e.agent_id.0.to_string(),
                "timestamp_ms": e.timestamp_ms,
                "kind": format!("{:?}", e.kind),
                "description": e.description,
                "undone": e.undone,
            })
        })
        .collect();
    json!({ "operations": items })
}

/// Single JSON bundle for human handoff: repo identity + workspace + short snapshot/oplog tails.
/// CLI: `vox dei takeover-status`; mirrors fields agents need alongside MCP tool calls.
pub async fn takeover_handoff_json(
    orch: &Orchestrator,
    repo_root_display: &str,
    repository_id: &str,
    agent_id: u64,
) -> Value {
    json!({
        "schema": "vox_takeover_handoff_v1",
        "schema_version": 1,
        "repository": {
            "root": repo_root_display,
            "repository_id": repository_id,
        },
        "agent_id": agent_id,
        "workspace": workspace_status_json(orch, agent_id),
        "snapshots": snapshot_list_json(orch, Some(agent_id), 5),
        "oplog": oplog_list_json(orch, Some(agent_id), 5).await,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Orchestrator;
    use crate::config::OrchestratorConfig;

    #[test]
    fn snapshot_list_json_empty_store() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = snapshot_list_json(&orch, None, 5);
        assert_eq!(v["snapshots"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn workspace_status_json_no_workspace() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = workspace_status_json(&orch, 0);
        assert_eq!(v["has_workspace"], false);
    }

    #[tokio::test]
    async fn takeover_handoff_json_has_core_keys() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = takeover_handoff_json(&orch, "/repo", "rid", 1).await;
        assert_eq!(v["schema"], "vox_takeover_handoff_v1");
        assert!(v.get("repository").is_some());
        assert!(v.get("workspace").is_some());
        assert!(v.get("snapshots").is_some());
        assert!(v.get("oplog").is_some());
    }
}
