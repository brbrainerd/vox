//! Single persistent TCP orchestrator daemon shared by the GUI.
//!
//! The GUI used to drive tool execution through an in-process `McpToolHost`
//! (its own `ServerState`) while HITL approvals and the status/event streams
//! talked to a *separate* daemon process spawned per `call_daemon`. Because a
//! one-shot stdio daemon is spawned per call, a parked dangerous-tool approval
//! gate and its later resolve could land in different processes.
//!
//! [`PersistentDaemon`] fixes this by ensuring exactly one long-lived TCP
//! daemon is reachable, so tool calls, approvals, and the live streams all hit
//! the same `ServerState`.

use std::process::Stdio;

use super::process_util::quiet_command;

use tokio::sync::RwLock;
use vox_cli_core::daemon_ipc::process_supervision::{
    resolve_managed_binary_path, resolve_or_stage_daemon_with_version_hint,
};
use vox_config::timeouts::{D_5S, D_15S, D_100MS};
use vox_orchestrator::orch_daemon::OrchDaemonClient;

/// Deadline for the spawned daemon to become reachable via ping.
const DAEMON_CONNECT_TIMEOUT: std::time::Duration = D_15S;
/// Poll interval while waiting for the daemon to start.
const DAEMON_POLL_INTERVAL: std::time::Duration = D_100MS;
/// Interval for the background supervision task's liveness ping (T3.1).
/// A few seconds — frequent enough that a daemon death is noticed promptly,
/// not so aggressive it floods the daemon with pings.
pub const SUPERVISION_INTERVAL: std::time::Duration = D_5S;

/// Default loopback TCP address the GUI binds its orchestrator daemon to when
/// `VOX_ORCHESTRATOR_DAEMON_SOCKET` is not set to a TCP address.
const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:9745";

/// Well-known file the daemon writes its auth token to at startup (T0.2).
/// Mirrors `vox-orchestrator-d`'s `token_file_path()`.
fn token_file_path() -> std::path::PathBuf {
    vox_config::paths::user_home_dir()
        .join(".vox")
        .join("run")
        .join("orchestrator-daemon.token")
}

/// Best-effort read of the well-known daemon token file; `None` if missing,
/// unreadable, or empty.
fn read_token_file() -> Option<String> {
    let contents = std::fs::read_to_string(token_file_path()).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Tauri-managed holder for the one long-lived orchestrator daemon.
///
/// The resolved `(addr, token)` pair is cached in an [`RwLock`] rather than a
/// `OnceCell` (T3.1) so that a background supervision task — or any caller
/// that discovers the cached daemon has gone unreachable — can invalidate the
/// cache and force a fresh `ensure()` run (adopt-if-reachable, else
/// spawn-fresh) instead of returning a permanently stale address. The spawned
/// child (if we started one) is kept alive for the lifetime of the app so the
/// daemon is not reaped.
#[derive(Default)]
pub struct PersistentDaemon {
    resolved: RwLock<Option<(String, String)>>,
    child: std::sync::Mutex<Option<std::process::Child>>,
    /// Serializes [`Self::reensure`] so concurrent `ensure`/`ensure_live`
    /// callers (e.g. the status stream, the event stream, and the
    /// supervisor's periodic tick all racing at once) cannot each spawn a
    /// competing `vox-orchestrator-d` process — mirrors the dedup guarantee
    /// `tokio::sync::OnceCell::get_or_try_init` gave the pre-T3.1 design.
    reensure_lock: tokio::sync::Mutex<()>,
    /// Last-detected mismatch between the daemon's self-reported version
    /// (from a ping response, or from `resolve_or_stage_daemon_with_version_hint`'s
    /// staged-binary hint) and this GUI binary's own version. `None` when no
    /// mismatch has been observed (matching versions, or unknown — see
    /// [`detect_version_mismatch`]'s doc on why "unknown" and "match" are not
    /// distinguished here).
    pub last_version_mismatch: tokio::sync::RwLock<Option<VersionMismatch>>,
}

/// A confirmed daemon/GUI version mismatch, named (not a positional tuple) so
/// the wire format sent to the frontend is self-describing rather than
/// relying on both sides independently agreeing which array index is which
/// version. Serialized to `{ "daemonVersion": ..., "guiVersion": ... }` to
/// match the frontend's existing field naming.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMismatch {
    pub daemon_version: String,
    pub gui_version: String,
}

impl PersistentDaemon {
    /// Ensure one long-lived TCP daemon is reachable; returns its address.
    ///
    /// Cached on success (subsequent calls reuse the address); failures are not
    /// cached, so a later call retries. If no daemon answers a ping, one is
    /// spawned and we poll until it is ready (or a ~15s deadline elapses).
    ///
    /// Auth (T0.2): reusing an already-running daemon requires a
    /// token-authenticated ping to actually succeed — a process that merely
    /// answers `orch.ping` unauthenticated is a port-squatter, not "our"
    /// daemon, and is never adopted. When spawning a fresh daemon, this
    /// generates a token, injects it into the child's environment, and stores
    /// it (retrieve via [`PersistentDaemon::token`] after `ensure()` resolves)
    /// so all subsequent calls from this `PersistentDaemon` are authenticated.
    ///
    /// T3.1: this now re-checks a cached address/token before trusting it —
    /// see [`Self::ensure_live`] doc for the supervised variant callers that
    /// need reconnection-on-death should prefer.
    // toestub-ignore(skeleton/untested-pub-api) — spawns/pings external vox-orchestrator-d process; covered by integration tests
    pub async fn ensure(&self) -> Result<String, String> {
        if let Some((addr, _token)) = self.resolved.read().await.clone() {
            return Ok(addr);
        }
        self.reensure().await
    }

    /// Like [`Self::ensure`], but first performs an authenticated liveness
    /// ping against any cached address (T3.1) and treats a failed ping as
    /// cache invalidation, forcing a full re-resolve (adopt-if-reachable else
    /// spawn-fresh) rather than returning the stale address. Callers on the
    /// hot path for reconnect loops (stream producers, the background
    /// supervisor) should use this instead of [`Self::ensure`] so a dead
    /// daemon is detected and replaced rather than silently returning a
    /// address nobody is listening on anymore.
    pub async fn ensure_live(&self) -> Result<String, String> {
        let cached = self.resolved.read().await.clone();
        if let Some((addr, token)) = cached.clone() {
            if let Ok(resp) = OrchDaemonClient::with_token(addr.clone(), token).ping().await {
                if let Some(mismatch) = detect_version_mismatch(&resp) {
                    *self.last_version_mismatch.write().await = Some(mismatch);
                }
                return Ok(addr);
            }
        }
        // The cached entry (if any) failed its liveness ping — invalidate it
        // before calling `reensure` so its post-lock cache re-check does not
        // trust the same dead entry we just disproved.
        if cached.is_some() {
            *self.resolved.write().await = None;
        }
        self.reensure().await
    }

    /// Clear any cached `(addr, token)` and re-run the full ensure logic
    /// (adopt-if-reachable, else spawn-fresh). Used by [`Self::ensure_live`]
    /// when the cached daemon fails a liveness check, and by [`Self::ensure`]
    /// when nothing is cached yet.
    async fn reensure(&self) -> Result<String, String> {
        // Serialize concurrent re-resolves (see `reensure_lock`'s doc). Once
        // we hold the lock, re-check the cache: another caller may have
        // already re-resolved while we were waiting, in which case we can
        // return its result instead of spawning a second competing daemon.
        let _guard = self.reensure_lock.lock().await;
        if let Some((addr, _token)) = self.resolved.read().await.clone() {
            return Ok(addr);
        }

        let addr = match std::env::var("VOX_ORCHESTRATOR_DAEMON_SOCKET") {
            Ok(s) if s.contains(':') => s,
            _ => DEFAULT_DAEMON_ADDR.to_string(),
        };

        // A daemon is already running — only adopt it if a
        // token-authenticated ping actually succeeds. Reading the
        // token file and pinging with it rejects port-squatters: any
        // process that merely answers ping (but doesn't share our
        // token) is not adopted, and we fall through to spawning a
        // fresh daemon of our own (which self-heals once the bind
        // becomes available).
        if let Some(existing_token) = read_token_file()
            && let Ok(resp) = OrchDaemonClient::with_token(addr.clone(), existing_token.clone())
                .ping()
                .await
        {
            if let Some(mismatch) = detect_version_mismatch(&resp) {
                *self.last_version_mismatch.write().await = Some(mismatch);
            }
            *self.resolved.write().await = Some((addr.clone(), existing_token));
            return Ok(addr);
        }

        // Spawn a fresh daemon and keep the child alive.
        // Stage from target/ into ~/.vox/bin first so the running
        // daemon exe does not lock the target dir (os error 5 on rebuild).
        let home = vox_config::paths::user_home_dir();
        let bin_dir = home.join(".vox").join("bin");
        let target_sibling = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent().map(|d| {
                    let name = if cfg!(windows) {
                        "vox-orchestrator-d.exe"
                    } else {
                        "vox-orchestrator-d"
                    };
                    d.join(name)
                })
            })
            .unwrap_or_else(|| std::path::PathBuf::from("vox-orchestrator-d"));
        let (daemon_bin_result, version_hint) =
            resolve_or_stage_daemon_with_version_hint(&target_sibling, &bin_dir);
        let daemon_bin = daemon_bin_result
            .unwrap_or_else(|_| resolve_managed_binary_path("vox-orchestrator-d"));
        if let Some(daemon_version) = version_hint
            && daemon_version != env!("CARGO_PKG_VERSION")
        {
            *self.last_version_mismatch.write().await = Some(VersionMismatch {
                daemon_version,
                gui_version: env!("CARGO_PKG_VERSION").to_string(),
            });
        }

        // Generate a token ourselves and inject it into the child's
        // environment — this avoids a race with reading the token
        // file before the daemon has written it (the daemon still
        // writes the file too, for other clients to auto-resolve).
        let spawned_token = uuid::Uuid::new_v4().to_string();
        let child = quiet_command(daemon_bin)
            .env("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr)
            .env("VOX_ORCHESTRATOR_DAEMON_TOKEN", &spawned_token)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn vox-orchestrator-d: {e}"))?;
        if let Ok(mut slot) = self.child.lock()
            && let Some(mut old) = slot.replace(child)
        {
            let _ = old.kill();
        }

        // Poll until the daemon answers an authenticated ping or the
        // deadline elapses.
        let deadline = std::time::Instant::now() + DAEMON_CONNECT_TIMEOUT;
        while std::time::Instant::now() < deadline {
            if let Ok(resp) = OrchDaemonClient::with_token(addr.clone(), spawned_token.clone())
                .ping()
                .await
            {
                if let Some(mismatch) = detect_version_mismatch(&resp) {
                    *self.last_version_mismatch.write().await = Some(mismatch);
                }
                *self.resolved.write().await = Some((addr.clone(), spawned_token));
                return Ok(addr);
            }
            tokio::time::sleep(DAEMON_POLL_INTERVAL).await;
        }

        if let Ok(mut slot) = self.child.lock()
            && let Some(mut spawned) = slot.take()
        {
            let _ = spawned.kill();
        }

        Err(format!(
            "vox-orchestrator-d did not become reachable at {addr} within 15s"
        ))
    }

    /// The daemon auth token resolved as a side effect of [`PersistentDaemon::ensure`]
    /// (or [`PersistentDaemon::ensure_live`]). Call one of those first; `None`
    /// if neither has yet succeeded.
    pub async fn token(&self) -> Option<String> {
        self.resolved.read().await.clone().map(|(_, token)| token)
    }

    /// Force the cache to be dropped and re-resolved on the next `ensure`/
    /// `ensure_live` call, without performing a liveness ping itself. Useful
    /// for callers that already know the cached daemon is gone (e.g. a
    /// stream that just observed its connection close) and want the next
    /// `ensure()` to re-resolve rather than pinging first.
    pub async fn invalidate(&self) {
        *self.resolved.write().await = None;
    }

    /// Spawn a background task that periodically (every [`SUPERVISION_INTERVAL`])
    /// calls [`Self::ensure_live`] against `self`, so a daemon that dies
    /// mid-session is detected and replaced even if no GUI stream or command
    /// happens to touch the daemon in the meantime (T3.1). Intended to be
    /// spawned once from `main.rs`'s `.setup()` hook, alongside the existing
    /// stream-spawn calls.
    pub fn spawn_supervisor(self: std::sync::Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(SUPERVISION_INTERVAL).await;
                if let Err(e) = self.ensure_live().await {
                    tracing::debug!("orchestrator daemon supervision: {e}");
                }
            }
        });
    }
}

/// Returns the last-detected daemon/GUI version mismatch, if any (T2/Task 2).
/// `None` when versions match or no mismatch has been observed yet.
#[tauri::command]
pub async fn orchestrator_version_mismatch(
    state: tauri::State<'_, std::sync::Arc<PersistentDaemon>>,
) -> Result<Option<VersionMismatch>, String> {
    Ok(state.last_version_mismatch.read().await.clone())
}

/// Compares the daemon's self-reported `version` (from its ping response)
/// against this GUI binary's own compile-time version. Returns `None` when
/// they match (or the daemon's response is missing the field — an older
/// daemon binary pre-dating Task 1, treated as "unknown, don't warn" rather
/// than "mismatch", since we can't distinguish an old-but-compatible daemon
/// from a genuinely incompatible one without the field). Returns
/// `Some(VersionMismatch { .. })` on a confirmed mismatch.
pub fn detect_version_mismatch(ping_response: &serde_json::Value) -> Option<VersionMismatch> {
    let daemon_version = ping_response.get("version")?.as_str()?.to_string();
    let gui_version = env!("CARGO_PKG_VERSION").to_string();
    if daemon_version != gui_version {
        Some(VersionMismatch {
            daemon_version,
            gui_version,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod version_mismatch_tests {
    use super::*;

    #[test]
    fn no_mismatch_when_versions_match() {
        let resp = serde_json::json!({"ok": true, "version": env!("CARGO_PKG_VERSION")});
        assert_eq!(detect_version_mismatch(&resp), None);
    }

    #[test]
    fn mismatch_detected_when_versions_differ() {
        let resp = serde_json::json!({"ok": true, "version": "0.0.1-stale"});
        let result = detect_version_mismatch(&resp);
        assert_eq!(
            result,
            Some(VersionMismatch {
                daemon_version: "0.0.1-stale".to_string(),
                gui_version: env!("CARGO_PKG_VERSION").to_string(),
            })
        );
    }

    #[test]
    fn no_mismatch_reported_when_version_field_missing() {
        let resp = serde_json::json!({"ok": true});
        assert_eq!(detect_version_mismatch(&resp), None);
    }

    #[test]
    fn version_mismatch_serializes_to_camel_case_named_fields() {
        let mismatch = VersionMismatch {
            daemon_version: "0.5.9".to_string(),
            gui_version: "0.6.0".to_string(),
        };
        let json = serde_json::to_value(&mismatch).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"daemonVersion": "0.5.9", "guiVersion": "0.6.0"})
        );
    }
}

#[cfg(test)]
mod tests {
    //! T3.1 RED tests: `PersistentDaemon`'s liveness re-check must genuinely
    //! detect a dead daemon and re-resolve rather than returning a stale
    //! cached address forever.
    //!
    //! These tests exercise the *cache-invalidation and re-resolve* logic
    //! directly (bind a real in-process `vox-orchestrator-d` TCP listener via
    //! `orch_daemon::serve_listener_with_extra`, no external process spawn —
    //! matching `crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs`'s
    //! pattern) rather than spawning the real `vox-orchestrator-d` binary,
    //! which isn't available/practical in a unit-test sandbox. A dedicated
    //! `#[tokio::test(flavor = "multi_thread")]` per test avoids cross-test
    //! interference since each test binds its own ephemeral port and mutates
    //! only its own `PersistentDaemon` instance (no shared process-global
    //! env-var state is required by these tests: `ensure_live`/`reensure`'s
    //! liveness re-check path is driven purely from the cached `(addr, token)`
    //! pair, not from re-reading `VOX_ORCHESTRATOR_DAEMON_SOCKET`).
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use vox_orchestrator::{Orchestrator, OrchestratorConfig, orch_daemon};

    /// Bind a fresh in-process orchestrator daemon on an ephemeral port with
    /// the given `token`; returns `(addr, join_handle)`. Aborting the handle
    /// simulates the daemon process dying (connection refused thereafter).
    async fn spawn_test_daemon(token: &str) -> (String, tokio::task::JoinHandle<()>) {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr").to_string();
        let bind_label = addr.clone();
        let tok: Arc<str> = Arc::from(token);
        let handle = tokio::spawn(async move {
            let _ = orch_daemon::serve_listener_with_extra(
                listener,
                bind_label,
                "t3-1-test-repo".to_string(),
                orch,
                None,
                Some(tok),
            )
            .await;
        });

        // Wait for readiness via authenticated ping (mirrors
        // orchestrator_daemon_tcp.rs's wait_until_async pattern).
        let deadline = std::time::Instant::now() + D_15S;
        while std::time::Instant::now() < deadline {
            if OrchDaemonClient::with_token(addr.clone(), token.to_string())
                .ping()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(D_100MS).await;
        }
        (addr, handle)
    }

    /// `ensure_live()` on a freshly-cached, live daemon must return the same
    /// address without disturbing the cache (the happy path — no false
    /// invalidation of a daemon that is actually still up).
    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_live_reuses_cache_when_daemon_still_reachable() {
        let (addr, _daemon_handle) = spawn_test_daemon("tok-alive").await;

        let pd = PersistentDaemon::default();
        *pd.resolved.write().await = Some((addr.clone(), "tok-alive".to_string()));

        let resolved = pd.ensure_live().await.expect("ensure_live");
        assert_eq!(
            resolved, addr,
            "ensure_live must reuse the still-live cached address"
        );
    }

    /// Core T3.1 regression: once the cached daemon goes unreachable (process
    /// killed / connection refused), `ensure_live()` must NOT keep returning
    /// the stale cached address — it must detect the failure and clear the
    /// cache so a subsequent full `reensure()` can adopt or spawn a
    /// replacement, rather than silently handing back a dead address forever
    /// (the bug this task fixes: the old `OnceCell` cached the address
    /// permanently after first success).
    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_live_detects_dead_daemon_and_invalidates_cache() {
        let (addr, daemon_handle) = spawn_test_daemon("tok-dying").await;

        // `reensure()`'s fallback path reads `VOX_ORCHESTRATOR_DAEMON_SOCKET`
        // (defaulting to port 9745) to decide where to adopt-or-spawn. Pin it
        // to this test's own (now-dying) ephemeral address so the assertions
        // below aren't polluted by a real `vox-orchestrator-d` that may
        // already be running on the developer's machine at the default port
        // — without this, `reensure` would legitimately adopt that unrelated
        // live daemon and the "must fail to reconnect" assertion would be
        // testing the wrong thing.
        // SAFETY: single-threaded w.r.t. this env var within this test's
        // lifetime; `--test-threads=1` or per-test isolation is required for
        // other tests reading/writing the same var to not race this one.
        unsafe {
            std::env::set_var("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr);
        }

        let pd = PersistentDaemon::default();
        *pd.resolved.write().await = Some((addr.clone(), "tok-dying".to_string()));

        // Simulate the daemon process dying: abort the listener task (Rust
        // JoinHandle::abort — matches the plan's "process-kill uses the Rust
        // Child::kill path, not shell" constraint in spirit; here the
        // in-process equivalent since this test doesn't spawn a real child
        // process).
        daemon_handle.abort();
        // Give the OS a moment to actually tear down the listening socket.
        let deadline = std::time::Instant::now() + D_15S;
        loop {
            if OrchDaemonClient::with_token(addr.clone(), "tok-dying".to_string())
                .ping()
                .await
                .is_err()
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("test daemon did not actually go unreachable after abort()");
            }
            tokio::time::sleep(D_20MS).await;
        }

        // Re-occupy the now-freed port with a listener that accepts
        // connections but never replies, so any ping against it times out /
        // gets a connection-reset rather than a valid response — and, load
        // bearing for *not spawning a real daemon process from this unit
        // test*: with the port held, `reensure`'s spawn-fresh fallback's
        // `vox-orchestrator-d` child fails to bind and exits immediately,
        // so `reensure` fails fast via its own poll-timeout instead of
        // successfully starting (and orphaning) a real daemon process.
        let blocker = TcpListener::bind(&addr).await.expect("reclaim port");
        let _blocker_task = tokio::spawn(async move {
            loop {
                if blocker.accept().await.is_err() {
                    break;
                }
                // Accept and immediately drop the connection without
                // replying — never a valid `orch.ping` response.
            }
        });

        // No replacement daemon is reachable at this address (nothing else is
        // listening with our token), so `ensure_live` must fail to
        // *reconnect* — but the important assertion is that it does NOT
        // report success with the stale address, and it must have cleared
        // the cache rather than leaving the dead entry in place for a future
        // caller to trust.
        let result = pd.ensure_live().await;
        unsafe {
            std::env::remove_var("VOX_ORCHESTRATOR_DAEMON_SOCKET");
        }
        assert!(
            result.is_err(),
            "ensure_live must not report success once the cached daemon is unreachable \
             and nothing else is listening at its address"
        );
        assert!(
            pd.resolved.read().await.is_none(),
            "ensure_live must clear the stale cache entry on liveness-check failure, \
             not leave the dead (addr, token) pair cached for future callers"
        );
    }

    /// `invalidate()` unconditionally drops the cache so the next `ensure()`
    /// re-resolves, without itself performing a network ping.
    #[tokio::test(flavor = "multi_thread")]
    async fn invalidate_clears_cache_without_pinging() {
        let pd = PersistentDaemon::default();
        *pd.resolved.write().await = Some(("127.0.0.1:1".to_string(), "tok".to_string()));
        pd.invalidate().await;
        assert!(pd.resolved.read().await.is_none());
    }

    const D_20MS: std::time::Duration = std::time::Duration::from_millis(20);
}
