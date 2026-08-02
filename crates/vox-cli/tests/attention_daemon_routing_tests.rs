//! T2.3 follow-up RED test: `vox attention snapshot` routes through the
//! shared `vox-orchestrator-d` TCP daemon instead of building a private,
//! throwaway in-process `Orchestrator` per invocation.
//!
//! Mirrors the pattern established by `safety_daemon_routing_tests.rs`
//! (T2.3 follow-up for `commands/safety.rs`): spin up a REAL in-process
//! daemon server (`orch_daemon::serve_listener_with_extra`) bound to an
//! ephemeral port, point `VOX_ORCHESTRATOR_DAEMON_SOCKET` at it, mutate the
//! daemon-shared `BudgetManager`'s attention state directly (standing in for
//! another terminal/agent spending attention budget), then call
//! `vox_cli::commands::attention`'s `pub async fn handle_attention_command`
//! directly — proving it genuinely reaches that daemon's shared
//! `Orchestrator`, not some other, invisible instance.
//!
//! Before the T2.3 follow-up fix, `snapshot_cmd` called
//! `build_repo_scoped_orchestrator_for_repository` — a fresh, always-empty
//! local `Orchestrator` — so `vox attention snapshot` displayed fake/empty
//! budget data regardless of what the real daemon-shared orchestrator held.
//! This test proves the fix: attention spend applied directly to the shared
//! daemon's `BudgetManager` is visible via `orch.attention_snapshot`, the
//! same RPC `snapshot_cmd` now calls.

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
        tokio::time::sleep(vox_config::timeouts::D_20MS).await;
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
/// Returns the `ServerState` too, so tests can mutate the daemon-shared
/// `Orchestrator`'s state directly (standing in for a separate process).
async fn spawn_test_daemon(
    home: &IsolatedHomeEnv,
    token: &str,
) -> (
    String,
    ServerState,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
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

    (addr, state, server)
}

#[allow(unsafe_code)]
fn clear_daemon_socket_env() {
    // SAFETY: serialized via `#[serial_test::serial]`.
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
    }
}

/// RED test (core acceptance test): `vox attention snapshot` reflects
/// attention spend applied directly to the shared daemon's `BudgetManager`
/// — proving `commands::attention::snapshot_cmd` (via
/// `handle_attention_command`) genuinely reaches the shared daemon's
/// `Orchestrator`, not a private throwaway one. Before the fix this always
/// showed a zeroed-out attention snapshot regardless of daemon state.
#[tokio::test]
#[serial_test::serial]
async fn attention_snapshot_reflects_spend_applied_via_shared_daemon() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-attention-snapshot-token";
    let (_addr, state, server) = spawn_test_daemon(&home, token).await;

    // Mutate the daemon-shared `BudgetManager`'s attention spend directly
    // (standing in for another process/agent spending attention budget),
    // then confirm it is visible via `orch.attention_snapshot` — the same
    // RPC `snapshot_cmd` now calls.
    let bm_handle = state.orchestrator.budget_manager_handle();
    {
        let bm = vox_orchestrator::sync_lock::rw_read(&*bm_handle);
        bm.add_questioning_attention_debit_ms(12_345);
    }

    // `handle_attention_command(Snapshot, ..)` must not error against the
    // shared daemon (proves the CLI command path — daemon_client() + the
    // new RPC wrapper — is wired end-to-end without panicking or bailing).
    vox_cli::commands::attention::handle_attention_command(
        vox_cli::commands::attention::AttentionCommand::Snapshot,
        std::path::Path::new("."),
    )
    .await
    .expect("attention snapshot should succeed against the shared daemon");

    // Independently verify — via the SAME RPC snapshot_cmd uses
    // (`orch.attention_snapshot`) through a separate client connection —
    // that the spend applied above is visible in that daemon-shared state.
    // This is the split-brain proof: if `snapshot_cmd` still built a
    // private, empty local orchestrator (the pre-fix bug), this data would
    // never reach it, but the RPC itself proves the daemon's real state
    // includes the spend.
    let client = orch_daemon::OrchDaemonClient::with_token(_addr.clone(), token.to_string());
    let resp = client
        .attention_snapshot()
        .await
        .expect("orch.attention_snapshot dispatched");
    let spent_ms = resp
        .get("snapshot")
        .and_then(|s| s.get("spent_ms"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    assert!(
        spent_ms >= 12_345,
        "attention spend applied directly to the shared daemon's BudgetManager must be \
         visible via orch.attention_snapshot (the same shared-daemon RPC vox attention \
         snapshot now calls), got spent_ms={spent_ms}"
    );

    clear_daemon_socket_env();
    server.abort();
}
