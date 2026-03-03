//! JJ-inspired VCS tool handlers for the Vox MCP server.
//!
//! Covers: snapshots, operation log (oplog), conflicts, workspaces, and change tracking.

use vox_orchestrator::{AgentId, ConflictId, ConflictResolution, OperationId, SnapshotId, TaskId};

use crate::params::ToolResult;
use crate::server::ServerState;

// ---------------------------------------------------------------------------
// Snapshots
// ---------------------------------------------------------------------------

/// List recent snapshots for an agent (async).
pub async fn snapshot_list(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id_val = args.get("agent_id").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let orch = state.orchestrator.lock().await;

    let store = orch.snapshot_store();
    let agent = agent_id_val.map(AgentId);
    let snaps = store.list(agent, limit);

    let items: Vec<serde_json::Value> = snaps
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id.to_string(),
                "agent_id": s.agent_id.to_string(),
                "timestamp_ms": s.timestamp_ms,
                "description": s.description,
                "file_count": s.files.len(),
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({ "snapshots": items })).to_json()
}

/// Show diff between two snapshots (async).
pub async fn snapshot_diff(state: &ServerState, args: serde_json::Value) -> String {
    let before_id = args.get("before").and_then(|v| v.as_u64()).unwrap_or(0);
    let after_id = args.get("after").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = state.orchestrator.lock().await;

    let store = orch.snapshot_store();
    let before = store.get(SnapshotId(before_id));
    let after = store.get(SnapshotId(after_id));

    match (before, after) {
        (Some(b), Some(a)) => {
            let diffs = vox_orchestrator::SnapshotStore::diff(b, a);
            let items: Vec<serde_json::Value> = diffs
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "path": d.path.display().to_string(),
                        "kind": format!("{:?}", d.kind),
                    })
                })
                .collect();
            ToolResult::ok(serde_json::json!({ "diffs": items })).to_json()
        }
        _ => ToolResult::<String>::err("One or both snapshot IDs not found".to_string()).to_json(),
    }
}

/// Restore the workspace to a specific snapshot (async).
pub async fn snapshot_restore(state: &ServerState, args: serde_json::Value) -> String {
    let snapshot_id_str = args
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let snapshot_id = snapshot_id_str
        .strip_prefix("S-")
        .and_then(|s| s.parse::<u64>().ok())
        .map(vox_orchestrator::snapshot::SnapshotId);

    let Some(sid) = snapshot_id else {
        return ToolResult::<String>::err("Invalid snapshot_id format. Expected S-XXXXXX")
            .to_json();
    };

    let orch = state.orchestrator.lock().await;

    match orch.restore_fs_snapshot(sid).await {
        Ok(_) => {
            ToolResult::ok(format!("Workspace restored to snapshot {}", sid)).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("Restore failed: {}", e)).to_json(),
    }
}

// ---------------------------------------------------------------------------
// Operation log
// ---------------------------------------------------------------------------

/// List recent operations from the operation log (async).
pub async fn oplog_list(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id_val = args.get("agent_id").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let orch = state.orchestrator.lock().await;

    let log = orch.oplog();
    let agent = agent_id_val.map(AgentId);
    let entries = log.list(agent, limit);

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id.to_string(),
                "agent_id": e.agent_id.to_string(),
                "timestamp_ms": e.timestamp_ms,
                "kind": format!("{:?}", e.kind),
                "description": e.description,
                "undone": e.undone,
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({ "operations": items })).to_json()
}

/// Undo an operation (async).
pub async fn oplog_undo(state: &ServerState, args: serde_json::Value) -> String {
    let op_id = args
        .get("operation_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut orch = state.orchestrator.lock().await;

    match orch.undo_operation(OperationId(op_id)).await {
        Ok(_) => ToolResult::ok(serde_json::json!({
            "undone": true,
            "operation_id": op_id,
        }))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("Undo failed: {}", e)).to_json(),
    }
}

/// Redo an operation (async).
pub async fn oplog_redo(state: &ServerState, args: serde_json::Value) -> String {
    let op_id = args
        .get("operation_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut orch = state.orchestrator.lock().await;

    match orch.redo_operation(OperationId(op_id)).await {
        Ok(_) => ToolResult::ok(serde_json::json!({
            "redone": true,
            "operation_id": op_id,
        }))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("Redo failed: {}", e)).to_json(),
    }
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

/// List active conflicts (async).
pub async fn conflicts_list(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;

    let mgr = orch.conflict_manager();
    let conflicts = mgr.active_conflicts();

    let items: Vec<serde_json::Value> = conflicts
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id.to_string(),
                "path": c.path.display().to_string(),
                "sides": c.sides.len(),
                "created_ms": c.created_ms,
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({
        "active_conflicts": items,
        "total_active": mgr.active_count(),
    }))
    .to_json()
}

/// Resolve a conflict (async).
pub async fn resolve_conflict(state: &ServerState, args: serde_json::Value) -> String {
    let conflict_id = args
        .get("conflict_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let strategy = args
        .get("strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("take_left");

    let mut orch = state.orchestrator.lock().await;

    let resolution = match strategy {
        "take_right" => ConflictResolution::TakeRight,
        "defer" => {
            let agent_id = args
                .get("defer_to_agent")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            ConflictResolution::DeferToAgent(AgentId(agent_id))
        }
        _ => ConflictResolution::TakeLeft,
    };

    let ok = orch
        .conflict_manager_mut()
        .resolve(ConflictId(conflict_id), resolution);

    if ok {
        ToolResult::ok("Conflict resolved".to_string()).to_json()
    } else {
        ToolResult::<String>::err("Conflict not found or already resolved".to_string()).to_json()
    }
}

// ---------------------------------------------------------------------------
// Workspaces
// ---------------------------------------------------------------------------

/// Create a workspace for an agent (async).
pub async fn workspace_create(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let mut orch = state.orchestrator.lock().await;

    let base_id = orch.snapshot_store_mut().take_snapshot(
        AgentId(agent_id),
        &[],
        "workspace base",
    );

    let ws = orch
        .workspace_manager_mut()
        .create_workspace(AgentId(agent_id), base_id);

    ToolResult::ok(serde_json::json!({
        "workspace_created": true,
        "agent_id": ws.agent_id.to_string(),
        "base_snapshot": base_id.to_string(),
    }))
    .to_json()
}

/// Show workspace status (async).
pub async fn workspace_status(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = state.orchestrator.lock().await;

    match orch
        .workspace_manager()
        .get_workspace(AgentId(agent_id))
    {
        Some(ws) => {
            let paths: Vec<String> = ws
                .modified_paths()
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            ToolResult::ok(serde_json::json!({
                "has_workspace": true,
                "modified_files": paths,
                "modified_count": ws.modified_count(),
                "base_snapshot": ws.base_snapshot.to_string(),
                "active_change": ws.active_change.map(|c| c.to_string()),
            }))
            .to_json()
        }
        None => ToolResult::ok(serde_json::json!({ "has_workspace": false })).to_json(),
    }
}

/// Merge workspace back to main (async).
pub async fn workspace_merge(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let mut orch = state.orchestrator.lock().await;

    match orch
        .workspace_manager_mut()
        .destroy_workspace(AgentId(agent_id))
    {
        Some(ws) => {
            let count = ws.modified_count();
            ToolResult::ok(serde_json::json!({
                "merged": true,
                "files_merged": count,
            }))
            .to_json()
        }
        None => {
            ToolResult::<String>::err("No active workspace for this agent".to_string()).to_json()
        }
    }
}

// ---------------------------------------------------------------------------
// Change tracking
// ---------------------------------------------------------------------------

/// Create a new logical change (async).
pub async fn change_create(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed change");

    let mut orch = state.orchestrator.lock().await;

    let change_id = orch
        .workspace_manager_mut()
        .create_change(AgentId(agent_id), description);

    ToolResult::ok(serde_json::json!({
        "change_id": change_id.to_string(),
        "description": description,
    }))
    .to_json()
}

/// Show history of a change (async).
pub async fn change_log(state: &ServerState, args: serde_json::Value) -> String {
    let change_id = args.get("change_id").and_then(|v| v.as_u64());
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let orch = state.orchestrator.lock().await;

    if let Some(cid) = change_id {
        match orch
            .workspace_manager()
            .get_change(vox_orchestrator::workspace::ChangeId(cid))
        {
            Some(change) => ToolResult::ok(serde_json::json!({
                "change_id": change.id.to_string(),
                "description": change.description,
                "agent_id": change.agent_id.to_string(),
                "status": format!("{:?}", change.status),
                "snapshots": change.snapshots.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "created_ms": change.created_ms,
            }))
            .to_json(),
            None => ToolResult::<String>::err("Change not found".to_string()).to_json(),
        }
    } else {
        let agent = agent_id.map(AgentId);
        let changes = orch.workspace_manager().list_changes(agent, limit);
        let items: Vec<serde_json::Value> = changes
            .iter()
            .map(|c| {
                serde_json::json!({
                    "change_id": c.id.to_string(),
                    "description": c.description,
                    "agent_id": c.agent_id.to_string(),
                    "status": format!("{:?}", c.status),
                    "snapshot_count": c.snapshots.len(),
                })
            })
            .collect();
        ToolResult::ok(serde_json::json!({ "changes": items })).to_json()
    }
}
