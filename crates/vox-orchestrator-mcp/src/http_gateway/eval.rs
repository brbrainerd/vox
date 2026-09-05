use super::*;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::time::timeout;

#[derive(Debug, Deserialize)]
pub(super) struct EvalRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EvalResponse {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

const MAX_EVAL_CODE_BYTES: usize = 65_536;

fn validate_eval_request(req: &EvalRequest) -> Result<(), (axum::http::StatusCode, String)> {
    if req.code.len() > MAX_EVAL_CODE_BYTES {
        return Err((
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            format!("code payload exceeds {} byte limit", MAX_EVAL_CODE_BYTES),
        ));
    }
    Ok(())
}

pub(super) async fn http_eval(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<EvalRequest>,
) -> Response {
    if let Err((status, msg)) = validate_eval_request(&req) {
        return (status, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    let identity = request_identity(&state, &connect.0, &headers);

    if state.public_eval_enabled {
        if let Err(msg) = enforce_https_requirement(&state, &headers) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
        if state.public_eval_rate_limiter.check_key(&identity).is_err() {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(
                    serde_json::json!({ "error": "rate limit exceeded for public eval (10/min)" }),
                ),
            )
                .into_response();
        }
    } else {
        if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
            return *resp;
        }

        let _role = match resolve_access_role(&state, &headers, Some(&connect.0)) {
            Ok(r) => r,
            Err(msg) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response();
            }
        };
    }

    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return Json(EvalResponse {
                success: false,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("Failed to create tempdir: {}", e)),
            })
            .into_response();
        }
    };

    let file_path = dir.path().join("eval.vox");
    if let Err(e) = tokio::fs::write(&file_path, &req.code).await {
        return Json(EvalResponse {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("Failed to write file: {}", e)),
        })
        .into_response();
    }

    let mut cmd =
        tokio::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "vox".into()));
    // Use standard interpreter for pure computation (fastest)
    cmd.arg("run").arg("--interp").arg(&file_path);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // Hard execution boundary: 5 seconds
    let exec = timeout(vox_config::timeouts::D_5S, cmd.output()).await;

    let _ = dir.close();

    match exec {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Json(EvalResponse {
                success: output.status.success(),
                stdout,
                stderr,
                error: None,
            })
            .into_response()
        }
        Ok(Err(e)) => Json(EvalResponse {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("Execution failed: {}", e)),
        })
        .into_response(),
        Err(_) => Json(EvalResponse {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("Execution timed out after 5 seconds.".to_string()),
        })
        .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_request_rejects_oversized_code() {
        let big_code = "x".repeat(65_537);
        let req = EvalRequest { code: big_code };
        let result = validate_eval_request(&req);
        assert!(result.is_err(), "oversized code must be rejected");
    }

    #[test]
    fn eval_request_accepts_normal_code() {
        let req = EvalRequest {
            code: "println!(\"hello\")".to_string(),
        };
        let result = validate_eval_request(&req);
        assert!(result.is_ok(), "normal code must be accepted");
    }
}
