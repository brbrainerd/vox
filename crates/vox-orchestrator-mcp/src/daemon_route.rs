//! T2.2: stdio MCP server tool-call routing to the shared `vox-orchestrator-d`.
//!
//! `run_stdio_server_blocking` (`crate::lifecycle`) used to dispatch tool
//! calls against a local, private `ServerState` — a completely disjoint
//! orchestrator from the one `vox-orchestrator-d` (and every daemon client,
//! e.g. the GUI) shares. That meant an approval parked by a `vox mcp` client
//! was invisible to the GUI's Approvals view, and vice versa (the same
//! split-brain T2.1 eliminated for the GUI).
//!
//! [`crate::daemon_route::call_tool_via_daemon`] forwards each tool call over TCP to the daemon's
//! `orch.tool_call` method (served by `McpExtraDispatch`, see
//! `crate::daemon_extra`), spawning the daemon first if none is reachable
//! (via [`vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure`]).
//! Protocol-level concerns (tool-schema listing, resources, prompts) stay
//! local — see `crate::server::VoxMcpServer` — since they're static data that
//! doesn't require a live daemon connection.

use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;
use vox_foundation::protocol::orch_daemon_method;

/// Forward one tool call to the shared orchestrator daemon's `orch.tool_call`
/// method, spawning/ensuring the daemon is reachable first.
///
/// Returns the tool's raw JSON envelope (mirrors `crate::handle_tool_call`'s
/// `Ok(String)` shape) on success. On daemon-unreachable or dispatch-level
/// failure, returns `Err` with a human-readable message — `call_tool` in
/// `crate::server` wraps this in a `ToolResult` error envelope with
/// `REM_DAEMON_UNREACHABLE` remediation, rather than hanging or panicking.
pub async fn call_tool_via_daemon(
    daemon: &OrchestratorDaemonEnsure,
    name: &str,
    args: serde_json::Value,
) -> anyhow::Result<String> {
    let client = daemon
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("could not reach or spawn vox-orchestrator-d: {e}"))?;

    let value = client
        .call(
            orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": name, "args": args }),
        )
        .await
        .map_err(|e| anyhow::anyhow!("tool '{name}' failed via orch.tool_call: {e}"))?;

    Ok(value.to_string())
}
