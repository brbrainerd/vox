//! B5 integration coverage for the in-process MCP bridge.
//!
//! Builds a real `ServerState` and dispatches a read-only tool, asserting a
//! non-error envelope. `ServerState::new_full` constructs a full orchestrator
//! (and spawns background pollers / reads `Vox.toml` relative to CWD), so this
//! is gated behind `#[ignore]` to avoid a flaky, environment-dependent test in
//! the default `cargo test` run. The compile path plus the already-tested
//! `dispatch::handle_tool_call` cover the wiring; run with
//! `cargo test -p vox-gui -- --ignored` inside a repo with a `Vox.toml`.

use std::sync::Arc;

use vox_db::{DbConnectSurface, connect_workspace_journey_optional};
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, load_config};

#[tokio::test]
#[ignore = "ServerState::new_full spawns orchestrator pollers and reads Vox.toml from CWD; run explicitly with --ignored"]
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
