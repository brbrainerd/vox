//! Coordinator: wires app + proxy + tunnel-backend together.

use crate::utils::share::auth::AuthMode;
use crate::utils::share::backend::{BackendKind, TunnelBackend, TunnelHandle};
use crate::utils::share::backends::cloudflare::CloudflareBackend;
use crate::utils::share::backends::lan::LanBackend;
use crate::utils::share::backends::localhost_run::LocalhostRunBackend;
use crate::utils::share::backends::tailscale::TailscaleBackend;
use crate::utils::share::error::{ShareError, ShareResult};
use crate::utils::share::proxy::{ProxyConfig, build_app as build_proxy_app};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpListener;

/// Configuration for a share session.
#[derive(Debug, Clone)]
pub struct ShareConfig {
    pub backend: BackendKind,
    /// The localhost port the bundled Vox app is listening on.
    pub upstream_port: u16,
    /// Where to bind the proxy (0 = OS-pick).
    pub proxy_port: u16,
    /// Auto-shutdown after this duration. None = unbounded.
    pub duration: Option<Duration>,
    /// Path to the bundled app binary. None = assume already running.
    pub app_binary: Option<PathBuf>,
    /// Time to wait for the tunnel to come up.
    pub connect_timeout: Duration,
    /// When true and backend is Cloudflare, fall back to localhost.run on failure.
    pub allow_fallback: bool,
    /// Authentication mode for the share session.
    pub auth_mode: AuthMode,
    /// If true, don't auto-switch away from Cloudflare even if SSE routes detected.
    pub allow_buffered_streaming: bool,
}

/// An active share session. Drop or call `shutdown` to clean up.
pub struct ShareSession {
    pub tunnel_handle: TunnelHandle,
    pub proxy_port: u16,
    /// The public URL to share with users — may include auth token decoration.
    pub public_url: String,
    proxy_shutdown: tokio::sync::oneshot::Sender<()>,
    /// Receives a signal when the duration elapses (None if duration is unbounded).
    duration_done_rx: Option<tokio::sync::mpsc::Receiver<()>>,
    _app_child: Option<tokio::process::Child>,
}

impl ShareSession {
    pub async fn start(cfg: ShareConfig) -> ShareResult<Self> {
        // Optionally spawn the app binary.
        let app_child = if let Some(path) = &cfg.app_binary {
            let child = tokio::process::Command::new(path)
                .env("PORT", cfg.upstream_port.to_string())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| ShareError::Config(format!("spawn app: {}", e)))?;
            Some(child)
        } else {
            None
        };

        // Bind the proxy listener.
        let bind_addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], cfg.proxy_port));
        let listener = TcpListener::bind(bind_addr).await?;
        let actual_proxy_port = listener.local_addr()?.port();

        let proxy_cfg = ProxyConfig {
            upstream_addr: SocketAddr::from(([127, 0, 0, 1], cfg.upstream_port)),
            bind_addr,
            auth_mode: cfg.auth_mode.clone(),
        };
        let proxy_app = build_proxy_app(proxy_cfg);

        let (proxy_shutdown_tx, proxy_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, proxy_app);
            let proxy_shutdown_rx = async move {
                let _ = proxy_shutdown_rx.await;
            };
            tokio::pin!(proxy_shutdown_rx);
            tokio::select! {
                res = server => { let _ = res; }
                _ = &mut proxy_shutdown_rx => {}
            }
        });

        // S6: SSE detection — if Cloudflare and SSE routes found, switch to localhost.run.
        if matches!(cfg.backend, BackendKind::Cloudflare)
            && !cfg.allow_buffered_streaming
            && crate::utils::share::sse_detect::has_sse_routes(cfg.upstream_port).await
        {
            println!(
                "[vox share] App uses streaming (SSE); auto-selected --backend localhost-run for SSE compatibility"
            );
            println!(
                "[vox share] Use --allow-buffered-streaming to keep Cloudflare (SSE will be buffered)"
            );
            let fallback = make_backend(BackendKind::LocalhostRun);
            fallback.preflight().await?;
            let tunnel_handle = fallback
                .start(actual_proxy_port, cfg.connect_timeout)
                .await?;
            let public_url = cfg.auth_mode.decorate_url(&tunnel_handle.public_url);
            let duration_done_rx = if let Some(d) = cfg.duration {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(crate::utils::share::lifecycle::run_countdown(d, tx));
                tokio::spawn(crate::utils::share::lifecycle::run_countdown_printer(d));
                Some(rx)
            } else {
                None
            };
            return Ok(ShareSession {
                tunnel_handle,
                proxy_port: actual_proxy_port,
                public_url,
                proxy_shutdown: proxy_shutdown_tx,
                duration_done_rx,
                _app_child: app_child,
            });
        }

        // Bring up the tunnel backend.
        let backend: Box<dyn TunnelBackend> = make_backend(cfg.backend);
        backend.preflight().await?;
        let tunnel_handle = match backend.start(actual_proxy_port, cfg.connect_timeout).await {
            Ok(h) => h,
            Err(e) if cfg.allow_fallback && matches!(cfg.backend, BackendKind::Cloudflare) => {
                println!(
                    "[vox share] Cloudflare unavailable ({}); falling back to localhost.run",
                    e
                );
                let fallback = make_backend(BackendKind::LocalhostRun);
                fallback.preflight().await?;
                fallback
                    .start(actual_proxy_port, cfg.connect_timeout)
                    .await?
            }
            Err(e) => return Err(e),
        };

        // Optional auto-shutdown timer.
        let duration_done_rx = if let Some(d) = cfg.duration {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(crate::utils::share::lifecycle::run_countdown(d, tx));
            Some(rx)
        } else {
            None
        };

        let public_url = cfg.auth_mode.decorate_url(&tunnel_handle.public_url);
        Ok(ShareSession {
            tunnel_handle,
            proxy_port: actual_proxy_port,
            public_url,
            proxy_shutdown: proxy_shutdown_tx,
            duration_done_rx,
            _app_child: app_child,
        })
    }

    pub async fn shutdown(self) {
        self.tunnel_handle.shutdown();
        // Drop duration_done_rx — this will close the channel and the countdown task will
        // exit on the next send attempt (already completed or about to be dropped).
        drop(self.duration_done_rx);
        let _ = self.proxy_shutdown.send(());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    /// Wait for the session to end: either Ctrl+C or duration elapsed.
    /// Handles shutdown cleanup automatically.
    pub async fn wait(self) {
        // Extract the receiver so we can move it into the select without
        // holding a borrow on `self`.
        let ShareSession {
            tunnel_handle,
            proxy_port,
            public_url,
            proxy_shutdown,
            mut duration_done_rx,
            _app_child,
        } = self;

        let done = async {
            if let Some(ref mut rx) = duration_done_rx {
                rx.recv().await;
            } else {
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("[vox share] Ctrl+C received; shutting down.");
            }
            _ = done => {
                println!("[vox share] Duration elapsed; shutting down.");
            }
        }

        // Reassemble and shut down.
        let session = ShareSession {
            tunnel_handle,
            proxy_port,
            public_url,
            proxy_shutdown,
            duration_done_rx,
            _app_child,
        };
        session.shutdown().await;
    }
}

fn make_backend(kind: BackendKind) -> Box<dyn TunnelBackend> {
    match kind {
        BackendKind::Lan => Box::new(LanBackend::new()),
        BackendKind::Cloudflare => Box::new(CloudflareBackend::new()),
        BackendKind::LocalhostRun => Box::new(LocalhostRunBackend::new()),
        BackendKind::Tailscale => Box::new(TailscaleBackend::new()),
    }
}
