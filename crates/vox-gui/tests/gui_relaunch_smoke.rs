//! Track D: automated, headless end-to-end relaunch smoke for the Vox GUI.
//!
//! Proves — with NO windowing/display runtime — the two display-free behaviors
//! that actually carry GUI→backend wiring:
//!   1. The command-catalog logic every GUI surface calls first
//!      (`get_command_catalog` → `vox_cli::command_catalog::build_catalog`) links
//!      and returns a non-empty catalog.
//!   2. The real `vox-orchestrator-d` binary relaunches exactly as the GUI's
//!      `PersistentDaemon::ensure` does (same `resolve_managed_binary_path` +
//!      `VOX_ORCHESTRATOR_DAEMON_SOCKET` env), and the real `OrchDaemonClient`
//!      RPC (`ping` → `orchestrator_status` → `agent_ids`) answers, with a fresh
//!      daemon reporting zero agents.
//!
//! It never constructs a `tauri::Builder` app, so it needs no WebView2/WebKitGTK
//! or display — it runs on the same bare CI runners as the WebIR lane (unlike the
//! browser-gated Playwright lane). Rendering / input / a11y stay in Playwright.
//!
//! Gated behind `VOX_GUI_RELAUNCH_SMOKE=1` (a local-dev opt-out convenience —
//! no `#[ignore]`/`--ignored` involved) because it requires a built
//! `vox-orchestrator-d` binary; the `gui-orchestrator-relaunch-smoke` CI job
//! builds that binary and sets the env var, making this a required gate
//! (CR-U6).

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use vox_cli_core::daemon_ipc::process_supervision::resolve_managed_binary_path;
use vox_orchestrator::orch_daemon::OrchDaemonClient;

fn relaunch_smoke_enabled() -> bool {
    std::env::var("VOX_GUI_RELAUNCH_SMOKE").ok().as_deref() == Some("1")
}

/// Bind an ephemeral loopback port, then release it — avoids a hard-coded port
/// racing with a concurrent run.
fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    format!("127.0.0.1:{port}")
}

/// RAII: a panicking assertion must never leak the spawned daemon process.
struct DaemonGuard(Child);
impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn gui_relaunch_boots_daemon_and_core_surfaces_respond() {
    if !relaunch_smoke_enabled() {
        eprintln!("skipping gui relaunch smoke: set VOX_GUI_RELAUNCH_SMOKE=1");
        return;
    }

    // (1) The first call every GUI surface makes must link and return real data.
    let catalog = vox_cli::command_catalog::build_catalog();
    assert!(
        !catalog.entries.is_empty(),
        "GUI command catalog (get_command_catalog) must be non-empty"
    );

    // (2) Relaunch the real daemon binary exactly as PersistentDaemon::ensure does.
    let addr = free_loopback_addr();
    let bin = resolve_managed_binary_path("vox-orchestrator-d");

    // Generate a token ourselves and inject it into the child's environment —
    // this is the same idiom `PersistentDaemon::ensure`
    // (`crates/vox-gui/src/commands/daemon.rs`) and every other daemon-spawn
    // test in this repo use (e.g. `orchestrator_daemon_accepts_correct_token`
    // in `orchestrator_daemon_tcp.rs`, `stdio_daemon_routing_tests.rs`). It
    // avoids a race with reading the daemon's token file before the daemon
    // has written it: `OrchDaemonClient::with_token` lets the client
    // authenticate immediately, with no dependency on file-write timing.
    let spawned_token = uuid::Uuid::new_v4().to_string();
    let child = Command::new(&bin)
        .env("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr)
        .env("VOX_ORCHESTRATOR_DAEMON_TOKEN", &spawned_token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn daemon {}: {e}", bin.display()));
    let _guard = DaemonGuard(child);

    let client = OrchDaemonClient::with_token(addr.clone(), spawned_token);

    // Poll until the daemon process is actually listening and answering —
    // no token-file race is involved here, just normal process startup time.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if client.ping().await.is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "daemon never became reachable at {addr}"
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    // Core read RPCs respond.
    let _status = client
        .orchestrator_status()
        .await
        .expect("orchestrator_status RPC should succeed");

    let agents = client
        .agent_ids()
        .await
        .expect("agent_ids RPC should succeed");
    if let Some(arr) = agents.get("agent_ids").and_then(|v| v.as_array()) {
        assert!(
            arr.is_empty(),
            "a freshly-relaunched daemon should report zero agents, got {arr:?}"
        );
    }

    // The relaunched daemon's self-reported version matches this build's own
    // workspace version — a real CI guard against ever shipping a
    // version-reporting regression (e.g. a hardcoded stale version string).
    let ping_response = client.ping().await.expect("ping should succeed");
    assert_eq!(
        ping_response.get("version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION")),
        "relaunched daemon's ping response version must match this build's workspace version"
    );
}
