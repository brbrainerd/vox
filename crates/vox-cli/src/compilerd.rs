//! `vox-compilerd` — stdio JSON dispatcher for long-lived and one-shot compiler RPC.
//!
//! Reads **one line** per invocation (in practice the client sends a single request and closes
//! stdin). Emits [`crate::dispatch_protocol::DispatchResponse`] JSON lines on stdout until `Done`
//! or, for `dev`, until the process is interrupted.
//!
//! Plain `println!` from subcommands may interleave with JSON lines; the CLI client treats
//! non-JSON lines as human-readable output.

use crate::cli_args::{BuildMode, BundleMode};
use crate::config;
use crate::dispatch_protocol::{DispatchPayload, DispatchRequest, DispatchResponse};
use crate::watcher;
use anyhow::Context;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use vox_bounded_fs::read_utf8_path_capped;

#[derive(Debug, Deserialize)]
struct BuildParams {
    file: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CheckParams {
    file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct BundleParams {
    file: PathBuf,
    out_dir: PathBuf,
    target: Option<String>,
    #[serde(default = "default_release")]
    release: bool,
    #[serde(default)]
    mode: BundleMode,
}

fn default_release() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct FmtParams {
    file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct DocParams {
    file: PathBuf,
    out_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct TestParams {
    file: PathBuf,
    filter: Option<String>,
    forall_iterations: Option<u32>,
    #[serde(default)]
    coverage: bool,
    #[serde(default)]
    update_snapshots: bool,
}

#[derive(Debug, Deserialize)]
struct RunParams {
    file: PathBuf,
    #[serde(default)]
    open: bool,
    #[serde(default = "serde_default_http_port")]
    port: u16,
    #[serde(default)]
    args: Vec<String>,
    /// `auto`, `app`, or `script` — same semantics as `vox run --mode`.
    #[serde(default = "default_run_mode_str")]
    mode: String,
}

fn default_run_mode_str() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize)]
struct ProfileParams {
    file: PathBuf,
    #[serde(default)]
    json: bool,
    #[serde(default)]
    no_cache: bool,
}

#[derive(Debug, Deserialize)]
struct DevParams {
    file: String,
    out_dir: String,
    #[serde(default = "serde_default_http_port")]
    port: u16,
    #[serde(default)]
    open: bool,
    /// Same as `vox build --target` (`fullstack` | `server` | `client`).
    #[serde(default)]
    target: Option<crate::cli_args::BuildTargetArg>,
}

fn serde_default_http_port() -> u16 {
    config::DEFAULT_PORT
}

async fn write_resp(id: &str, payload: DispatchPayload) -> anyhow::Result<()> {
    let r = DispatchResponse {
        id: id.to_string(),
        payload,
    };
    let line = serde_json::to_string(&r)? + "\n";
    let mut out = tokio::io::stdout();
    out.write_all(line.as_bytes()).await?;
    out.flush().await?;
    Ok(())
}

async fn finish_ok(id: &str, value: Value) -> anyhow::Result<()> {
    write_resp(id, DispatchPayload::Result { value }).await?;
    write_resp(id, DispatchPayload::Done { exit: 0 }).await?;
    Ok(())
}

async fn finish_err(id: &str, e: anyhow::Error) -> anyhow::Result<()> {
    write_resp(
        id,
        DispatchPayload::Error {
            message: e.to_string(),
            code: 1,
        },
    )
    .await?;
    write_resp(id, DispatchPayload::Done { exit: 1 }).await?;
    Ok(())
}

/// Daemon entry: read requests from stdin until EOF.
pub async fn run() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut buf = String::new();

    loop {
        buf.clear();
        let n = reader.read_line(&mut buf).await?;
        if n == 0 {
            break;
        }
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: DispatchRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("vox-compilerd: invalid JSON request: {e}");
                continue;
            }
        };

        if let Err(e) = dispatch_one(&req).await {
            finish_err(&req.id, e).await?;
        }
    }

    Ok(())
}

async fn dispatch_one(req: &DispatchRequest) -> anyhow::Result<()> {
    match req.method.as_str() {
        "build" => handle_build(req).await,
        "check" => handle_check(req).await,
        "bundle" => handle_bundle(req).await,
        "fmt" => handle_fmt(req, false).await,
        "fmt.check" => handle_fmt(req, true).await,
        "doc" => handle_doc(req).await,
        "test" => handle_test(req).await,
        "run" => handle_run(req).await,
        "profile" => handle_profile(req).await,
        "dev" => handle_dev(req).await,
        _ => {
            write_resp(
                &req.id,
                DispatchPayload::Error {
                    message: format!("unknown method: {}", req.method),
                    code: 404,
                },
            )
            .await?;
            write_resp(&req.id, DispatchPayload::Done { exit: 1 }).await?;
            Ok(())
        }
    }
}

async fn handle_build(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: BuildParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\", \"out_dir\": \"...\" }}")?;
    crate::commands::build::run(
        &p.file,
        &p.out_dir,
        None,
        None,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
        None,
        None,
    )
    .await
    .context("build failed")?;
    finish_ok(&req.id, Value::Null).await
}

async fn handle_check(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: CheckParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\" }}")?;

    let source = read_utf8_path_capped(&p.file)
        .with_context(|| format!("Failed to read source file: {}", p.file.display()))?;

    let file_path = p.file.to_string_lossy();
    let diagnostics = vox_compiler::pipeline::check_file(&source, &file_path);

    let mut error_count = 0;

    for diag in diagnostics {
        let is_error = diag.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error;
        if is_error {
            error_count += 1;
        }

        let sev = match diag.severity {
            vox_compiler::typeck::diagnostics::TypeckSeverity::Error => "error",
            vox_compiler::typeck::diagnostics::TypeckSeverity::Warning => "warning",
        };

        write_resp(
            &req.id,
            DispatchPayload::Diag {
                severity: sev.to_string(),
                message: diag.message,
                file: diag.file_path,
                line: diag.span.start_line as u32,
                col: diag.span.start_col as u32,
            },
        )
        .await?;
    }

    if error_count > 0 {
        return Err(anyhow::anyhow!(
            "Check failed with {} error(s)",
            error_count
        ));
    }

    finish_ok(&req.id, Value::Null).await
}

async fn handle_bundle(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: BundleParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\", \"out_dir\", \"target\"?, \"release\"? }}")?;
    crate::commands::bundle::run(
        &p.file,
        &p.out_dir,
        p.target.as_deref(),
        p.release,
        p.mode,
        vox_codegen::codegen_rust::RustAppShell::default(),
    )
    .await
    .context("bundle failed")?;
    finish_ok(&req.id, Value::Null).await
}

async fn handle_fmt(req: &DispatchRequest, check_only: bool) -> anyhow::Result<()> {
    let p: FmtParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\" }}")?;
    let before = read_utf8_path_capped(&p.file).ok();
    crate::commands::fmt::run(&p.file, check_only).context("fmt failed")?;
    let changed = before
        .as_ref()
        .map(|b| b.as_str() != read_utf8_path_capped(&p.file).unwrap_or_default().as_str())
        .unwrap_or(false);
    finish_ok(&req.id, json!({ "changed": changed })).await
}

async fn handle_doc(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: DocParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\", \"out_dir\": \"...\" }}")?;
    crate::commands::doc::run(&p.file, &p.out_dir)
        .await
        .context("doc failed")?;
    finish_ok(&req.id, Value::Null).await
}

async fn handle_test(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: TestParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\", ... }}")?;
    crate::commands::test::run(&crate::cli_args::TestArgs {
        file: p.file,
        filter: p.filter,
        forall_iterations: p.forall_iterations,
        coverage: p.coverage,
        update_snapshots: p.update_snapshots,
        watch: false,
    })
    .await
    .context("test failed")?;
    finish_ok(&req.id, Value::Null).await
}

async fn handle_run(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: RunParams = serde_json::from_value(req.params.clone()).context(
        "params must be {{ \"file\": \"...\", \"open\"?, \"port\"?, \"args\"?, \"mode\"? }}",
    )?;
    config::set_process_vox_port(p.port);
    if p.open {
        let url = format!("http://127.0.0.1:{}/", p.port);
        tokio::spawn(async move {
            tokio::time::sleep(vox_config::timeouts::D_2S).await;
            crate::fs_utils::open_browser(&url).await;
        });
    }
    let run_mode = crate::commands::run::parse_run_mode_from_str(&p.mode);
    crate::commands::run::run(&p.file, &p.args, run_mode)
        .await
        .context("run failed")?;
    finish_ok(&req.id, Value::Null).await
}

async fn handle_profile(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: ProfileParams = serde_json::from_value(req.params.clone())
        .context("params must be {{ \"file\": \"...\", \"json\"?, \"no_cache\"? }}")?;
    if p.no_cache {
        let _ = std::fs::remove_dir_all("dist");
        let _ = std::fs::remove_dir_all(PathBuf::from("target").join("generated"));
    }
    let out_dir = PathBuf::from("dist");
    let t0 = Instant::now();
    let args = crate::cli_args::CheckArgs {
        file: p.file.clone(),
        emit_ir: false,
        output_format: "text".to_string(),
        for_llm: false,
        strict: false,
        human_diagnostics: false,
    };
    crate::commands::check::run(&args)
        .await
        .context("check (profile) failed")?;
    let t_check = t0.elapsed();
    let t1 = Instant::now();
    crate::commands::build::run(
        &p.file,
        &out_dir,
        None,
        None,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
        None,
        None,
    )
    .await
    .context("build (profile) failed")?;
    let t_build = t1.elapsed();
    let total = t0.elapsed();

    let summary = json!({
        "check_ms": t_check.as_millis(),
        "build_ms": t_build.as_millis(),
        "total_ms": total.as_millis(),
    });

    if p.json {
        finish_ok(&req.id, summary).await?;
    } else {
        write_resp(
            &req.id,
            DispatchPayload::Log {
                level: "info".into(),
                msg: format!(
                    "profile: check={:?} build={:?} total={:?}",
                    t_check, t_build, total
                ),
            },
        )
        .await?;
        finish_ok(&req.id, summary).await?;
    }
    Ok(())
}

async fn handle_dev(req: &DispatchRequest) -> anyhow::Result<()> {
    let p: DevParams = serde_json::from_value(req.params.clone()).context(
        "params must be {{ \"file\": \"...\", \"out_dir\": \"...\", \"port\"?, \"open\"?, \"target\"? }}",
    )?;
    let file = PathBuf::from(p.file);
    let out_dir = PathBuf::from(p.out_dir);
    config::set_process_vox_port(p.port);
    let cli_build_target = p.target.map(Into::into);

    crate::commands::build::run(
        &file,
        &out_dir,
        None,
        cli_build_target,
        false,
        false,
        BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
        None,
        None,
    )
    .await
    .context("initial dev build failed")?;

    if p.open {
        let url = format!("http://127.0.0.1:{}/", p.port);
        tokio::spawn(async move {
            tokio::time::sleep(vox_config::timeouts::D_2S).await;
            crate::fs_utils::open_browser(&url).await;
        });
    }

    write_resp(
        &req.id,
        DispatchPayload::Log {
            level: "info".into(),
            msg: format!(
                "dev: watching {} (notify); Ctrl+C to stop [port {}]",
                file.display(),
                p.port
            ),
        },
    )
    .await?;

    let file_canon = std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
    let watch_dir = file
        .parent()
        .map(Path::to_path_buf)
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));

    let req_id = req.id.clone();
    watcher::each_modify_hit(file_canon, watch_dir, || {
        let file = file.clone();
        let out_dir = out_dir.clone();
        let req_id = req_id.clone();
        async move {
            if let Err(e) = crate::commands::build::run(
                &file,
                &out_dir,
                None,
                cli_build_target,
                false,
                false,
                BuildMode::App,
                vox_codegen::codegen_rust::RustAppShell::default(),
                None,
                None,
            )
            .await
            {
                if write_resp(
                    &req_id,
                    DispatchPayload::Log {
                        level: "warn".into(),
                        msg: format!("rebuild failed: {e}"),
                    },
                )
                .await
                .is_err()
                {
                    eprintln!("vox-compilerd: failed to emit rebuild error log");
                }
            }
        }
    })
    .await?;

    Ok(())
}

#[cfg(test)]
mod dev_params_tests {
    use super::DevParams;
    use serde_json::json;

    #[test]
    fn dev_params_deserializes_lowercase_target() {
        let p: DevParams = serde_json::from_value(json!({
            "file": "x.vox",
            "out_dir": "dist",
            "target": "server",
        }))
        .expect("deserialize DevParams");
        assert_eq!(p.target, Some(crate::cli_args::BuildTargetArg::Server));
    }
}

#[cfg(test)]
mod run_params_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn run_params_default_mode_is_auto() {
        let p: RunParams = serde_json::from_value(json!({ "file": "x.vox" })).expect("deserialize");
        assert_eq!(p.mode, "auto");
        assert!(p.args.is_empty());
    }

    #[test]
    fn run_params_accepts_mode_and_args() {
        let p: RunParams = serde_json::from_value(json!({
            "file": "s.vox",
            "mode": "script",
            "args": ["a", "b"],
            "port": 3000
        }))
        .expect("deserialize");
        assert_eq!(p.mode, "script");
        assert_eq!(p.args, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(p.port, 3000);
    }

    #[test]
    fn run_mode_parsing_matches_cli() {
        use crate::commands::run::{RunMode, parse_run_mode_from_str};
        assert_eq!(
            parse_run_mode_from_str(&default_run_mode_str()),
            RunMode::Auto
        );
        assert_eq!(parse_run_mode_from_str("script"), RunMode::Script);
        assert_eq!(parse_run_mode_from_str("APP"), RunMode::App);
    }
}
