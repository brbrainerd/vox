//! Compiler, build, lint, and test tool handlers for the Vox MCP server.
//!
//! Covers: validate_file, run_tests, check_workspace, test_all, build_crate,
//! lint_crate, coverage_report, generate_vox_code.

use std::path::PathBuf;

use crate::params::{DiagnosticInfo, RunTestsParams, ToolResult, ValidateFileParams, ValidateResponse};

/// Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR).
pub fn validate_file(params: ValidateFileParams) -> String {
    let path = PathBuf::from(&params.path);

    if !path.exists() {
        return ToolResult::<ValidateResponse>::err(format!("file not found: {}", params.path))
            .to_json();
    }

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            return ToolResult::<ValidateResponse>::err(format!("failed to read file: {e}"))
                .to_json()
        }
    };

    let diagnostics = vox_lsp::validate_document(&text);
    let infos: Vec<DiagnosticInfo> = diagnostics
        .iter()
        .map(|d| DiagnosticInfo {
            severity: match d.severity {
                Some(s) if s == tower_lsp_server::ls_types::DiagnosticSeverity::ERROR => {
                    "error".to_string()
                }
                _ => "warning".to_string(),
            },
            message: d.message.clone(),
            source: d.source.clone().unwrap_or_default(),
            start_line: d.range.start.line,
            start_col: d.range.start.character,
            end_line: d.range.end.line,
            end_col: d.range.end.character,
        })
        .collect();

    ToolResult::ok(ValidateResponse {
        count: infos.len(),
        diagnostics: infos,
    })
    .to_json()
}

/// Run `cargo test` for a specific crate.
pub fn run_tests(params: RunTestsParams) -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("test").arg("-p").arg(&params.crate_name);

    if let Some(filter) = &params.test_filter {
        cmd.arg("--").arg(filter);
    }

    cmd.arg("--").arg("--nocapture");

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}");

            if output.status.success() {
                ToolResult::ok(combined).to_json()
            } else {
                ToolResult::<String>::err(combined).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err(format!("failed to run cargo test: {e}")).to_json(),
    }
}

/// Run `cargo check` for the entire workspace.
pub fn check_workspace() -> String {
    let output = std::process::Command::new("cargo")
        .arg("check")
        .arg("--workspace")
        .arg("--message-format=short")
        .output();

    match output {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                ToolResult::ok("workspace check passed".to_string()).to_json()
            } else {
                ToolResult::<String>::err(format!("check failed:\n{stderr}")).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err(format!("failed to run cargo check: {e}")).to_json(),
    }
}

/// Run `cargo test` for the entire workspace.
pub fn test_all() -> String {
    let output = std::process::Command::new("cargo")
        .arg("test")
        .arg("--workspace")
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}");

            if output.status.success() {
                ToolResult::ok(combined).to_json()
            } else {
                ToolResult::<String>::err(combined).to_json()
            }
        }
        Err(e) => {
            ToolResult::<String>::err(format!("failed to run cargo test --workspace: {e}"))
                .to_json()
        }
    }
}

/// Run `cargo build` for a crate or the whole workspace.
pub fn build_crate(crate_name: Option<&str>) -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    if let Some(c) = crate_name {
        cmd.args(["-p", c]);
    } else {
        cmd.arg("--workspace");
    }

    match cmd.output() {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if output.status.success() {
                ToolResult::ok(format!("Build succeeded.\n{stdout}")).to_json()
            } else {
                ToolResult::<String>::err(format!("Build failed:\n{stderr}")).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err(format!("failed to run cargo build: {e}")).to_json(),
    }
}

/// Run `cargo clippy` and TOESTUB for a crate or the whole workspace.
pub fn lint_crate(crate_name: Option<&str>) -> String {
    let mut clippy_cmd = std::process::Command::new("cargo");
    clippy_cmd.arg("clippy");
    if let Some(c) = crate_name {
        clippy_cmd.args(["-p", c]);
    } else {
        clippy_cmd.arg("--workspace");
    }
    clippy_cmd.args(["--", "-D", "warnings"]);

    let clippy_out = match clippy_cmd.output() {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                "Clippy clean.".to_string()
            } else {
                format!("Clippy errors:\n{stderr}")
            }
        }
        Err(e) => format!("failed to run cargo clippy: {e}"),
    };

    // TOESTUB architectural check
    use vox_toestub::{Severity, ToestubConfig, ToestubEngine};
    let root = if let Some(c) = crate_name {
        let p = PathBuf::from("crates").join(c);
        if p.exists() { p } else { PathBuf::from(".") }
    } else {
        PathBuf::from(".")
    };

    let ts_config = ToestubConfig {
        roots: vec![root],
        min_severity: Severity::Warning,
        schema_path: Some(PathBuf::from("vox-schema.json")),
        ..ToestubConfig::default()
    };
    let ts_engine = ToestubEngine::new(ts_config);
    let (_, ts_report) = ts_engine.run_and_report();

    let combined = format!(
        "### 📎 Clippy Results\n{}\n\n### 🦶 TOESTUB Architectural Scan\n{}",
        clippy_out, ts_report
    );

    ToolResult::ok(combined).to_json()
}

/// Run `cargo llvm-cov` or `cargo tarpaulin` for code coverage.
pub fn coverage_report(crate_name: Option<&str>) -> String {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("llvm-cov");
    if let Some(c) = crate_name {
        cmd.args(["-p", c]);
    }
    cmd.args(["--text"]);

    match cmd.output() {
        Ok(output) if output.status.success() => {
            ToolResult::ok(String::from_utf8_lossy(&output.stdout).to_string()).to_json()
        }
        _ => ToolResult::<String>::err(
            "Coverage tool (llvm-cov or tarpaulin) not installed. Run `cargo install cargo-llvm-cov`."
                .to_string(),
        )
        .to_json(),
    }
}

/// Generate validated Vox code using the QWEN inference server.
pub async fn generate_vox_code(args: serde_json::Value) -> String {
    let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let validate = args
        .get("validate")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let max_retries = args
        .get("max_retries")
        .and_then(|v| v.as_u64())
        .unwrap_or(3);

    if prompt.is_empty() {
        return ToolResult::<String>::err("Missing 'prompt' parameter").to_json();
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => return ToolResult::<String>::err(format!("HTTP client error: {e}")).to_json(),
    };

    let body = serde_json::json!({
        "prompt": prompt,
        "validate": validate,
        "max_retries": max_retries,
    });

    match client
        .post("http://127.0.0.1:7863/generate")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.text().await {
                    Ok(text) => {
                        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text) {
                            ToolResult::ok(result).to_json()
                        } else {
                            ToolResult::ok(text).to_json()
                        }
                    }
                    Err(e) => ToolResult::<String>::err(format!("Response read error: {e}")).to_json(),
                }
            } else {
                ToolResult::<String>::err(format!(
                    "Inference server error ({}). Is it running? Start with: python scripts/vox_inference.py --serve",
                    resp.status()
                ))
                .to_json()
            }
        }
        Err(_) => ToolResult::<String>::err(
            "Cannot connect to inference server at localhost:7863. Start it with: python scripts/vox_inference.py --serve"
        )
        .to_json(),
    }
}
