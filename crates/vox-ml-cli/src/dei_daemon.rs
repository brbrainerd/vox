//! JSON-line RPC to `vox-orchestrator-d` for vox-ml-cli (corpus / AI-assisted flows).
//! Wire types are [`vox_protocol`](https://docs.rs/vox-protocol) — shared with `vox-cli` and the daemon.

use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

pub const BINARY: &str = "vox-orchestrator-d";

/// Method constants (`ai.check`, `config.get`, …) — SSOT in `vox-protocol`.
pub use vox_foundation::protocol::dei_method as method;

pub async fn call(method: &str, params: Value, _auto_open: bool) -> anyhow::Result<Value> {
    let mut child = Command::new(BINARY)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn daemon '{}': {}", BINARY, e))?;

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
    let mut final_result = Value::Null;
    let mut had_error: Option<String> = None;
    let mut exit_code = 0i32;

    while let Ok(Some(line)) = reader.next_line().await {
        match serde_json::from_str::<DispatchResponse>(&line) {
            Ok(resp) => match resp.payload {
                DispatchPayload::Log { level, msg } => {
                    eprintln!("[{}] {}", level.to_uppercase(), msg);
                }
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
                    eprintln!("✓ artifact: {}", path);
                }
                DispatchPayload::Progress { percent, status } => {
                    eprintln!("[{:.0}%] {}", percent, status);
                }
                DispatchPayload::Chunk { text } => {
                    use std::io::Write;
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                }
                DispatchPayload::Result { value } => {
                    final_result = value;
                }
                DispatchPayload::Error { message, code } => {
                    had_error = Some(format!("Daemon error (code {}): {}", code, message));
                }
                DispatchPayload::Done { exit } => {
                    exit_code = exit;
                    break;
                }
            },
            Err(_) => {
                // Unstructured daemon stdout line — mirror thin CLI behavior for resilience.
                eprintln!("{}", line);
            }
        }
    }

    let _ = child.wait().await;

    if let Some(err) = had_error {
        anyhow::bail!("{}", err);
    }
    if exit_code != 0 {
        anyhow::bail!("Daemon '{}' exited with code {}", BINARY, exit_code);
    }

    Ok(final_result)
}
