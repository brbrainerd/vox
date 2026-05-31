//! B5 integration coverage for the in-process MCP bridge.
//!
//! Builds a real `ServerState` (the same `new_full` + optional-DB path the GUI's
//! `McpToolHost` uses) and dispatches the read-only `vox_git_status` tool,
//! asserting a non-error envelope. This exercises the full
//! `dispatch::handle_tool_call` pipeline end-to-end, not just the type wiring.
//! `vox_git_status` is deterministic (local git in the checkout) and the
//! orchestrator's background pollers are fire-and-forget, so the test is stable
//! in a normal `cargo test` run.

use std::sync::Arc;

use vox_db::{DbConnectSurface, connect_workspace_journey_optional};
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, load_config};

#[tokio::test]
async fn invokes_read_only_tool_with_non_error_envelope() {
    let mut state = ServerState::new_full(load_config());
    if let Some(db) = connect_workspace_journey_optional(DbConnectSurface::Mcp, false).await {
        state = state.with_db_initialized(Arc::new(db)).await;
    }

    let raw = handle_tool_call(&state, "vox_git_status", serde_json::json!({}))
        .await
        .expect("read-only git status dispatch should not error");

    assert!(
        !tool_json_envelope_is_error(&raw),
        "expected non-error envelope, got: {raw}"
    );
}
