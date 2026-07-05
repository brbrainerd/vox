//! Shared "ensure a running, authenticated `vox-orchestrator-d`" helper.
//!
//! T2.2: extracted from `vox-gui`'s `PersistentDaemon`
//! (`crates/vox-gui/src/commands/daemon.rs`) so non-GUI clients — the `vox
//! mcp` stdio server, and (per T2.3) `vox-cli`'s own daemon-routed commands —
//! can reach the *same* single shared orchestrator daemon the GUI talks to,
//! rather than each booting a private `ServerState`. `vox-gui`'s
//! `PersistentDaemon` is NOT reused directly here (it's a Tauri-specific type
//! with a `tauri::State`-managed lifetime) — this is a logically equivalent,
//! standalone implementation. Consolidating `PersistentDaemon` to wrap this
//! helper is a worthwhile follow-up but out of scope for T2.2 (see
//! docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md).
//!
//! Ping-first: an already-running daemon is adopted only if a
//! token-authenticated ping succeeds (rejects port-squatters). Otherwise a
//! fresh `vox-orchestrator-d` is spawned (staged into `~/.vox/bin` first, so
//! the running daemon never locks a `target/` build dir), given a freshly
//! generated token via `VOX_ORCHESTRATOR_DAEMON_TOKEN`, and polled for
//! readiness up to a ~15s deadline.

use std::process::Stdio;
use std::sync::Mutex;

use tokio::sync::OnceCell;
use vox_config::timeouts::{D_15S, D_100MS};
use vox_orchestrator::orch_daemon::OrchDaemonClient;

use super::process_supervision::{resolve_managed_binary_path, resolve_or_stage_daemon};

/// Deadline for the spawned daemon to become reachable via ping.
const DAEMON_CONNECT_TIMEOUT: std::time::Duration = D_15S;
/// Poll interval while waiting for the daemon to start.
const DAEMON_POLL_INTERVAL: std::time::Duration = D_100MS;

/// Default loopback TCP address `vox-orchestrator-d` binds to when
/// `VOX_ORCHESTRATOR_DAEMON_SOCKET` is not set to a TCP address.
const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:9745";

/// Well-known file the daemon writes its auth token to at startup (T0.2).
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

/// Spawn helper mirroring `vox-gui`'s `process_util::quiet_command` — sets
/// `CREATE_NO_WINDOW` on Windows so spawning `vox-orchestrator-d` from a
/// console-attached process (e.g. `vox mcp` run under an MCP client that
/// hides the console) never flashes an extra window.
fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Ensures exactly one long-lived TCP `vox-orchestrator-d` is reachable,
/// spawning it if absent. Logically equivalent to `vox-gui`'s
/// `PersistentDaemon`, but usable from any process (no Tauri dependency).
///
/// Cheap to construct — typically held as a `once_cell`/`static`/long-lived
/// field by the caller so `ensure()`'s address/token cache is reused across
/// calls within one process.
#[derive(Default)]
pub struct OrchestratorDaemonEnsure {
    addr: OnceCell<String>,
    token: OnceCell<String>,
    child: Mutex<Option<std::process::Child>>,
}

impl OrchestratorDaemonEnsure {
    /// Ensure one long-lived TCP daemon is reachable; returns its address.
    ///
    /// Cached on success (subsequent calls reuse the address); failures are
    /// not cached, so a later call retries. If no daemon answers an
    /// authenticated ping, one is spawned and this polls until it is ready
    /// (or the ~15s deadline elapses).
    pub async fn ensure(&self) -> Result<String, String> {
        self.addr
            .get_or_try_init(|| async {
                let addr = match std::env::var("VOX_ORCHESTRATOR_DAEMON_SOCKET") {
                    Ok(s) if s.contains(':') => s,
                    _ => DEFAULT_DAEMON_ADDR.to_string(),
                };

                // A daemon may already be running — only adopt it if a
                // token-authenticated ping actually succeeds. This rejects
                // port-squatters: any process that merely answers ping (but
                // doesn't share our token) is not adopted.
                if let Some(existing_token) = read_token_file()
                    && OrchDaemonClient::with_token(addr.clone(), existing_token.clone())
                        .ping()
                        .await
                        .is_ok()
                {
                    let _ = self.token.set(existing_token);
                    return Ok(addr);
                }

                // Spawn a fresh daemon and keep the child alive. Stage from
                // target/ into ~/.vox/bin first so the running daemon exe
                // does not lock the target dir (os error 5 on rebuild).
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
                // environment — avoids a race with reading the token file
                // before the daemon has written it (the daemon still writes
                // the file too, for other clients to auto-resolve).
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

    /// The daemon auth token resolved as a side effect of
    /// [`OrchestratorDaemonEnsure::ensure`]. Call `ensure()` first; `None` if
    /// `ensure()` has not yet succeeded.
    pub async fn token(&self) -> Option<String> {
        self.token.get().cloned()
    }

    /// Build an [`OrchDaemonClient`] for `addr`, using the resolved token if
    /// available (falling back to the client's own best-effort token-file
    /// read otherwise). Convenience wrapper around `ensure()` + `token()`.
    pub async fn client(&self) -> Result<OrchDaemonClient, String> {
        let addr = self.ensure().await?;
        Ok(match self.token().await {
            Some(token) => OrchDaemonClient::with_token(addr, token),
            None => OrchDaemonClient::new(addr),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: constructing the helper and reading its (unset) token
    /// before `ensure()` is called must not panic.
    #[tokio::test]
    async fn token_is_none_before_ensure() {
        let ensure = OrchestratorDaemonEnsure::default();
        assert_eq!(ensure.token().await, None);
    }
}
