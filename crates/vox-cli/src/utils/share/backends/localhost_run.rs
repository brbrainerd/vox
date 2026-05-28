//! localhost.run SSH backend.
//!
//! No client binary to ship: uses the system's stock OpenSSH (`ssh`).
//! Spawns: `ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=60
//!           -R 80:localhost:<port> nokey@localhost.run`
//! Parses the tunnel URL from stdout (line contains `https://` + `.lhr.life`).

use crate::utils::share::backend::{BackendKind, TunnelBackend, TunnelHandle, UrlStability};
use crate::utils::share::error::{ShareError, ShareResult};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Detect whether `ssh` is available on PATH.
pub fn detect_ssh() -> Option<std::path::PathBuf> {
    which_ssh()
}

fn which_ssh() -> Option<std::path::PathBuf> {
    let candidates = if cfg!(windows) {
        vec!["ssh.exe", "ssh"]
    } else {
        vec!["ssh"]
    };
    for name in candidates {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }
    None
}

#[derive(Debug, Default)]
pub struct LocalhostRunBackend;

impl LocalhostRunBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TunnelBackend for LocalhostRunBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::LocalhostRun
    }

    async fn preflight(&self) -> ShareResult<()> {
        detect_ssh().map(|_| ()).ok_or_else(|| {
            ShareError::BackendUnavailable(
                "localhost-run",
                "ssh not found on PATH. Install OpenSSH: https://www.openssh.com/".into(),
            )
        })
    }

    async fn start(&self, local_port: u16, connect_timeout: Duration) -> ShareResult<TunnelHandle> {
        let ssh = detect_ssh().ok_or_else(|| {
            ShareError::BackendUnavailable("localhost-run", "ssh not found on PATH".into())
        })?;

        let remote_spec = format!("80:localhost:{}", local_port);
        let mut child = Command::new(&ssh)
            .args([
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "ServerAliveInterval=60",
                "-o",
                "ExitOnForwardFailure=yes",
                "-R",
                &remote_spec,
                "nokey@localhost.run",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ShareError::TunnelCreate(format!("spawn ssh: {}", e)))?;

        let stdout = child.stdout.take().expect("stdout piped");
        let mut lines = BufReader::new(stdout).lines();

        let (url_tx, url_rx) = tokio::sync::oneshot::channel::<String>();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let mut url_tx = Some(url_tx);
            loop {
                tokio::select! {
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(l)) => {
                                if let Some(url) = extract_lhr_life_url(&l)
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

        let public_url = tokio::time::timeout(connect_timeout, url_rx)
            .await
            .map_err(|_| {
                ShareError::TunnelCreate(format!(
                    "localhost.run did not produce a URL within {:?}",
                    connect_timeout
                ))
            })?
            .map_err(|_| ShareError::TunnelCreate("URL channel closed".into()))?;

        Ok(TunnelHandle::new(
            public_url,
            BackendKind::LocalhostRun,
            UrlStability::PerSession,
            shutdown_tx,
        ))
    }
}

fn extract_lhr_life_url(line: &str) -> Option<String> {
    let marker = "https://";
    let suffix = ".lhr.life";
    let start = line.find(marker)?;
    let rest = &line[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    let candidate = &rest[..end];
    if candidate.contains(suffix) {
        Some(
            candidate
                .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                .to_string(),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::extract_lhr_life_url;

    #[test]
    fn extracts_url_from_localhost_run_output() {
        let line = "https://fancy-name.lhr.life tunneled with tls termination";
        assert_eq!(
            extract_lhr_life_url(line),
            Some("https://fancy-name.lhr.life".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrelated_line() {
        assert_eq!(
            extract_lhr_life_url("Pseudo-terminal will not be allocated"),
            None
        );
    }
}
