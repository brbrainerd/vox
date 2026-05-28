//! Backend trait + kind enum.

use crate::utils::share::error::ShareResult;
use async_trait::async_trait;
use std::str::FromStr;
use std::time::Duration;

/// Which backend to use for the share session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// LAN-only: bind 0.0.0.0; no internet exposure.
    Lan,
    /// Cloudflare Quick Tunnel via `*.trycloudflare.com` (default for `vox share`).
    #[default]
    Cloudflare,
    /// SSH-based public URL via `*.lhr.life`.
    LocalhostRun,
    /// Tailscale Funnel via `*.ts.net` (requires Tailscale account + funnel enabled).
    Tailscale,
}

impl FromStr for BackendKind {
    type Err = crate::utils::share::error::ShareError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lan" => Ok(Self::Lan),
            "cloudflare" => Ok(Self::Cloudflare),
            "localhost-run" => Ok(Self::LocalhostRun),
            "tailscale" => Ok(Self::Tailscale),
            other => Err(crate::utils::share::error::ShareError::InvalidBackend(other.to_string())),
        }
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Lan => "lan",
            Self::Cloudflare => "cloudflare",
            Self::LocalhostRun => "localhost-run",
            Self::Tailscale => "tailscale",
        })
    }
}

/// A handle to an active tunnel session. Drop = shutdown.
#[derive(Debug)]
pub struct TunnelHandle {
    /// The public URL the user can share. For LAN backend this is `http://<lan-ip>:<port>`.
    pub public_url: String,
    /// Backend that produced this handle.
    pub backend: BackendKind,
    /// Hint about how stable this URL is across reconnects.
    pub url_stability: UrlStability,
    /// Shutdown channel — sender dropped triggers backend shutdown.
    shutdown: tokio::sync::oneshot::Sender<()>,
}

#[derive(Debug, Clone, Copy)]
pub enum UrlStability {
    /// Same URL every run for the same machine/account (Tailscale, registered cloudflared).
    Stable,
    /// New URL each run (Quick Tunnel, anonymous localhost.run).
    PerSession,
}

impl TunnelHandle {
    pub fn new(
        public_url: String,
        backend: BackendKind,
        url_stability: UrlStability,
        shutdown: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self {
            public_url,
            backend,
            url_stability,
            shutdown,
        }
    }

    /// Trigger graceful shutdown. Idempotent.
    pub fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

/// A backend creates and manages a tunnel from `127.0.0.1:<port>` to a public URL.
#[async_trait]
pub trait TunnelBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    /// Verify prerequisites (binary present, account authorized, etc.). Called before `start`.
    async fn preflight(&self) -> ShareResult<()>;

    /// Start the tunnel. Returns once the public URL is known and routable.
    /// `local_port` is the localhost port the backend should forward.
    /// `connect_timeout` is the max time to wait for the URL to become available.
    async fn start(&self, local_port: u16, connect_timeout: Duration) -> ShareResult<TunnelHandle>;
}
