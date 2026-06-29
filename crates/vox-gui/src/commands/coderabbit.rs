//! CodeRabbit review panel — async Tauri commands that shell out to the `vox` sidecar.
//!
//! No review logic lives here: the panel previews the planned slice manifest, triggers
//! an execute run in the background (emitting progress), reports run-state + DB findings,
//! and shows read-only token presence. All heavy lifting is `vox review coderabbit …`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::ShellExt;

const SIDECAR: &str = "vox";

/// Event channel the panel subscribes to for execute-run progress/completion.
pub const PROGRESS_EVENT: &str = "coderabbit://progress";

/// Guards against concurrent `--execute` sweeps (each opens real PRs and mutates
/// `.coderabbit/run-state.json`). A single in-process flag; the CLI side should add an
/// on-disk lock to also exclude a second process.
static SWEEP_RUNNING: AtomicBool = AtomicBool::new(false);

/// Resolve the repository root by walking up from the process CWD for a `.git` entry,
/// so file reads and the sidecar's writes agree even when the GUI runs from a subdir.
fn repo_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut d = cwd.as_path();
    loop {
        if d.join(".git").exists() {
            return d.to_path_buf();
        }
        match d.parent() {
            Some(p) => d = p,
            None => return cwd,
        }
    }
}

/// Run the `vox` sidecar with `args`, returning stdout (or stderr on failure).
async fn run_vox(app: &AppHandle, args: Vec<String>) -> Result<String, String> {
    let out = app
        .shell()
        .sidecar(SIDECAR)
        .map_err(|e| e.to_string())?
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Build the `semantic-submit` arg vector shared by plan (preview) and run (execute).
fn submit_args(
    repo: &str,
    since: &str,
    cap: u32,
    rank_weights: &str,
    top: Option<u32>,
    execute: bool,
) -> Vec<String> {
    let mut a = vec![
        "review".into(),
        "coderabbit".into(),
        "semantic-submit".into(),
        repo.into(),
        "--since".into(),
        since.into(),
        "--max-files-per-pr".into(),
        cap.to_string(),
        "--rank-weights".into(),
        rank_weights.into(),
    ];
    if let Some(n) = top {
        a.push("--top".into());
        a.push(n.to_string());
    }
    if execute {
        a.push("--execute".into());
    }
    a
}

/// Preview: plan-only (no `--execute`); returns the written slice manifest.
#[tauri::command]
pub async fn coderabbit_plan(
    app: AppHandle,
    since: String,
    cap: u32,
    rank_weights: String,
) -> Result<Value, String> {
    if since.trim().is_empty() {
        return Err("Pick a 'modified since' date first.".into());
    }
    let root = repo_root();
    run_vox(
        &app,
        submit_args(
            &root.to_string_lossy(),
            &since,
            cap,
            &rank_weights,
            None,
            false,
        ),
    )
    .await?;
    let m = std::fs::read_to_string(root.join(".coderabbit/semantic-manifest.json"))
        .map_err(|e| format!("read manifest: {e}"))?;
    serde_json::from_str(&m).map_err(|e| e.to_string())
}

/// Execute: rate-limited multi-hour run. Returns immediately; emits `coderabbit://progress`.
#[tauri::command]
pub async fn coderabbit_run_async(
    app: AppHandle,
    since: String,
    cap: u32,
    rank_weights: String,
    top: Option<u32>,
) -> Result<Value, String> {
    if since.trim().is_empty() {
        return Err("Pick a 'modified since' date first.".into());
    }
    // Reject a second concurrent sweep: two `--execute` runs would open duplicate PRs
    // and interleave writes to `.coderabbit/run-state.json`.
    if SWEEP_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A CodeRabbit sweep is already running.".into());
    }
    tokio::spawn(async move {
        let root = repo_root();
        let res = run_vox(
            &app,
            submit_args(
                &root.to_string_lossy(),
                &since,
                cap,
                &rank_weights,
                top,
                true,
            ),
        )
        .await;
        SWEEP_RUNNING.store(false, Ordering::SeqCst);
        let payload = match &res {
            Ok(_) => serde_json::json!({ "status": "done" }),
            Err(e) => serde_json::json!({ "status": "error", "error": e }),
        };
        let _ = app.emit(PROGRESS_EVENT, payload);
    });
    Ok(serde_json::json!({ "status": "running" }))
}

/// Findings + slice status: run-state (per-PR statuses) + DB summary (findings totals).
#[tauri::command]
pub async fn coderabbit_report(app: AppHandle) -> Result<Value, String> {
    let run_state = std::fs::read_to_string(repo_root().join(".coderabbit/run-state.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null);
    // Distinguish "db-status failed" from "0 findings" so the panel can show the
    // difference instead of silently rendering an error as an empty report.
    let (db_status, db_error) = match run_vox(
        &app,
        vec![
            "review".into(),
            "coderabbit".into(),
            "db-status".into(),
            "--json".into(),
        ],
    )
    .await
    {
        Ok(s) => match serde_json::from_str::<Value>(&s) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, Value::String(format!("parse db-status: {e}"))),
        },
        Err(e) => (Value::Null, Value::String(e)),
    };
    Ok(serde_json::json!({ "run_state": run_state, "db_status": db_status, "db_error": db_error }))
}

/// Read-only token presence via the secrets layer (never a direct env read).
/// `ForgeToken` resolution already covers both `FORGE_TOKEN` and `GITHUB_TOKEN`.
#[tauri::command]
pub fn coderabbit_token_present() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::ForgeToken)
        .expose()
        .is_some()
}
