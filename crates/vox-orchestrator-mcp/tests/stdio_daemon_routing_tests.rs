//! T2.2 RED tests: `vox mcp`'s stdio server routes tool calls through the
//! single shared `vox-orchestrator-d` daemon instead of a private
//! `ServerState`, and degrades cleanly when no daemon is reachable.
//!
//! These exercise `vox_orchestrator_mcp::daemon_route::call_tool_via_daemon`
//! (the function `crate::server::VoxMcpServer::call_tool` now delegates to)
//! against a REAL TCP daemon spun up the same way
//! `tests/daemon_extra_tests.rs` does — the closest practical proxy for "the
//! stdio server itself now routes through the daemon" without driving actual
//! stdio framing.

use std::sync::Arc;

use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;
use vox_orchestrator::orch_daemon::{self, ExtraDispatch};
use vox_orchestrator_mcp::daemon_extra::McpExtraDispatch;
use vox_orchestrator_mcp::daemon_route::call_tool_via_daemon;
use vox_orchestrator_mcp::{ServerState, load_config};

const D_15S: std::time::Duration = std::time::Duration::from_secs(15);

async fn wait_ready(addr: &str, token: Option<&str>) {
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        let c = match token {
            Some(t) => orch_daemon::OrchDaemonClient::with_token(addr.to_string(), t.to_string()),
            None => orch_daemon::OrchDaemonClient::new(addr.to_string()),
        };
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

/// Serialized-env-mutation guard: points `USERPROFILE` (Windows) / `HOME`
/// (else) at a fresh temp dir so `vox_config::paths::user_home_dir()` — and
/// therefore the well-known daemon token file path both `OrchDaemonClient`
/// and `OrchestratorDaemonEnsure` read — resolves under our isolated temp
/// dir. Callers MUST apply `#[serial_test::serial]` (unnamed — same key as
/// the rest of this crate's env-mutating tests) so no other test races the
/// env var while this guard is alive.
struct IsolatedHomeEnv {
    _tempdir: tempfile::TempDir,
    prev_userprofile: Option<String>,
    prev_home: Option<String>,
    prev_path: Option<String>,
}

impl IsolatedHomeEnv {
    #[allow(unsafe_code)]
    fn new() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let prev_userprofile = std::env::var("USERPROFILE").ok();
        let prev_home = std::env::var("HOME").ok();
        // SAFETY: `#[serial_test::serial]` on every caller serializes env
        // mutation against the rest of this crate's tests.
        unsafe {
            std::env::set_var("USERPROFILE", tempdir.path());
            std::env::set_var("HOME", tempdir.path());
        }
        Self {
            _tempdir: tempdir,
            prev_userprofile,
            prev_home,
            prev_path: None,
        }
    }

    fn home(&self) -> &std::path::Path {
        self._tempdir.path()
    }

    fn write_token(&self, token: &str) {
        let dir = self.home().join(".vox").join("run");
        std::fs::create_dir_all(&dir).expect("create .vox/run");
        std::fs::write(dir.join("orchestrator-daemon.token"), token).expect("write token file");
    }

    /// Also neutralize `PATH` so `which::which("vox-orchestrator-d")` cannot
    /// resolve a REAL installed daemon binary from this dev machine's
    /// `~/.vox/bin` (which is typically on `PATH` outside the isolated home
    /// override above). Used by the "no daemon spawnable" test only — most
    /// tests in this file want the real binary discoverable so they can
    /// exercise a genuine spawn.
    #[allow(unsafe_code)]
    fn neutralize_path(&mut self) {
        self.prev_path = std::env::var("PATH").ok();
        // SAFETY: see `new()`.
        unsafe {
            std::env::set_var("PATH", self.home());
        }
    }
}

impl Drop for IsolatedHomeEnv {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: see `new()`.
        unsafe {
            match &self.prev_userprofile {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
            match &self.prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            if let Some(p) = &self.prev_path {
                std::env::set_var("PATH", p);
            }
        }
    }
}

/// RED test 1 (core acceptance test): a tool call made via
/// `call_tool_via_daemon` (the stdio server's tool-execution path) lands in
/// the SAME daemon-owned `ServerState` a separate client connection sees. A
/// dangerous tool call parks a pending approval; a second, independent
/// `OrchDaemonClient` connection (standing in for "the GUI") lists and
/// resolves it via `orch.list_pending_approvals` / `orch.resolve_approval` —
/// proving cross-client-visible shared state, not two disjoint orchestrators.
#[tokio::test]
#[serial_test::serial]
async fn stdio_routed_tool_call_approval_is_visible_and_resolvable_from_a_separate_client() {
    let home = IsolatedHomeEnv::new();
    let token = "t22-shared-token";
    home.write_token(token);

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
        Some(Arc::from(token)),
    ));
    wait_ready(&addr, Some(token)).await;

    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr);
    }

    // This is the exact call `VoxMcpServer::call_tool` makes: ensure a daemon
    // (adopts our already-running one via the token file) and forward a
    // dangerous tool call to it.
    let daemon_ensure = OrchestratorDaemonEnsure::default();
    let call = tokio::spawn(async move {
        call_tool_via_daemon(
            &daemon_ensure,
            "vox_run_shell",
            serde_json::json!({ "command": "echo hi" }),
        )
        .await
    });

    // A completely separate client connection (standing in for the GUI) lists
    // the pending approval — proving it landed in the daemon's shared state,
    // not some other, invisible ServerState.
    let gui_client = orch_daemon::OrchDaemonClient::with_token(addr.clone(), token.to_string());
    let deadline = tokio::time::Instant::now() + D_15S;
    let mut approvals = serde_json::json!({});
    loop {
        approvals = gui_client
            .call(
                vox_foundation::protocol::orch_daemon_method::LIST_PENDING_APPROVALS,
                serde_json::json!({}),
            )
            .await
            .expect("orch.list_pending_approvals dispatched");
        if approvals
            .get("approvals")
            .and_then(|a| a.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "stdio-routed vox_run_shell call never became visible via a separate client's orch.list_pending_approvals"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let approval_id = approvals["approvals"][0]["approval_id"]
        .as_str()
        .expect("approval_id present")
        .to_string();
    assert_eq!(
        approvals["approvals"][0]["tool"].as_str(),
        Some("vox_run_shell")
    );

    // Resolve it from that same separate client — the "GUI's ApprovalsView"
    // stand-in — proving the approval is genuinely resolvable cross-client,
    // not just visible.
    let resolve_result = gui_client
        .call(
            vox_foundation::protocol::orch_daemon_method::RESOLVE_APPROVAL,
            serde_json::json!({ "approval_id": approval_id, "outcome": "rejected" }),
        )
        .await
        .expect("orch.resolve_approval dispatched");
    assert_eq!(
        resolve_result.get("resolved"),
        Some(&serde_json::Value::Bool(true))
    );

    // The originally stdio-routed call must have woken up and completed.
    let outcome = call
        .await
        .expect("join")
        .expect("call_tool_via_daemon returned a tool envelope");
    assert!(
        vox_orchestrator_mcp::server::tool_json_envelope_is_error(&outcome),
        "rejected approval must surface as an error envelope, got: {outcome}"
    );

    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
    }
    server.abort();
}

/// RED test 2: tool-schema listing works without any live daemon. Mirrors
/// what `crate::server::VoxMcpServer::list_tools` does — build local state
/// and merge the static registry — with no daemon running anywhere and no
/// `VOX_ORCHESTRATOR_DAEMON_SOCKET` pointing at one.
#[tokio::test]
#[serial_test::serial]
async fn tool_schema_listing_works_without_a_live_daemon() {
    let home = IsolatedHomeEnv::new();
    // Deliberately do NOT write a token file or start any daemon.
    let _ = home.home(); // keep the tempdir alive for the duration of the test

    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
    }

    let state = ServerState::new_full(load_config());
    let tools = vox_orchestrator_mcp::registry::merged_tool_registry(&state);
    assert!(
        !tools.is_empty(),
        "tool registry must be non-empty even with no daemon running"
    );
    assert!(
        tools.iter().any(|t| t.name == "vox_git_status"),
        "expected a known static tool in the registry"
    );
}

/// RED test 3: a tool call with no daemon reachable and none spawnable
/// (binary resolution fails because there is no `vox-orchestrator-d` on PATH,
/// in `~/.vox/bin`, or sibling to the test binary under our isolated temp
/// home) surfaces a clear `Err`, not a hang or panic. Uses a short-lived
/// override of `VOX_ORCHESTRATOR_DAEMON_SOCKET` pointed at a port nothing is
/// listening on, combined with an isolated home with no staged binary, so
/// `ensure()` fails fast rather than actually spawning a real daemon process
/// during this test run.
#[tokio::test]
#[serial_test::serial]
async fn tool_call_with_no_daemon_reachable_surfaces_a_clear_error() {
    let mut home = IsolatedHomeEnv::new();
    // No token file (skip the "adopt an already-running daemon" path) AND a
    // neutralized PATH/home so `resolve_managed_binary_path` cannot find this
    // dev machine's real installed `vox-orchestrator-d` — the spawn itself
    // must fail, proving the "none spawnable" branch surfaces a clear error
    // rather than actually spawning a real daemon during this test.
    home.neutralize_path();

    // Point at a bound-but-unlistened loopback port so the initial ping
    // fails fast, and no token file exists so the "already running" adopt
    // path is skipped entirely (matches PersistentDaemon's own behavior).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener); // free the port again so nothing answers there

    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr);
    }

    let daemon_ensure = OrchestratorDaemonEnsure::default();
    let result =
        call_tool_via_daemon(&daemon_ensure, "vox_git_status", serde_json::json!({})).await;

    assert!(
        result.is_err(),
        "tool call must surface a clear error when no daemon is reachable/spawnable, got: {result:?}"
    );

    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
    }
}
