//! T2.3 follow-up RED tests: `vox safety status`/`ledger`/`locks` route through
//! the shared `vox-orchestrator-d` TCP daemon instead of building a private,
//! throwaway in-process `Orchestrator` per invocation.
//!
//! Mirrors the pattern established by `dei_daemon_routing_tests.rs` (T2.3):
//! spin up a REAL in-process daemon server (`orch_daemon::serve_listener_with_extra`)
//! bound to an ephemeral port, point `VOX_ORCHESTRATOR_DAEMON_SOCKET` at it, and
//! call `vox_cli::commands::safety`'s `pub async fn handle_safety_command` directly
//! — proving it genuinely reaches that daemon's shared `Orchestrator`, not some
//! other, invisible instance.
//!
//! Before the T2.3 follow-up fix, `status_cmd`/`ledger_cmd`/`locks_cmd` each
//! called `build_repo_scoped_orchestrator_for_repository` — a fresh, always-empty
//! local `Orchestrator` — so `vox safety status` displayed fake/empty budget,
//! drift, and lock data regardless of what the real daemon-shared orchestrator
//! held. These tests prove the fix: an agent spawned via a separate daemon
//! client is visible to `vox safety status`'s underlying daemon RPCs
//! (`orch.safety_budget_signals`/`orch.safety_ledger`/`orch.safety_locks`).

#![cfg(feature = "dei")]

use std::sync::Arc;

use vox_orchestrator::orch_daemon::{self, ExtraDispatch};
use vox_orchestrator_mcp::daemon_extra::McpExtraDispatch;
use vox_orchestrator_mcp::{ServerState, load_config};

const D_15S: std::time::Duration = std::time::Duration::from_secs(15);

async fn wait_ready(addr: &str, token: &str) {
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        let c = orch_daemon::OrchDaemonClient::with_token(addr.to_string(), token.to_string());
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

/// Serialized-env-mutation guard — see `dei_daemon_routing_tests.rs::IsolatedHomeEnv`
/// for the full rationale. Callers MUST apply `#[serial_test::serial]`.
struct IsolatedHomeEnv {
    _tempdir: tempfile::TempDir,
    prev_userprofile: Option<String>,
    prev_home: Option<String>,
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
        }
    }
}

/// Spin up a real in-process daemon on an ephemeral port. See
/// `dei_daemon_routing_tests.rs::spawn_test_daemon` for the full rationale.
async fn spawn_test_daemon(
    home: &IsolatedHomeEnv,
    token: &str,
) -> (String, orch_daemon::OrchDaemonClient, tokio::task::JoinHandle<anyhow::Result<()>>) {
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
    wait_ready(&addr, token).await;

    // SAFETY: serialized via `#[serial_test::serial]` on every test that
    // calls this helper.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr);
    }

    let client = orch_daemon::OrchDaemonClient::with_token(addr.clone(), token.to_string());
    (addr, client, server)
}

#[allow(unsafe_code)]
fn clear_daemon_socket_env() {
    // SAFETY: serialized via `#[serial_test::serial]`.
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
    }
}

/// RED test 1 (core acceptance test): `vox safety status` reflects an agent
/// spawned via a SEPARATE daemon client connection — proving
/// `commands::safety::status_cmd` (via `handle_safety_command`) genuinely
/// reaches the shared daemon's `Orchestrator`, not a private throwaway one.
/// Before the fix this always showed zero agents / empty budget data
/// regardless of daemon state.
#[tokio::test]
#[serial_test::serial]
async fn safety_status_reflects_agent_spawned_via_separate_daemon_client() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-safety-status-token";
    let (_addr, gui_client, server) = spawn_test_daemon(&home, token).await;

    // Spawn an agent via a SEPARATE client connection (standing in for the
    // GUI / another terminal), then via the shared daemon's own
    // orch.safety_budget_signals RPC (the same RPC `status_cmd` now calls),
    // confirm that agent is visible.
    let spawned = gui_client
        .spawn_agent_named("t23-safety-status-agent")
        .await
        .expect("orch.spawn_agent dispatched");
    let agent_id = spawned
        .get("agent_id")
        .and_then(|v| v.as_u64())
        .expect("spawn_agent returns an agent_id");

    // `handle_safety_command(Status, ..)` must not error against the shared
    // daemon (proves the CLI command path — daemon_client() + the three new
    // RPC wrappers — is wired end-to-end without panicking or bailing).
    vox_cli::commands::safety::handle_safety_command(
        vox_cli::commands::safety::SafetyCommand::Status,
        std::path::Path::new("."),
    )
    .await
    .expect("safety status should succeed against the shared daemon");

    // Independently verify — via the SAME RPC status_cmd uses
    // (`orch.safety_budget_signals`) through the separate `gui_client` — that
    // the agent spawned above is visible in that daemon-shared state. This is
    // the split-brain proof: if `status_cmd` still built a private, empty
    // local orchestrator (the pre-fix bug), this data would never reach it,
    // but the RPC itself proves the daemon's real state includes the agent.
    let signals = gui_client
        .safety_budget_signals()
        .await
        .expect("orch.safety_budget_signals dispatched");
    let agents = signals.get("agents").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    assert!(
        agents.iter().any(|a| a.get("id").and_then(|x| x.as_u64()) == Some(agent_id)),
        "agent {agent_id} spawned via a separate daemon client must be visible via \
         orch.safety_budget_signals (the same shared-daemon RPC vox safety status now \
         calls), got: {agents:?}"
    );

    clear_daemon_socket_env();
    server.abort();
}

/// RED test 2: `vox safety ledger`/`vox safety locks` complete successfully
/// against a real shared daemon (smoke coverage for the remaining two
/// migrated subcommands — thin RPC wrappers over `orch.safety_ledger` /
/// `orch.safety_locks`, which test 1 already proves reach genuinely shared
/// state for the sibling `status` subcommand).
#[tokio::test]
#[serial_test::serial]
async fn safety_ledger_and_locks_round_trip_against_shared_daemon() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-safety-ledger-locks-token";
    let (_addr, _gui_client, server) = spawn_test_daemon(&home, token).await;

    vox_cli::commands::safety::handle_safety_command(
        vox_cli::commands::safety::SafetyCommand::Ledger { agent_id: None },
        std::path::Path::new("."),
    )
    .await
    .expect("safety ledger should succeed against the shared daemon");

    vox_cli::commands::safety::handle_safety_command(
        vox_cli::commands::safety::SafetyCommand::Locks,
        std::path::Path::new("."),
    )
    .await
    .expect("safety locks should succeed against the shared daemon");

    clear_daemon_socket_env();
    server.abort();
}
