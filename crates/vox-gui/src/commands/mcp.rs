//! B5: MCP tool execution for the Vox GUI.
//!
//! Tool calls are dispatched against the single persistent orchestrator daemon
//! (see [`crate::commands::daemon::PersistentDaemon`]) via its `orch.tool_call`
//! method. Routing every call through one shared daemon keeps tool execution,
//! HITL approval gates, and their later resolves inside the same `ServerState`
//! process — previously each one-shot stdio daemon was a distinct process, so a
//! parked dangerous-tool approval could not be resolved.

use std::sync::Arc;

use serde_json::Value;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;

/// Execute an MCP tool against the persistent daemon and return the parsed
/// result envelope.
///
/// The returned object carries an `is_error` flag (true when the daemon's
/// payload reports `success: false`) alongside the tool's own `result` value.
/// Dispatch-level failures surface as the `Err` arm (a human-readable string).
///
/// `permission_mode` (T0.3, `"ask" | "accept_edits" | "accept_all" | "plan"`)
/// is the GUI's selected `PermissionMode`, set on the outgoing
/// `DispatchRequest`'s own top-level field via
/// [`OrchDaemonClient::with_permission_mode`] — NEVER folded into `args`,
/// which is caller/tool-composed JSON the LLM agent can influence. Omitted
/// or `None` leaves the client's mode unset, which the daemon's dispatch
/// gate treats as the fail-safe `ask` default (today's always-park
/// behavior for dangerous tools).
#[tauri::command]
pub async fn invoke_mcp_tool(
    tool: String,
    args: Value,
    permission_mode: Option<String>,
    daemon: tauri::State<'_, Arc<crate::commands::daemon::PersistentDaemon>>,
) -> Result<Value, String> {
    let addr = daemon.ensure().await?;
    let mut client = match daemon.token().await {
        Some(token) => OrchDaemonClient::with_token(addr, token),
        None => OrchDaemonClient::new(addr),
    };
    if let Some(mode) = permission_mode {
        client = client.with_permission_mode(mode);
    }
    let value = client
        .call(
            orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": tool, "args": args }),
        )
        .await
        .map_err(|e| format!("MCP tool '{tool}' failed: {e}"))?;

    let is_error = value.get("success") == Some(&serde_json::Value::Bool(false));

    Ok(serde_json::json!({
        "tool": tool,
        "is_error": is_error,
        "result": value,
    }))
}
