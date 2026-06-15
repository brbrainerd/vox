use vox_orchestrator::json_vcs_facade;

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_SNAPSHOT_PAIR: &str = "List snapshots with `snapshot_list` and pass valid numeric `before`/`after` ids that exist in the store.";
const REM_SNAPSHOT_ID: &str = "Pass `snapshot_id` as `S-XXXXXX` from `snapshot_list`.";
const REM_SNAPSHOT_RESTORE: &str =
    "Verify the snapshot exists on disk and the workspace allows restore operations.";

/// List recent snapshots for an agent (async).
pub async fn snapshot_list(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id_val = args.get("agent_id").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let orch = &state.orchestrator;

    let v = json_vcs_facade::snapshot_list_json(orch, agent_id_val, limit);
    ToolResult::ok(v).to_json()
}

/// Show diff between two snapshots (async).
pub async fn snapshot_diff(state: &ServerState, args: serde_json::Value) -> String {
    let before_id = args.get("before").and_then(|v| v.as_u64()).unwrap_or(0);
    let after_id = args.get("after").and_then(|v| v.as_u64()).unwrap_or(0);

    let orch = &state.orchestrator;

    let v = json_vcs_facade::snapshot_diff_json(orch, before_id, after_id);
    if v.get("error").is_some() {
        return ToolResult::<String>::err_with_remediation(
            "One or both snapshot IDs not found".to_string(),
            REM_SNAPSHOT_PAIR,
        )
        .to_json();
    }
    ToolResult::ok(v).to_json()
}

/// Restore the workspace to a specific snapshot (async).
pub async fn snapshot_restore(state: &ServerState, args: serde_json::Value) -> String {
    let snapshot_id_str = args
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let orch = &state.orchestrator;

    match json_vcs_facade::snapshot_restore_json(orch, snapshot_id_str).await {
        Ok(v) => ToolResult::ok(v).to_json(),
        Err(e) if e.contains("invalid snapshot_id") => {
            ToolResult::<String>::err_with_remediation(e, REM_SNAPSHOT_ID).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_SNAPSHOT_RESTORE).to_json(),
    }
}
