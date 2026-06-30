//! Tailscale Funnel backend.
//!
//! Requires `tailscale` CLI and an account with Funnel enabled.
//! Only ports 443, 8443, 10000 are supported by Tailscale Funnel.
//!
//! Unlike Cloudflare and localhost.run, Tailscale produces a *stable* URL
//! (`https://<machine>.<tailnet>.ts.net`) that persists across runs.

use crate::backend::{BackendKind, TunnelBackend, TunnelHandle, UrlStability};
use crate::error::{ShareError, ShareResult};
use async_trait::async_trait;
use std::time::Duration;

/// Ports supported by Tailscale Funnel. Others will be rejected at preflight.
const FUNNEL_PORTS: &[u16] = &[443, 8443, 10000];

#[derive(Debug, Default)]
pub struct TailscaleBackend;

impl TailscaleBackend {
    pub fn new() -> Self {
        Self
    }
}

/// Detect `tailscale` CLI on PATH. Returns its path if found.
pub fn detect_tailscale() -> Option<std::path::PathBuf> {
    which::which("tailscale").ok()
}

#[async_trait]
impl TunnelBackend for TailscaleBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Tailscale
    }

    async fn preflight(&self) -> ShareResult<()> {
        let ts = detect_tailscale().ok_or_else(|| {
            ShareError::BackendUnavailable(
                "tailscale",
                "tailscale CLI not found. Install from https://tailscale.com/download".into(),
            )
        })?;

        // Check daemon is running.
        let status = tokio::process::Command::new(&ts)
            .args(["status", "--json"])
            .output()
            .await
            .map_err(|e| {
                ShareError::BackendUnavailable("tailscale", format!("run tailscale status: {}", e))
            })?;

        if !status.status.success() {
            return Err(ShareError::BackendUnavailable(
                "tailscale",
                "tailscale daemon is not running. Start it with `tailscale up`".into(),
            ));
        }

        // Parse the status JSON to check BackendState.
        let json: serde_json::Value = serde_json::from_slice(&status.stdout).map_err(|e| {
            ShareError::BackendUnavailable("tailscale", format!("parse status JSON: {}", e))
        })?;

        let backend_state = json
            .get("BackendState")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        if backend_state != "Running" {
            return Err(ShareError::BackendUnavailable(
                "tailscale",
                format!(
                    "tailscale is not connected (state: {}). Run `tailscale up`",
                    backend_state
                ),
            ));
        }

        Ok(())
    }

    async fn start(
        &self,
        local_port: u16,
        _connect_timeout: Duration,
    ) -> ShareResult<TunnelHandle> {
        // Validate port is supported by Tailscale Funnel.
        if !FUNNEL_PORTS.contains(&local_port) {
            return Err(ShareError::Config(format!(
                "Tailscale Funnel only supports ports {:?}, got {}. \
                 Use --port 443, 8443, or 10000 with --backend tailscale.",
                FUNNEL_PORTS, local_port
            )));
        }

        let ts = detect_tailscale().ok_or_else(|| {
            ShareError::BackendUnavailable("tailscale", "tailscale CLI not found".into())
        })?;

        // Enable funnel for this port.
        let serve_out = tokio::process::Command::new(&ts)
            .args(["funnel", &local_port.to_string()])
            .output()
            .await
            .map_err(|e| ShareError::TunnelCreate(format!("tailscale funnel: {}", e)))?;

        if !serve_out.status.success() {
            let stderr = String::from_utf8_lossy(&serve_out.stderr);
            return Err(ShareError::TunnelCreate(format!(
                "tailscale funnel failed: {}",
                stderr
            )));
        }

        // Discover the public URL from `tailscale funnel status`.
        let status_out = tokio::process::Command::new(&ts)
            .args(["funnel", "status", "--json"])
            .output()
            .await
            .map_err(|e| ShareError::TunnelCreate(format!("tailscale funnel status: {}", e)))?;

        let public_url = parse_funnel_url(&status_out.stdout, local_port).ok_or_else(|| {
            ShareError::TunnelCreate(
                "could not determine Tailscale Funnel URL from status output".into(),
            )
        })?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // When shutdown is requested, disable the funnel.
        let ts_clone = ts.clone();
        let port_str = local_port.to_string();
        tokio::spawn(async move {
            let _ = shutdown_rx.await;
            let _ = tokio::process::Command::new(&ts_clone)
                .args(["funnel", "--bg", &port_str, "off"])
                .output()
                .await;
        });

        Ok(TunnelHandle::new(
            public_url,
            BackendKind::Tailscale,
            UrlStability::Stable,
            shutdown_tx,
        ))
    }
}

/// Parse the Tailscale Funnel public URL from `tailscale funnel status --json` output.
///
/// The JSON structure varies by Tailscale version. Try a few patterns:
/// - `SelfNode.DNSName` + `.ts.net` for the hostname
fn parse_funnel_url(stdout: &[u8], port: u16) -> Option<String> {
    if stdout.is_empty() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(stdout).ok()?;

    // Try to extract from common JSON shapes.
    // Pattern 1: { "SelfNode": { "DNSName": "machine.tailnet.ts.net." } }
    if let Some(dns) = json.pointer("/SelfNode/DNSName").and_then(|v| v.as_str()) {
        let host = dns.trim_end_matches('.');
        if host.ends_with(".ts.net") {
            return Some(format!("https://{}:{}", host, port));
        }
    }

    // Pattern 2: top-level "DNSName"
    if let Some(dns) = json.get("DNSName").and_then(|v| v.as_str()) {
        let host = dns.trim_end_matches('.');
        if host.ends_with(".ts.net") {
            return Some(format!("https://{}:{}", host, port));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_funnel_url_from_self_node() {
        let json = r#"{"SelfNode":{"DNSName":"my-machine.example-tailnet.ts.net."}}"#;
        let url = parse_funnel_url(json.as_bytes(), 443);
        assert_eq!(
            url,
            Some("https://my-machine.example-tailnet.ts.net:443".to_string())
        );
    }

    #[test]
    fn parse_funnel_url_returns_none_for_empty() {
        assert!(parse_funnel_url(b"", 443).is_none());
        assert!(parse_funnel_url(b"{}", 443).is_none());
    }

    #[test]
    fn funnel_ports_validation() {
        assert!(FUNNEL_PORTS.contains(&443));
        assert!(FUNNEL_PORTS.contains(&8443));
        assert!(FUNNEL_PORTS.contains(&10000));
        assert!(!FUNNEL_PORTS.contains(&7860));
    }
}
