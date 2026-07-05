//! T2.3 RED test: `vox ludus hud` reaches for the shared `vox-orchestrator-d`
//! daemon rather than silently falling back to a private, isolated
//! in-process `Orchestrator`.
//!
//! See `live_daemon_routing_tests.rs`'s module doc for why a fail-fast test
//! is the practical proof here (infinite-loop `run()`, unchanged
//! rendering/companion-mood logic — only the event source changed).
//!
//! NOTE: as of this writing, `cargo test -p vox-cli --features ludus-hud`
//! fails to compile for a reason UNRELATED to T2.3 — `commands/extras/ludus/
//! {auth,ctx,profile}.rs` call `VoxDb::upsert_vox_identity`/`get_vox_identities`,
//! methods that no longer exist on `vox-db::VoxDb` (confirmed pre-existing on
//! this branch before any T2.3 changes). This test file is otherwise
//! self-contained and correct; it will run once that unrelated drift is
//! fixed (flagged separately, not addressed by T2.3).

#![cfg(feature = "ludus-hud")]

use std::time::Duration;

/// Serialized-env-mutation guard — see `live_daemon_routing_tests.rs`'s
/// identical helper for rationale.
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

/// RED test: `vox ludus hud` fails fast with a clear error when no daemon is
/// reachable/spawnable, instead of hanging forever on its own always-empty
/// local `Orchestrator`'s bulletin bus (the old in-process behavior, which
/// had no failure mode to observe at all).
#[tokio::test]
#[serial_test::serial]
async fn hud_run_fails_fast_when_no_daemon_reachable() {
    let _env = IsolatedNoDaemonEnv::new();
    point_at_dead_port().await;

    let result = tokio::time::timeout(
        Duration::from_secs(20),
        vox_cli::commands::extras::ludus::ludus_hud_run(),
    )
    .await;
    let outcome = result.expect(
        "vox_cli::commands::extras::ludus::ludus_hud_run() must return, not hang, when no daemon is reachable",
    );
    assert!(
        outcome.is_err(),
        "hud::run() must surface a clear error with no daemon reachable/spawnable, got: {outcome:?}"
    );
}
