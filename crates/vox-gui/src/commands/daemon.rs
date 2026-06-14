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

use std::process::{Command, Stdio};

use tokio::sync::OnceCell;
use vox_cli_core::daemon_ipc::process_supervision::resolve_managed_binary_path;
use vox_config::timeouts::{D_15S, D_100MS};
use vox_orchestrator::orch_daemon::OrchDaemonClient;

/// Deadline for the spawned daemon to become reachable via ping.
const DAEMON_CONNECT_TIMEOUT: std::time::Duration = D_15S;
/// Poll interval while waiting for the daemon to start.
const DAEMON_POLL_INTERVAL: std::time::Duration = D_100MS;

/// Default loopback TCP address the GUI binds its orchestrator daemon to when
/// `VOX_ORCHESTRATOR_DAEMON_SOCKET` is not set to a TCP address.
const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:9745";

/// Tauri-managed holder for the one long-lived orchestrator daemon.
///
/// The resolved address is cached only on success (see [`PersistentDaemon::ensure`]),
/// and the spawned child (if we started one) is kept alive for the lifetime of
/// the app so the daemon is not reaped.
#[derive(Default)]
pub struct PersistentDaemon {
    addr: OnceCell<String>,
    child: std::sync::Mutex<Option<std::process::Child>>,
}

impl PersistentDaemon {
    /// Ensure one long-lived TCP daemon is reachable; returns its address.
    ///
    /// Cached on success (subsequent calls reuse the address); failures are not
    /// cached, so a later call retries. If no daemon answers a ping, one is
    /// spawned and we poll until it is ready (or a ~15s deadline elapses).
    // toestub-ignore(skeleton/untested-pub-api) — spawns/pings external vox-orchestrator-d process; covered by integration tests
    pub async fn ensure(&self) -> Result<String, String> {
        self.addr
            .get_or_try_init(|| async {
                let addr = match std::env::var("VOX_ORCHESTRATOR_DAEMON_SOCKET") {
                    Ok(s) if s.contains(':') => s,
                    _ => DEFAULT_DAEMON_ADDR.to_string(),
                };

                // A daemon is already running — reuse it.
                if OrchDaemonClient::new(addr.clone()).ping().await.is_ok() {
                    return Ok(addr);
                }

                // Spawn a fresh daemon and keep the child alive.
                let child = Command::new(resolve_managed_binary_path("vox-orchestrator-d"))
                    .env("VOX_ORCHESTRATOR_DAEMON_SOCKET", &addr)
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

                // Poll until the daemon answers a ping or the deadline elapses.
                let deadline = std::time::Instant::now() + DAEMON_CONNECT_TIMEOUT;
                while std::time::Instant::now() < deadline {
                    if OrchDaemonClient::new(addr.clone()).ping().await.is_ok() {
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
}
