use vox_orchestrator::json_vcs_facade;

use super::parse::parse_operation_id_value;
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_OPLOG_ID: &str = "Pass `operation_id` as a number or `OP-XXXXXX` from `oplog_list`.";
const REM_OPLOG_UNDO: &str =
    "Verify the operation exists, is not already undone, and orchestrator VCS state is healthy.";

/// List recent operations from the operation log (async).
pub async fn oplog_list(state: &ServerState, args: serde_json::Value) -> String {
    let agent_id_val = args.get("agent_id").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let orch = &state.orchestrator;
    let v = json_vcs_facade::oplog_list_json(orch, agent_id_val, limit).await;
    ToolResult::ok(v).to_json()
}

/// Undo an operation (async).
pub async fn oplog_undo(state: &ServerState, args: serde_json::Value) -> String {
    let Some(op_id) = parse_operation_id_value(args.get("operation_id")) else {
        return ToolResult::<String>::err_with_remediation(
            "Missing or invalid operation_id (number or OP-XXXXXX string)".to_string(),
            REM_OPLOG_ID,
        )
        .to_json();
    };

    let orch = &state.orchestrator;

    match orch.undo_operation(op_id).await {
        Ok(_) => ToolResult::ok(serde_json::json!({
            "undone": true,
            "operation_id": op_id.0,
        }))
        .to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("Undo failed: {}", e),
            REM_OPLOG_UNDO,
        )
        .to_json(),
    }
}

/// Redo an operation (async).
pub async fn oplog_redo(state: &ServerState, args: serde_json::Value) -> String {
    let Some(op_id) = parse_operation_id_value(args.get("operation_id")) else {
        return ToolResult::<String>::err_with_remediation(
            "Missing or invalid operation_id (number or OP-XXXXXX string)".to_string(),
            REM_OPLOG_ID,
        )
        .to_json();
    };

    let orch = &state.orchestrator;

    match orch.redo_operation(op_id).await {
        Ok(_) => ToolResult::ok(serde_json::json!({
            "redone": true,
            "operation_id": op_id.0,
        }))
        .to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("Redo failed: {}", e),
            REM_OPLOG_UNDO,
        )
        .to_json(),
    }
}
