//! T2.3 RED tests: `vox dei` subcommands route through the shared
//! `vox-orchestrator-d` TCP daemon instead of building a private, throwaway
//! in-process `Orchestrator` per invocation.
//!
//! Mirrors the pattern established by
//! `crates/vox-orchestrator-mcp/tests/stdio_daemon_routing_tests.rs` (T2.2):
//! spin up a REAL in-process daemon server (`orch_daemon::serve_listener_with_extra`)
//! bound to an ephemeral port, point `VOX_ORCHESTRATOR_DAEMON_SOCKET` at it, and
//! call `vox_cli::commands::dei`'s `pub async fn`s directly — proving they
//! genuinely reach that daemon's shared `Orchestrator`, not some other,
//! invisible instance.

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

/// Serialized-env-mutation guard: points `USERPROFILE` (Windows) / `HOME`
/// (else) at a fresh temp dir so `vox_config::paths::user_home_dir()` — and
/// therefore the well-known daemon token file path both `OrchDaemonClient`
/// and `OrchestratorDaemonEnsure` read — resolves under our isolated temp
/// dir. Callers MUST apply `#[serial_test::serial]` so no other test races
/// the env var while this guard is alive. Mirrors
/// `stdio_daemon_routing_tests.rs::IsolatedHomeEnv`.
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

/// Spin up a real in-process daemon on an ephemeral port, with a token file
/// written so `OrchestratorDaemonEnsure` adopts it (rather than trying to
/// spawn a real `vox-orchestrator-d.exe`, which isn't built in a unit-test
/// run). Returns the daemon's bound address and its own `OrchDaemonClient`
/// handle (for a second, independent "GUI-standing-in" connection) plus the
/// `JoinHandle` so the caller can `.abort()` it at the end.
async fn spawn_test_daemon(
    home: &IsolatedHomeEnv,
    token: &str,
) -> (
    String,
    orch_daemon::OrchDaemonClient,
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

/// RED test 1 (core acceptance test): `vox dei submit` lands a task in the
/// SAME daemon-owned `Orchestrator` a separate client connection sees —
/// proving `commands::dei::submit` (and thus its `daemon_client()` helper)
/// genuinely routes through the shared daemon rather than building a
/// private, invisible `Orchestrator`.
#[tokio::test]
#[serial_test::serial]
async fn dei_submit_task_is_visible_from_a_separate_daemon_client() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-dei-submit-token";
    let (_addr, gui_client, server) = spawn_test_daemon(&home, token).await;

    vox_cli::commands::dei::submit(
        "T2.3 RED test task [[unique-marker-dei-submit]]",
        &[],
        None,
        None,
    )
    .await
    .expect("dei::submit should succeed against the shared daemon");

    // A completely separate client connection (standing in for the GUI)
    // lists tasks — proving the submitted task landed in the daemon's
    // shared state, not some other, invisible Orchestrator.
    let list = gui_client
        .call(
            vox_foundation::protocol::orch_daemon_method::LIST_TASKS,
            serde_json::json!({}),
        )
        .await
        .expect("orch.list_tasks dispatched");
    let tasks = list
        .get("tasks")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        tasks.iter().any(|t| {
            t.get("description")
                .and_then(|d| d.as_str())
                .map(|d| d.contains("unique-marker-dei-submit"))
                .unwrap_or(false)
        }),
        "task submitted via commands::dei::submit must be visible via a separate \
         client's orch.list_tasks (shared daemon state), got: {tasks:?}"
    );

    clear_daemon_socket_env();
    server.abort();
}

/// RED test 2: `vox dei doubt` (raw `orch.doubt_task` RPC) mutates state
/// visible from a separate client — same shared-daemon-state proof as test 1,
/// for the doubt/overrule RPC path (distinct code path from `submit_task`).
#[tokio::test]
#[serial_test::serial]
async fn dei_doubt_is_visible_from_a_separate_daemon_client() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-dei-doubt-token";
    let (_addr, gui_client, server) = spawn_test_daemon(&home, token).await;

    // Submit a task via the same client this test is validating, then doubt
    // it — both must land in the same shared daemon state.
    vox_cli::commands::dei::submit(
        "Doubt-target task [[unique-marker-dei-doubt]]",
        &[],
        None,
        None,
    )
    .await
    .expect("dei::submit should succeed against the shared daemon");

    let list = gui_client
        .call(
            vox_foundation::protocol::orch_daemon_method::LIST_TASKS,
            serde_json::json!({}),
        )
        .await
        .expect("orch.list_tasks dispatched");
    let tasks = list
        .get("tasks")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let task_id = tasks
        .iter()
        .find(|t| {
            t.get("description")
                .and_then(|d| d.as_str())
                .map(|d| d.contains("unique-marker-dei-doubt"))
                .unwrap_or(false)
        })
        .and_then(|t| t.get("id"))
        .and_then(|i| i.as_u64())
        .expect("submitted task visible via orch.list_tasks");

    vox_cli::commands::dei::doubt(task_id, Some("T2.3 RED test doubt".to_string()))
        .await
        .expect("dei::doubt should succeed against the shared daemon");

    // Verify from the SEPARATE client that the task's raw status reflects
    // the doubt — proving commands::dei::doubt mutated the shared daemon's
    // Orchestrator, not a private one. `orch.list_tasks`'s `status` field is
    // the raw `TaskStatus` enum (serializes as `{"Doubted": [..]}`), which
    // is a more direct signal here than `task_lifecycle_status_label` (which
    // reports a doubted-but-requeued task as "Queued").
    let list_after = gui_client
        .call(
            vox_foundation::protocol::orch_daemon_method::LIST_TASKS,
            serde_json::json!({}),
        )
        .await
        .expect("orch.list_tasks dispatched");
    let status_after = list_after
        .get("tasks")
        .and_then(|t| t.as_array())
        .and_then(|tasks| {
            tasks
                .iter()
                .find(|t| t.get("id").and_then(|i| i.as_u64()) == Some(task_id))
        })
        .and_then(|t| t.get("status"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let status_str = status_after.to_string();
    assert!(
        status_str.contains("Doubted"),
        "task doubted via commands::dei::doubt must show a Doubted status via a \
         separate client's orch.list_tasks (shared daemon state), got status: {status_str}"
    );

    clear_daemon_socket_env();
    server.abort();
}

/// RED test 3: `vox dei rebalance`/`vox dei pause`/`vox dei resume` all
/// complete successfully against a real shared daemon (smoke coverage for
/// the remaining migrated RPC-wrapper subcommands, which are thin enough
/// that a success/failure smoke check is proportionate — the doubt/submit
/// tests above already prove the cross-client-visibility property these
/// share).
#[tokio::test]
#[serial_test::serial]
async fn dei_agent_lifecycle_commands_round_trip_against_shared_daemon() {
    let home = IsolatedHomeEnv::new();
    let token = "t23-dei-lifecycle-token";
    let (_addr, gui_client, server) = spawn_test_daemon(&home, token).await;

    // The default embedded config may not pre-spawn any agents; spawn one
    // explicitly via the same shared daemon rather than assuming defaults.
    let spawned = gui_client
        .spawn_agent_named("t23-lifecycle-agent")
        .await
        .expect("orch.spawn_agent dispatched");
    let agent_id = spawned
        .get("agent_id")
        .and_then(|v| v.as_u64())
        .expect("spawn_agent returns an agent_id");

    vox_cli::commands::dei::pause(agent_id)
        .await
        .expect("dei::pause should not error against the shared daemon");
    vox_cli::commands::dei::resume(agent_id)
        .await
        .expect("dei::resume should not error against the shared daemon");
    vox_cli::commands::dei::rebalance()
        .await
        .expect("dei::rebalance should not error against the shared daemon");
    vox_cli::commands::dei::status()
        .await
        .expect("dei::status should not error against the shared daemon");

    clear_daemon_socket_env();
    server.abort();
}
