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

/// Sidecar command paths this panel invokes.
///
/// These exist as constants, and are validated against the CLI's own command
/// catalog by `sidecar_command_paths_exist_in_the_cli_catalog` below, because
/// the GUI reaches the CLI through a subprocess: a path that stops existing is
/// a RUNTIME failure with no compile-time signal. The generic action-manifest
/// path (`execute.rs`) is already safe — it receives catalog-derived paths — so
/// hand-written panels like this one are the only place that can drift.
///
/// Build every sidecar argv from these, never from inline string literals; a
/// literal is invisible to the test and reintroduces the silent break.
pub(crate) const SEMANTIC_SUBMIT_PATH: [&str; 3] = ["review", "coderabbit", "semantic-submit"];
pub(crate) const DB_STATUS_PATH: [&str; 3] = ["review", "coderabbit", "db-status"];

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
/// `full_repo` sweeps every tracked file (`--full-repo`); `--since` takes precedence
/// in the CLI, so the two are mutually exclusive here.
fn submit_args(
    repo: &str,
    since: &str,
    cap: u32,
    rank_weights: &str,
    top: Option<u32>,
    full_repo: bool,
    execute: bool,
) -> Vec<String> {
    let mut a: Vec<String> = SEMANTIC_SUBMIT_PATH
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    a.extend([
        repo.into(),
        "--max-files-per-pr".into(),
        cap.to_string(),
        "--rank-weights".into(),
        rank_weights.into(),
    ]);
    if full_repo {
        a.push("--full-repo".into());
    } else {
        a.push("--since".into());
        a.push(since.into());
    }
    if let Some(n) = top {
        a.push("--top".into());
        a.push(n.to_string());
    }
    if execute {
        a.push("--execute".into());
    }
    a
}

/// Scope guard shared by plan and run: full-repo needs no date; date mode needs one.
fn validate_scope(since: &str, full_repo: bool) -> Result<(), String> {
    if !full_repo && since.trim().is_empty() {
        return Err("Pick a 'modified since' date or enable full-repo.".into());
    }
    Ok(())
}

/// Preview: plan-only (no `--execute`); returns the written slice manifest.
#[tauri::command]
pub async fn coderabbit_plan(
    app: AppHandle,
    since: String,
    cap: u32,
    rank_weights: String,
    top: Option<u32>,
    full_repo: Option<bool>,
) -> Result<Value, String> {
    let full_repo = full_repo.unwrap_or(false);
    validate_scope(&since, full_repo)?;
    let root = repo_root();
    run_vox(
        &app,
        submit_args(
            &root.to_string_lossy(),
            &since,
            cap,
            &rank_weights,
            top,
            full_repo,
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
    full_repo: Option<bool>,
) -> Result<Value, String> {
    let full_repo = full_repo.unwrap_or(false);
    validate_scope(&since, full_repo)?;
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
                full_repo,
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
        DB_STATUS_PATH
            .iter()
            .map(|s| (*s).to_string())
            .chain(std::iter::once("--json".to_string()))
            .collect(),
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

#[cfg(test)]
mod sidecar_path_tests {
    use super::*;
    use serde_yaml::Value;

    /// The GUI reaches the CLI through a subprocess, so a removed subcommand does
    /// not fail to compile — the panel breaks at runtime, silently, for whoever
    /// clicks it.
    ///
    /// This validates against the OPERATIONS CATALOG rather than
    /// `vox_cli::command_catalog::build_catalog()`, deliberately. `coderabbit` is
    /// not a default feature of `vox-cli`, and `vox-gui` links it with default
    /// features, so the compiled clap catalog here would never contain these
    /// paths and the assertion would fail for a reason that has nothing to do
    /// with the command actually existing. The YAML catalog is feature-
    /// independent and carries `feature_gate: coderabbit` as data.
    ///
    /// SCOPE, stated honestly: this asserts the ROOT command still exists. That
    /// is the failure mode that matters here — retiring CodeRabbit deletes the
    /// whole `vox review` surface — but it does NOT catch a rename of a leaf
    /// subcommand like `semantic-submit`. Catching that needs the feature
    /// enabled in a test build; see the residual-gap note in the retirement plan.
    #[test]
    fn sidecar_root_commands_still_exist_in_the_operations_catalog() {
        let root = vox_repository::resolve_repo_root_for_ci();
        let raw = std::fs::read_to_string(root.join("contracts/operations/catalog.v1.yaml"))
            .expect("read operations catalog");
        let parsed: Value = serde_yaml::from_str(&raw).expect("parse operations catalog");
        let ops = parsed
            .get("operations")
            .and_then(Value::as_sequence)
            .expect("catalog has an operations sequence");

        let cli_roots: Vec<String> = ops
            .iter()
            .filter_map(|op| {
                op.get("cli")?
                    .get("path")?
                    .as_sequence()?
                    .first()?
                    .as_str()
                    .map(str::to_owned)
            })
            .collect();

        for path in [SEMANTIC_SUBMIT_PATH.as_slice(), DB_STATUS_PATH.as_slice()] {
            let wanted = path[0];
            assert!(
                cli_roots.iter().any(|r| r == wanted),
                "the GUI shells out to `vox {}`, but no operation in                  contracts/operations/catalog.v1.yaml declares a cli.path starting with                  {wanted:?}. The panel would fail at runtime with no compile error. If this                  command was retired on purpose, remove the panel and these constants too.",
                path.join(" ")
            );
        }
    }
}
