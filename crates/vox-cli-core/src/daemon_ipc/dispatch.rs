//! Spawn a managed daemon, send one [`vox_foundation::protocol::DispatchRequest`], stream payloads.

use super::dispatch_protocol::{DispatchPayload, DispatchRequest, DispatchResponse};
use super::process_supervision::{resolve_managed_binary_path, terminate_process_tree};
use crate::fs_utils;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Leading text for daemon spawn failures from [`call_daemon`] and [`call_daemon_streaming`].
pub const DAEMON_SPAWN_FAILED_PREFIX: &str = "Failed to spawn daemon";

pub async fn call_daemon(
    daemon: &str,
    method: &str,
    params: serde_json::Value,
    auto_open: bool,
) -> anyhow::Result<serde_json::Value> {
    let daemon_path = resolve_managed_binary_path(daemon);

    let mut child = Command::new(&daemon_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "{} '{}': {}",
                DAEMON_SPAWN_FAILED_PREFIX,
                daemon_path.display(),
                e
            )
        })?;

    let mut stdin = child.stdin.take().expect("stdin was piped");
    let stdout = child.stdout.take().expect("stdout was piped");

    let req = DispatchRequest {
        id: uuid::Uuid::new_v4().to_string(),
        method: method.into(),
        params,
    };
    let json = serde_json::to_string(&req)? + "\n";
    stdin.write_all(json.as_bytes()).await?;
    stdin.flush().await?;
    drop(stdin);

    let mut reader = BufReader::new(stdout).lines();
    let mut final_result: serde_json::Value = serde_json::Value::Null;
    let mut had_error: Option<String> = None;
    let mut exit_code = 0i32;

    while let Ok(Some(line)) = reader.next_line().await {
        match serde_json::from_str::<DispatchResponse>(&line) {
            Ok(resp) => match resp.payload {
                DispatchPayload::Log { level, msg } => match level.as_str() {
                    "error" | "warn" => eprintln!("[{}] {}", level.to_uppercase(), msg),
                    _ => println!("[{}] {}", level.to_uppercase(), msg),
                },
                DispatchPayload::Diag {
                    severity,
                    message,
                    file,
                    line,
                    col,
                } => {
                    eprintln!(
                        "{}: {}:{}:{} — {}",
                        severity.to_uppercase(),
                        file,
                        line,
                        col,
                        message
                    );
                }
                DispatchPayload::Artifact { path } => {
                    println!("✓ artifact: {}", path);
                }
                DispatchPayload::Progress { percent, status } => {
                    println!("[{:.0}%] {}", percent, status);
                }
                DispatchPayload::Chunk { text } => {
                    use std::io::Write;
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                }
                DispatchPayload::Result { value } => {
                    final_result = value;
                }
                // Structured stream frames (orch.subscribe) are consumed by the
                // dedicated subscribe path; a one-shot call ignores them.
                DispatchPayload::Event { .. } => {}
                DispatchPayload::Error { message, code } => {
                    had_error = Some(format!("Daemon error (code {}): {}", code, message));
                }
                DispatchPayload::Done { exit } => {
                    exit_code = exit;
                    break;
                }
            },
            Err(_) => {
                emit_unstructured_daemon_line(&line, auto_open, false).await;
            }
        }
    }

    let _ = child.wait().await;

    if let Some(err) = had_error {
        anyhow::bail!("{}", err);
    }
    if exit_code != 0 {
        anyhow::bail!("Daemon '{}' exited with code {}", daemon, exit_code);
    }

    Ok(final_result)
}

pub async fn call_daemon_streaming(
    daemon: &str,
    method: &str,
    params: serde_json::Value,
    auto_open: bool,
) -> anyhow::Result<()> {
    let daemon_path = resolve_managed_binary_path(daemon);

    let mut child = Command::new(&daemon_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "{} '{}': {}",
                DAEMON_SPAWN_FAILED_PREFIX,
                daemon_path.display(),
                e
            )
        })?;

    let mut stdin = child.stdin.take().expect("stdin was piped");
    let stdout = child.stdout.take().expect("stdout was piped");

    let req = DispatchRequest {
        id: uuid::Uuid::new_v4().to_string(),
        method: method.into(),
        params,
    };
    let json = serde_json::to_string(&req)? + "\n";
    stdin.write_all(json.as_bytes()).await?;
    stdin.flush().await?;
    drop(stdin);

    let child_id = child.id();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok()
            && let Some(pid) = child_id
        {
            let _ = terminate_process_tree(pid);
        }
    });

    let mut reader = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        match serde_json::from_str::<DispatchResponse>(&line) {
            Ok(resp) => match resp.payload {
                DispatchPayload::Log { level, msg } => match level.as_str() {
                    "error" | "warn" => eprintln!("[{}] {}", level.to_uppercase(), msg),
                    _ => println!("[{}] {}", level.to_uppercase(), msg),
                },
                DispatchPayload::Diag {
                    severity,
                    message,
                    file,
                    line,
                    col,
                } => {
                    eprintln!(
                        "{}: {}:{}:{} — {}",
                        severity.to_uppercase(),
                        file,
                        line,
                        col,
                        message
                    );
                }
                DispatchPayload::Progress { percent, status } => {
                    println!("[{:.0}%] {}", percent, status);
                }
                DispatchPayload::Artifact { path } => {
                    println!("✓ {}", path);
                }
                DispatchPayload::Error { message, code } => {
                    eprintln!("[ERROR {}] {}", code, message);
                }
                DispatchPayload::Done { .. } => break,
                DispatchPayload::Chunk { text } => {
                    use std::io::Write;
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                }
                DispatchPayload::Result { .. } => {}
                DispatchPayload::Event { .. } => {}
            },
            Err(_) => {
                emit_unstructured_daemon_line(&line, auto_open, true).await;
            }
        }
    }

    let _ = child.wait().await;
    Ok(())
}

/// Spawn the managed `daemon` over stdio, send one [`DispatchRequest`] with the
/// given `method` (e.g. `orch_daemon_method::SUBSCRIBE`), then stream structured
/// [`DispatchPayload::Event`] values into `tx` until the daemon stops, the stream
/// ends, or the receiver is dropped.
///
/// Mirrors [`call_daemon_streaming`]'s spawn/child-handling model but forwards
/// events into a channel instead of printing to stdout. Returns `Ok(())` once the
/// loop exits and the child process has been terminated.
// toestub-ignore(skeleton/untested-pub-api) — spawns and streams from a child daemon process; covered by integration tests
pub async fn subscribe_daemon(
    daemon: &str,
    method: &str,
    params: serde_json::Value,
    tx: tokio::sync::mpsc::Sender<serde_json::Value>,
) -> anyhow::Result<()> {
    let daemon_path = resolve_managed_binary_path(daemon);

    let mut child = Command::new(&daemon_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "{} '{}': {}",
                DAEMON_SPAWN_FAILED_PREFIX,
                daemon_path.display(),
                e
            )
        })?;

    let mut stdin = child.stdin.take().expect("stdin was piped");
    let stdout = child.stdout.take().expect("stdout was piped");

    let req = DispatchRequest {
        id: uuid::Uuid::new_v4().to_string(),
        method: method.into(),
        params,
    };
    let json = serde_json::to_string(&req)? + "\n";
    stdin.write_all(json.as_bytes()).await?;
    stdin.flush().await?;
    drop(stdin);

    let mut reader = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        match serde_json::from_str::<DispatchResponse>(&line) {
            Ok(resp) => match resp.payload {
                DispatchPayload::Event { value } => {
                    if tx.send(value).await.is_err() {
                        // Receiver dropped; stop streaming.
                        break;
                    }
                }
                DispatchPayload::Error { message, code } => {
                    eprintln!("[ERROR {}] {}", code, message);
                    break;
                }
                DispatchPayload::Done { .. } => break,
                // Other variants (logs, progress, results, etc.) are not part of
                // the subscribe contract and are ignored.
                _ => {}
            },
            // Unstructured lines are not part of the subscribe contract; ignore.
            Err(_) => {}
        }
    }

    // Mirror call_daemon_streaming's child handling: best-effort terminate the
    // process tree, then reap.
    if let Some(pid) = child.id() {
        let _ = terminate_process_tree(pid);
    }
    let _ = child.wait().await;
    Ok(())
}

async fn emit_unstructured_daemon_line(line: &str, auto_open: bool, app_launched_banner: bool) {
    if let Some(pos) = line.find("[VOX_DASHBOARD_READY: ") {
        if auto_open {
            let url = &line[pos + 22..];
            if let Some(end) = url.find(']') {
                fs_utils::open_browser(url[..end].trim()).await;
            }
        }
        println!("\n  ↳  Dashboard ready: {}", line.trim());
    } else if line.contains("http://") || line.contains("App Launched at") {
        if auto_open && let Some(pos) = line.find("http://") {
            let rest = &line[pos..];
            let extracted_url = rest.split_whitespace().next().unwrap_or(rest);
            fs_utils::open_browser(extracted_url.trim_end_matches(']')).await;
        }
        if app_launched_banner {
            println!("\n  ↳  App launched: {}", line.trim());
        } else {
            println!("{}", line);
        }
    } else {
        println!("{}", line);
    }
}
