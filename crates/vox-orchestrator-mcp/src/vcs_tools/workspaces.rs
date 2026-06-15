use vox_orchestrator::json_vcs_facade;

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_WORKSPACE_NONE: &str = "Create a workspace with `workspace_create` or verify `agent_id` matches an agent with an active workspace.";

/// Create a workspace for an agent (async).
pub async fn workspace_create(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = &state.orchestrator;
    let v = json_vcs_facade::workspace_create_json(orch, agent_id);
    ToolResult::ok(v).to_json()
}

/// Show workspace status (async).
pub async fn workspace_status(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = &state.orchestrator;

    let v = json_vcs_facade::workspace_status_json(orch, agent_id);
    ToolResult::ok(v).to_json()
}

/// Merge workspace back to main (async).
pub async fn workspace_merge(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id = args.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = &state.orchestrator;

    let v = json_vcs_facade::workspace_merge_json(orch, agent_id);
    if v.get("merged") == Some(&serde_json::Value::Bool(false)) {
        return ToolResult::<String>::err_with_remediation(
            "No active workspace for this agent".to_string(),
            REM_WORKSPACE_NONE,
        )
        .to_json();
    }
    ToolResult::ok(v).to_json()
}
