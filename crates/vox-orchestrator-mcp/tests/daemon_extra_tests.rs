//! B5 path-(c) + B3 cross-process: the daemon serves orch.tool_call /
//! orch.resolve_approval / orch.list_pending_approvals via an ExtraDispatch hook
//! carrying the daemon's MCP ServerState.

use std::sync::Arc;

use vox_orchestrator::orch_daemon::{self, ExtraDispatch};
use vox_orchestrator_mcp::daemon_extra::McpExtraDispatch;
use vox_orchestrator_mcp::{ServerState, load_config};

async fn wait_ready(addr: &str) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let c = orch_daemon::OrchDaemonClient::new(addr.to_string());
        if c.ping().await.is_ok() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "daemon never became ready"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn daemon_tool_call_runs_readonly_tool_via_extra_dispatch() {
    let state = ServerState::new_full(load_config());
    let orch = state.orchestrator.clone();
    let extra: Arc<dyn ExtraDispatch> = Arc::new(McpExtraDispatch::new(state.clone()));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(orch_daemon::serve_listener_with_extra(
        listener,
        addr.clone(),
        "ut-repo".to_string(),
        orch,
        Some(extra),
    ));
    wait_ready(&addr).await;

    let client = orch_daemon::OrchDaemonClient::new(addr);
    let value = client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_git_status", "args": {} }),
        )
        .await
        .expect("orch.tool_call dispatched");

    // The read-only tool's envelope must not be a failure.
    assert_ne!(
        value.get("success"),
        Some(&serde_json::Value::Bool(false)),
        "vox_git_status via orch.tool_call should succeed, got: {value}"
    );

    server.abort();
}
