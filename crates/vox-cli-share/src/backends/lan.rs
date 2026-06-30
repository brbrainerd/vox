//! LAN backend — return a LAN URL without any actual tunneling.
//!
//! The coordinator binds the proxy to 0.0.0.0; this backend's job is to
//! discover a routable LAN IP for the user-facing URL and produce a
//! [`TunnelHandle`]. No child process is spawned.

use crate::backend::{BackendKind, TunnelBackend, TunnelHandle, UrlStability};
use crate::error::ShareResult;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct LanBackend;

impl LanBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TunnelBackend for LanBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Lan
    }

    async fn preflight(&self) -> ShareResult<()> {
        Ok(())
    }

    async fn start(
        &self,
        local_port: u16,
        _connect_timeout: Duration,
    ) -> ShareResult<TunnelHandle> {
        let lan_ip = detect_lan_ip().unwrap_or_else(|| "0.0.0.0".to_string());
        let public_url = format!("http://{}:{}", lan_ip, local_port);
        let (tx, _rx) = tokio::sync::oneshot::channel();
        // LAN backend has no background task; rx is dropped immediately.
        // Coordinator owns the actual server bind; this handle is informational.
        Ok(TunnelHandle::new(
            public_url,
            BackendKind::Lan,
            UrlStability::Stable,
            tx,
        ))
    }
}

/// Best-effort discovery of a routable LAN IPv4 address.
///
/// Opens a UDP socket toward a public IP; the OS picks a routable local address
/// as the source without sending any packets.
fn detect_lan_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let local_addr = sock.local_addr().ok()?;
    let ip = local_addr.ip();
    if ip.is_unspecified() || ip.is_loopback() {
        None
    } else {
        Some(ip.to_string())
    }
}
