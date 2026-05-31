//! B5: in-process MCP tool execution for the Vox GUI.
//!
//! The GUI used to surface MCP tools as a dead-end ("MCP-only and not
//! executable via the GUI sidecar", see `ui/src/transport.ts`). This module
//! wires a Tauri command that dispatches a tool call directly against a cached
//! [`ServerState`], so the GUI can run read/write MCP tools in-process without
//! spawning a separate stdio server.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::OnceCell;

use vox_db::{DbConnectSurface, connect_workspace_journey_optional};
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, load_config};

/// Tauri-managed holder for the lazily-built, reused [`ServerState`].
///
/// `ServerState` owns an orchestrator plus background pollers, so it must be
/// built exactly once and shared across calls — never per invocation. The
/// [`OnceCell`] guarantees the heavy construction happens on the first call and
/// is reused thereafter.
#[derive(Default)]
pub struct McpToolHost {
    state: OnceCell<ServerState>,
}

impl McpToolHost {
    /// Build (or return the cached) [`ServerState`]. The first caller pays the
    /// construction cost; subsequent callers reuse the same instance.
    pub async fn get_or_init(&self) -> &ServerState {
        self.state
            .get_or_init(|| async {
                let mut state = ServerState::new_full(load_config());
                if let Some(db) =
                    connect_workspace_journey_optional(DbConnectSurface::Mcp, false).await
                {
                    state = state.with_db_initialized(Arc::new(db)).await;
                }
                state
            })
            .await
    }
}

/// Execute an MCP tool in-process and return the parsed result envelope.
///
/// The returned object always carries an `is_error` flag (derived via
/// [`tool_json_envelope_is_error`]) alongside the tool's own `result` payload.
/// Dispatch-level failures are surfaced as the `Err` arm (a human-readable
/// string) rather than panicking.
#[tauri::command]
pub async fn invoke_mcp_tool(
    tool: String,
    args: Value,
    host: tauri::State<'_, McpToolHost>,
) -> Result<Value, String> {
    let state = host.get_or_init().await;

    let raw = handle_tool_call(state, &tool, args)
        .await
        .map_err(|e| format!("MCP tool '{tool}' failed: {e}"))?;

    let is_error = tool_json_envelope_is_error(&raw);
    let result = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({ "raw": raw }));

    Ok(json!({
        "tool": tool,
        "is_error": is_error,
        "result": result,
    }))
}
