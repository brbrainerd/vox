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

use tokio::sync::OnceCell;
use vox_cli_core::daemon_ipc::process_supervision::{
    resolve_managed_binary_path, resolve_or_stage_daemon,
};
use vox_config::timeouts::{D_15S, D_100MS};
use vox_orchestrator::orch_daemon::OrchDaemonClient;

/// Deadline for the spawned daemon to become reachable via ping.
const DAEMON_CONNECT_TIMEOUT: std::time::Duration = D_15S;
/// Poll interval while waiting for the daemon to start.
const DAEMON_POLL_INTERVAL: std::time::Duration = D_100MS;

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
/// The resolved address is cached only on success (see [`PersistentDaemon::ensure`]),
/// and the spawned child (if we started one) is kept alive for the lifetime of
/// the app so the daemon is not reaped.
#[derive(Default)]
pub struct PersistentDaemon {
    addr: OnceCell<String>,
    token: OnceCell<String>,
    child: std::sync::Mutex<Option<std::process::Child>>,
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
    /// it in `self.token` (retrieve via [`PersistentDaemon::token`] after
    /// `ensure()` resolves) so all subsequent calls from this
    /// `PersistentDaemon` are authenticated.
    // toestub-ignore(skeleton/untested-pub-api) — spawns/pings external vox-orchestrator-d process; covered by integration tests
    pub async fn ensure(&self) -> Result<String, String> {
        self.addr
            .get_or_try_init(|| async {
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
                    && OrchDaemonClient::with_token(addr.clone(), existing_token.clone())
                        .ping()
                        .await
                        .is_ok()
                {
                    let _ = self.token.set(existing_token);
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
                let daemon_bin = resolve_or_stage_daemon(&target_sibling, &bin_dir)
                    .unwrap_or_else(|_| resolve_managed_binary_path("vox-orchestrator-d"));

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
                    if OrchDaemonClient::with_token(addr.clone(), spawned_token.clone())
                        .ping()
                        .await
                        .is_ok()
                    {
                        let _ = self.token.set(spawned_token);
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
            })
            .await
            .cloned()
    }

    /// The daemon auth token resolved as a side effect of [`PersistentDaemon::ensure`].
    /// Call `ensure()` first; `None` if `ensure()` has not yet succeeded.
    pub async fn token(&self) -> Option<String> {
        self.token.get().cloned()
    }
}
