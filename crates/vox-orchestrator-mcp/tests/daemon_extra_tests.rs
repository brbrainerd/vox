//! B5 path-(c) + B3 cross-process: the daemon serves orch.tool_call /
//! orch.resolve_approval / orch.list_pending_approvals via an ExtraDispatch hook
//! carrying the daemon's MCP ServerState.

use std::sync::Arc;

use vox_orchestrator::orch_daemon::{self, ExtraDispatch};
use vox_orchestrator_mcp::daemon_extra::McpExtraDispatch;
use vox_orchestrator_mcp::{ServerState, load_config};

const D_15S: std::time::Duration = std::time::Duration::from_secs(15);

async fn wait_ready(addr: &str) {
    let deadline = tokio::time::Instant::now() + D_15S;
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
        None,
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

/// `research.run` is the cross-process async executor for `vox research run
/// --async`: the persistent daemon creates the session row and spawns the
/// pipeline (fire-and-forget), returning a `running` envelope immediately. The
/// session reaches a terminal `completed`/`failed` status in the background.
#[tokio::test]
async fn daemon_research_run_enqueues_session_via_extra_dispatch() {
    // A DB-attached ServerState so the session row can be created.
    let db = Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("in-memory db"),
    );
    let state = ServerState::new_full(load_config())
        .with_db_initialized(db.clone())
        .await;
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
        None,
    ));
    wait_ready(&addr).await;

    let client = orch_daemon::OrchDaemonClient::new(addr);
    let value = client
        .call(
            vox_foundation::protocol::dei_method::RESEARCH_RUN,
            serde_json::json!({
                "query": "what is the latency trend?",
                "scope": "local",
                "max_sources": 3,
                "verify_claims": false,
            }),
        )
        .await
        .expect("research.run dispatched");

    // Fire-and-forget: returns immediately with a session id and a non-failure
    // status, never an error envelope.
    let session_id = value
        .get("session_id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_else(|| panic!("research.run must return a session_id, got: {value}"));
    assert!(session_id > 0, "session_id must be positive: {value}");
    // A2: the GUI relies on the immediate `running` status to switch into its
    // background-running indicator without blocking on the pipeline.
    assert_eq!(
        value.get("status").and_then(serde_json::Value::as_str),
        Some("running"),
        "research.run must return status=\"running\", got: {value}"
    );
    assert_ne!(
        value.get("success"),
        Some(&serde_json::Value::Bool(false)),
        "research.run should not be a failure envelope, got: {value}"
    );

    // The session row exists in the daemon's DB (real persistence, not a stub).
    let row = db
        .get_research_session(session_id)
        .await
        .expect("query session")
        .expect("session row must exist after research.run");
    assert_eq!(row.id, session_id);

    server.abort();
}

/// T0.3 follow-up review finding (Issue 2): proves `permission_mode` genuinely
/// travels over the wire — `OrchDaemonClient::with_permission_mode` sets
/// `DispatchRequest::permission_mode`, the daemon's `orch.tool_call`
/// `ExtraDispatch` reads `req.permission_mode` and passes it into
/// `handle_tool_call_with_mode`, and that reaches the dispatch gate's
/// auto-approve decision. Same real TCP daemon + `OrchDaemonClient` path the
/// GUI's `invoke_mcp_tool` Tauri command uses (`OrchDaemonClient::call` ->
/// `serve_listener_with_extra` -> `McpExtraDispatch::try_handle`) — this is
/// the closest practical proxy for "the GUI mode toggle measurably changes
/// daemon-side auto-approve behavior" without a full browser E2E harness.
#[tokio::test]
async fn permission_mode_on_the_wire_changes_the_gate_decision() {
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
        None,
    ));
    wait_ready(&addr).await;

    // No permission_mode set (mirrors a client that never selected a mode,
    // e.g. before the GUI toggle is touched): vox_write_file must park.
    let plain_client = orch_daemon::OrchDaemonClient::new(addr.clone());
    let s2 = state.clone();
    let plain_call = tokio::spawn(async move {
        plain_client
            .call(
                vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
                serde_json::json!({
                    "name": "vox_write_file",
                    "args": { "path": "wire-mode-test.txt", "content": "x" }
                }),
            )
            .await
    });
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !s2.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "vox_write_file with no permission_mode on the wire never parked"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert!(state.pending_approvals.resolve(
        &pending[0].approval_id,
        vox_orchestrator::ApprovalOutcome::Rejected
    ));
    plain_call.await.expect("join").expect("dispatch ok");
    assert!(state.pending_approvals.list().is_empty());

    // Same call, but the client sets permission_mode = "accept_edits" on the
    // wire via `with_permission_mode` — the real DispatchRequest field, not
    // a tool-call arg. vox_write_file (mutating + reversible) must now
    // auto-approve: no pending approval should ever appear.
    let mode_client =
        orch_daemon::OrchDaemonClient::new(addr.clone()).with_permission_mode("accept_edits");
    // The call itself may surface an "Unknown tool" / dispatch-level Err
    // once past the gate (vox_write_file isn't routed as a static match arm
    // in this test harness's dispatch table) — that's irrelevant to what
    // this test proves. What matters is that it never went through the
    // park-and-await path, which we assert via pending_approvals below
    // regardless of whether `.call` returned Ok or Err.
    let _ = mode_client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({
                "name": "vox_write_file",
                "args": { "path": "wire-mode-test-2.txt", "content": "y" }
            }),
        )
        .await;
    assert!(
        state.pending_approvals.list().is_empty(),
        "vox_write_file with permission_mode=accept_edits on the wire must auto-approve, not park"
    );

    server.abort();
}
