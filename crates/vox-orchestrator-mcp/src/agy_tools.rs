//! MCP tools for delegating to Antigravity `agy` (doctor + single + batch).

use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_ledger::{append_entry_locked, LedgerEntry};
use crate::agy_worktree::DelegationWorktree;
use crate::params::ToolResult;
use crate::server_state::ServerState;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

static DELEGATION_SEQ: AtomicU64 = AtomicU64::new(1);

const MAX_CONCURRENCY: usize = 8;

pub fn doctor_report_json() -> serde_json::Value {
    let status = detect();
    let (label, path) = match &status {
        AgyStatus::Missing => ("missing", None),
        AgyStatus::PresentUnauthed { path } => ("present_unauthed", Some(path.clone())),
        AgyStatus::Ready { path, .. } => ("ready", Some(path.clone())),
    };
    serde_json::json!({
        "status": label,
        "path": path,
        "remediation": remediation(&status),
    })
}

/// `vox_agy_doctor`
pub async fn vox_agy_doctor(_state: &ServerState, _args: serde_json::Value) -> String {
    ToolResult::ok(doctor_report_json()).to_json()
}

pub fn delegate_validate(args: &serde_json::Value) -> Result<(String, Option<String>, u64), String> {
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return Err("Missing non-empty 'task'.".into());
    }
    let model = args.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((task, model, timeout_secs))
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Unique, collision-free slug independent of the ledger id (which is allocated
/// later under lock). Monotonic counter keeps parallel workers disjoint.
fn fresh_slug(hint: &str) -> String {
    let n = DELEGATION_SEQ.fetch_add(1, Ordering::Relaxed);
    crate::agy_exec::sanitize_slug(&format!("d{n}-{hint}"))
}

fn tail(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n { return s.to_string(); }
    s.chars().skip(len - n).collect()
}

const REM_TASK: &str = "Provide a non-empty 'task' string with an exact, zero-ambiguity spec (file paths, target symbols).";

/// `vox_agy_delegate`
pub async fn vox_agy_delegate(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs) = match delegate_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };
    // Doctor gate: fail fast with actionable remediation, not an opaque spawn error.
    let report = doctor_report_json();
    if report["status"] != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {}).", report["status"]),
            report["remediation"].as_str().unwrap_or("Run vox_agy_doctor.").to_string(),
        ).to_json();
    }

    let repo_root = state.repository.root.clone();
    let slug = fresh_slug(&task);
    let wt = match DelegationWorktree::create(&repo_root, &slug).await {
        Ok(w) => w,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("could not create delegation worktree: {e}"),
            "Ensure the repo is a git work tree with a committed HEAD.",
        ).to_json(),
    };

    // Retry loop (quota/timeout-aware).
    let exec = AgyExec::new(&wt.path);
    let mut attempt = 0u32;
    let max_attempts = 3u32;
    let out = loop {
        let spec = AgySpec { task: task.clone(), model: model.clone(), timeout_secs };
        let o = exec.run(&spec).await;
        let (stderr, exit, timed) = match &o {
            Ok(x) => (x.stderr.clone(), x.exit_code, x.timed_out),
            Err(e) => (e.to_string(), -1, false),
        };
        match crate::agy_exec::classify_failure(&stderr, exit, timed) {
            Some(class) if crate::agy_exec::should_retry(class, attempt, max_attempts) => {
                tokio::time::sleep(std::time::Duration::from_secs(1u64 << attempt)).await;
                attempt += 1;
                continue;
            }
            _ => break o,
        }
    };

    let (outcome, exit_code, timed_out, stderr) = match &out {
        Ok(o) => (
            if o.timed_out { "failed" } else if o.exit_code == 0 { "partial" } else { "failed" },
            o.exit_code, o.timed_out, o.stderr.clone()
        ),
        Err(e) => ("failed", -1, false, e.to_string()),
    };
    let (diff, files_changed) = wt.capture().await.unwrap_or_else(|_| (String::new(), 0));

    let id = append_entry_locked(&repo_root, LedgerEntry::new(
        "agy-delegation", &task, outcome, timed_out, exit_code, files_changed, timeout_secs, &today(),
    )).await.unwrap_or_else(|_| "AGH-unwritten".into());

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": wt.path.to_string_lossy(),
        "branch": wt.branch,
        "outcome": outcome,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "attempts": attempt + 1,
        "files_changed": files_changed,
        "diff": diff,
        "stderr_tail": tail(&stderr, 2000),
        "billing": "antigravity-credits",
        "billing_note": "Antigravity credits (not USD); balance not queryable — see the credits SSOT doc.",
        "next_step": "Review the diff. If good: integrate `agy/<slug>` (merge/cherry-pick), then set the ledger verdict. If not: re-delegate with corrections.",
    })).to_json()
}

pub fn batch_validate(args: &serde_json::Value) -> Result<(Vec<String>, usize, u64), String> {
    let tasks: Vec<String> = args.get("tasks").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if tasks.is_empty() {
        return Err("Provide a non-empty 'tasks' array of file-disjoint spec strings.".into());
    }
    let conc = args.get("max_concurrency").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((tasks, conc.clamp(1, MAX_CONCURRENCY), timeout_secs))
}

/// `vox_agy_delegate_batch`
pub async fn vox_agy_delegate_batch(state: &ServerState, args: serde_json::Value) -> String {
    let (tasks, conc, timeout_secs) = match batch_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            e, "Each task must be a self-contained, file-disjoint spec (see dispatching-parallel-agents).",
        ).to_json(),
    };
    // One doctor check up front (fail the whole batch fast if agy isn't ready).
    let report = doctor_report_json();
    if report["status"] != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {}).", report["status"]),
            report["remediation"].as_str().unwrap_or("Run vox_agy_doctor.").to_string(),
        ).to_json();
    }

    let sem = Arc::new(Semaphore::new(conc));
    let mut handles = Vec::new();
    for task in tasks {
        let sem = sem.clone();
        let st = state.clone();
        let one = serde_json::json!({ "task": task, "timeout_secs": timeout_secs });
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore open");
            vox_agy_delegate(&st, one).await
        }));
    }
    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":\"worker join failed: {e}\"}}")));
    }
    ToolResult::ok(serde_json::json!({
        "workers": results.len(),
        "concurrency": conc,
        "results": results.iter().map(|r| serde_json::from_str::<serde_json::Value>(r).unwrap_or(serde_json::json!({"raw": r}))).collect::<Vec<_>>(),
        "next_step": "Review each worker's diff + ledger entry. Merge file-disjoint branches; resolve overlap sequentially. Two-strike rule (dispatching-parallel-agents) on repeated failures.",
    })).to_json()
}

pub fn credentials_status_json() -> serde_json::Value {
    let secret_rows: Vec<serde_json::Value> = vox_secrets::list_secret_status()
        .into_iter()
        .map(|row| serde_json::json!({
            "id": row.id,
            "env": row.canonical_env,
            "present": row.is_present,
            "required": row.required,
        }))
        .collect();
    let inference: Vec<String> = vox_orchestrator::models::key_guard::available_inference_providers()
        .into_iter()
        .map(|p| format!("{p:?}"))
        .collect();
    serde_json::json!({
        "inference_providers": inference,
        "secrets": secret_rows,
        "delegation": { "agy": doctor_report_json() },
        "note": "agy is billed in Antigravity credits (not USD) and has no queryable balance; see docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md",
    })
}

/// `vox_credentials_status`
pub async fn vox_credentials_status(_state: &ServerState, _args: serde_json::Value) -> String {
    ToolResult::ok(credentials_status_json()).to_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_json_has_status_and_remediation() {
        let v = doctor_report_json();
        assert!(v.get("status").is_some());
        assert!(v.get("remediation").is_some());
    }

    #[test]
    fn delegate_validate_requires_task_and_defaults_timeout() {
        assert!(delegate_validate(&serde_json::json!({})).is_err());
        let (task, _m, t) = delegate_validate(&serde_json::json!({"task":"do X"})).unwrap();
        assert_eq!(task, "do X");
        assert_eq!(t, 900);
    }

    #[test]
    fn batch_validate_requires_tasks_and_clamps_concurrency() {
        assert!(batch_validate(&serde_json::json!({"tasks": []})).is_err());
        let (tasks, conc, _t) = batch_validate(&serde_json::json!({"tasks":["a","b","c"],"max_concurrency":99})).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(conc <= 8 && conc >= 1);
    }

    #[test]
    fn credentials_status_has_inference_and_delegation_sections() {
        let v = credentials_status_json();
        assert!(v.get("inference_providers").is_some());
        assert!(v.get("delegation").and_then(|d| d.get("agy")).is_some());
    }
}
