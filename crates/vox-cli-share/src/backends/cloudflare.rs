//! Cloudflare Quick Tunnel backend.
//!
//! Spawns `cloudflared tunnel --url http://127.0.0.1:<port> --no-autoupdate`,
//! scans its stderr for the `*.trycloudflare.com` URL, and returns a
//! [`TunnelHandle`] once the URL is known.
//!
//! Limitations (documented in RESEARCH.md):
//! - 200 in-flight request cap (breaks long-polling / SSE — see S6 auto-switch)
//! - New URL each run (per-session stability)
//! - `cloudflared` Go binary, Apache-2.0, lazy-downloaded by `binary_cache`

use crate::backend::{BackendKind, TunnelBackend, TunnelHandle, UrlStability};
use crate::binary_cache::ensure_cloudflared;
use crate::error::{ShareError, ShareResult};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Default)]
pub struct CloudflareBackend;

impl CloudflareBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TunnelBackend for CloudflareBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Cloudflare
    }

    async fn preflight(&self) -> ShareResult<()> {
        // Verify the binary can be located (download if needed, or validate override path).
        // We don't spawn it yet; just confirm it's accessible.
        ensure_cloudflared().await.map(|_| ())
    }

    async fn start(&self, local_port: u16, connect_timeout: Duration) -> ShareResult<TunnelHandle> {
        let bin = ensure_cloudflared().await?;

        let mut child = Command::new(&bin)
            .args([
                "tunnel",
                "--url",
                &format!("http://127.0.0.1:{}", local_port),
                "--no-autoupdate",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ShareError::TunnelCreate(format!("spawn cloudflared: {}", e)))?;

        let stderr = child.stderr.take().expect("stderr was piped");
        let mut lines = BufReader::new(stderr).lines();

        // Channel to signal the URL has been found.
        let (url_tx, url_rx) = tokio::sync::oneshot::channel::<String>();
        // Channel to signal shutdown.
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn a task that scans stderr for the URL, then forwards child output.
        tokio::spawn(async move {
            let mut url_tx = Some(url_tx);
            loop {
                tokio::select! {
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(l)) => {
                                if let Some(url) = extract_trycloudflare_url(&l)
                                    && let Some(tx) = url_tx.take() {
                                        let _ = tx.send(url);
                                    }
                            }
                            _ => break,
                        }
                    }
                    _ = &mut shutdown_rx => {
                        let _ = child.kill().await;
                        break;
                    }
                }
            }
        });

        // Wait for the URL to appear within the timeout.
        let public_url = tokio::time::timeout(connect_timeout, url_rx)
            .await
            .map_err(|_| {
                ShareError::TunnelCreate(format!(
                    "cloudflared did not produce a URL within {:?}",
                    connect_timeout
                ))
            })?
            .map_err(|_| {
                ShareError::TunnelCreate("URL channel closed before URL was received".into())
            })?;

        Ok(TunnelHandle::new(
            public_url,
            BackendKind::Cloudflare,
            UrlStability::PerSession,
            shutdown_tx,
        ))
    }
}

/// Extract a `*.trycloudflare.com` URL from a cloudflared log line.
fn extract_trycloudflare_url(line: &str) -> Option<String> {
    // Pattern: https://SOMETHING.trycloudflare.com
    let marker = "https://";
    let suffix = ".trycloudflare.com";
    let start = line.find(marker)?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '|')
        .unwrap_or(rest.len());
    let candidate = &rest[..end];
    if candidate.ends_with(suffix) || candidate.contains(suffix) {
        // Trim any trailing non-URL chars
        let clean =
            candidate.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
        Some(clean.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::extract_trycloudflare_url;

    #[test]
    fn extracts_url_from_cloudflared_log_line() {
        let line = "2024-01-01T00:00:00Z INF |  https://fancy-name.trycloudflare.com  |";
        assert_eq!(
            extract_trycloudflare_url(line),
            Some("https://fancy-name.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrelated_line() {
        assert_eq!(extract_trycloudflare_url("INF tunnel registered"), None);
    }
}
