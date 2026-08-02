//! T2.3 RED test: `vox live` reaches for the shared `vox-orchestrator-d`
//! daemon rather than silently falling back to a private, isolated
//! in-process `Orchestrator`.
//!
//! Full command-level coverage (proving an event received by the daemon
//! renders in the dashboard) isn't practical here: `run()` is an
//! intentional infinite loop (Ctrl+C to quit) with no natural exit once a
//! daemon connection is established, and its rendering logic
//! (`merge_agent_event`/`render`) is unchanged by T2.3 — only the event
//! *source* changed. What IS new and worth a RED test: before T2.3, `vox
//! live` (in the default, non-file-tail mode) never touched the daemon at
//! all — it built its own, always-empty local `Orchestrator` and sat there
//! rendering nothing forever, succeeding "silently" with no observable
//! failure mode. After T2.3, it genuinely attempts to reach the shared
//! daemon first, so with no daemon reachable and none spawnable, it now
//! fails fast with a clear error — proving the routing is real, not a
//! fallback path that's silently skipped.

#![cfg(feature = "live")]

/// Serialized-env-mutation guard mirroring
/// `dei_daemon_routing_tests.rs::IsolatedHomeEnv` / T2.2's
/// `stdio_daemon_routing_tests.rs` — isolates the well-known daemon
/// token-file location and neutralizes `PATH` so no real installed
/// `vox-orchestrator-d` on this dev machine can be adopted or spawned.
struct IsolatedNoDaemonEnv {
    _tempdir: tempfile::TempDir,
    prev_userprofile: Option<String>,
    prev_home: Option<String>,
    prev_path: Option<String>,
}

impl IsolatedNoDaemonEnv {
    #[allow(unsafe_code)]
    fn new() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let prev_userprofile = std::env::var("USERPROFILE").ok();
        let prev_home = std::env::var("HOME").ok();
        let prev_path = std::env::var("PATH").ok();
        // SAFETY: `#[serial_test::serial]` on every caller serializes env
        // mutation against the rest of this crate's tests.
        unsafe {
            std::env::set_var("USERPROFILE", tempdir.path());
            std::env::set_var("HOME", tempdir.path());
            // Neutralize PATH so `which::which("vox-orchestrator-d")` cannot
            // resolve a real installed daemon binary from this dev machine.
            std::env::set_var("PATH", tempdir.path());
        }
        Self {
            _tempdir: tempdir,
            prev_userprofile,
            prev_home,
            prev_path,
        }
    }
}

impl Drop for IsolatedNoDaemonEnv {
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
            match &self.prev_path {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
            std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
        }
    }
}

/// Bind then immediately drop a loopback listener so `VOX_ORCHESTRATOR_DAEMON_SOCKET`
/// points at a port nothing answers on — any ping fails fast.
#[allow(unsafe_code)]
async fn point_at_dead_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);
    // SAFETY: serialized via `#[serial_test::serial]`.
    unsafe {
        std::env::set_var("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr);
    }
}

/// RED test: `vox live` (no `VOX_ORCHESTRATOR_EVENT_LOG` set — the
/// daemon-routed default path) fails fast with a clear error when no daemon
/// is reachable/spawnable, instead of hanging forever rendering an
/// always-empty local dashboard (the old in-process-Orchestrator behavior).
#[tokio::test]
#[serial_test::serial]
async fn live_run_fails_fast_when_no_daemon_reachable() {
    let _env = IsolatedNoDaemonEnv::new();
    // Ensure the event-log-tail branch is not taken.
    // SAFETY: serialized via `#[serial_test::serial]`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("VOX_ORCHESTRATOR_EVENT_LOG");
    }
    point_at_dead_port().await;

    let result =
        tokio::time::timeout(vox_config::timeouts::D_20S, vox_cli::commands::live::run()).await;
    let outcome = result.expect(
        "vox_cli::commands::live::run() must return, not hang, when no daemon is reachable",
    );
    assert!(
        outcome.is_err(),
        "live::run() must surface a clear error with no daemon reachable/spawnable, got: {outcome:?}"
    );
}
