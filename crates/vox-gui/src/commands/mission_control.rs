//! Tauri commands for the Mission Control surface.
//!
//! Exposes:
//! - `list_subagent_tree`   — delegation bindings from the orchestrator topology
//! - `list_mc_approvals`    — pending HITL approvals (delegates to existing orch method)
//! - `set_task_mesh_policy` — update `mesh_policy` on a queued task

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;

use crate::commands::daemon::PersistentDaemon;

async fn call_orchestrator_daemon(
    daemon: &PersistentDaemon,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => OrchDaemonClient::with_token(addr, token),
        None => OrchDaemonClient::new(addr),
    };
    client.call(method, params).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Subagent tree
// ---------------------------------------------------------------------------

/// One edge in the subagent delegation tree returned by `list_subagent_tree`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SubagentTreeNode {
    pub task_id: u64,
    pub agent_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<u64>,
    pub reason: String,
    /// Chat session that originated this delegation (Phase D Task D1/D3), when
    /// the spawn happened inside a chat turn. `None` for spawns with no chat
    /// origin (e.g. scaling/handoff spawns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_session_id: Option<String>,
    /// Provider tool-call id of the spawn request, for correlating this edge
    /// back to the exact turn (Phase D Task D1/D3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_turn_id: Option<String>,
}

/// Returns the current subagent delegation tree from the orchestrator.
/// The daemon serves this via `orch.subagent_tree` (ExtraDispatch hook).
#[tauri::command]
pub async fn list_subagent_tree(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<Vec<SubagentTreeNode>, String> {
    let resp = call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::SUBAGENT_TREE,
        serde_json::json!({}),
    )
    .await?;
    let nodes = resp
        .get("tree")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(nodes)
}

// ---------------------------------------------------------------------------
// Approvals (delegates to the existing LIST_PENDING_APPROVALS method)
// ---------------------------------------------------------------------------

/// Raw approval row forwarded from the orchestrator's PendingApprovals registry.
#[derive(Debug, Serialize, Deserialize)]
pub struct McApprovalRow {
    pub approval_id: String,
    pub tool: String,
    pub summary: String,
    pub requested_at_ms: i64,
}

/// Returns pending HITL approvals from the orchestrator daemon.
/// The GUI Approvals surface already calls this via MCP; this Tauri command
/// provides a direct path for MissionControlPanel without an extra MCP hop.
#[tauri::command]
pub async fn list_mc_approvals(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<Vec<McApprovalRow>, String> {
    let resp = call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::LIST_PENDING_APPROVALS,
        serde_json::json!({}),
    )
    .await?;
    let rows = resp
        .get("approvals")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Mesh policy
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetMeshPolicyInput {
    pub task_id: u64,
    /// `"any"` | `"local_only"` | `{"exclude": ["node-id", ...]}`
    pub policy: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct MeshPolicyResult {
    pub ok: bool,
    pub message: String,
}

/// Updates the `mesh_policy` of a queued task in the orchestrator.
/// The daemon serves `orch.set_mesh_policy` via ExtraDispatch.
#[tauri::command]
pub async fn set_task_mesh_policy(
    input: SetMeshPolicyInput,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<MeshPolicyResult, String> {
    let resp = call_orchestrator_daemon(
        &daemon,
        orch_daemon_method::SET_MESH_POLICY,
        serde_json::json!({
            "task_id": input.task_id,
            "policy": input.policy,
        }),
    )
    .await?;
    let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let message = resp
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or(if ok { "ok" } else { "error" })
        .to_string();
    Ok(MeshPolicyResult { ok, message })
}
