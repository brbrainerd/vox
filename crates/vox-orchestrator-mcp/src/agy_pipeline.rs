//! Stage-2/3 deterministic harness: the pure outcome classifier plus the
//! vox_agy_pipeline / vox_agy_review / vox_agy_ledger_digest tools.

use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_gates::{run_gates, Gate, GateResult};
use crate::agy_ledger::{append_entry_locked, LedgerEntry};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use std::sync::atomic::{AtomicU64, Ordering};

/// green   = files changed AND every specified gate passed.
/// partial = files changed but a gate failed, OR no gates specified (unverified).
/// failed  = timed out or no files changed.
///
/// agy's own exit code is intentionally NOT used — it's an agent wrapper whose
/// exit code doesn't reliably reflect correctness; the EFFECT is the signal (B-9).
pub fn classify_outcome(files_changed: usize, gates: &[GateResult], timed_out: bool) -> &'static str {
    if timed_out || files_changed == 0 {
        return "failed";
    }
    if gates.is_empty() {
        return "partial";
    }
    if gates.iter().all(|g| g.passed) {
        "green"
    } else {
        "partial"
    }
}

static PIPELINE_SEQ: AtomicU64 = AtomicU64::new(1);

const REM_TASK: &str =
    "Provide a non-empty 'task' with an exact, zero-ambiguity spec, and 'gates' \
     scoped to the touched crate (e.g. cargo build -p <crate>, with env CARGO_TARGET_DIR \
     set to the main target) so the result is verified.";

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn fresh_slug(hint: &str) -> String {
    let n = PIPELINE_SEQ.fetch_add(1, Ordering::Relaxed);
    crate::agy_exec::sanitize_slug(&format!("p{n}-{hint}"))
}

fn doctor_status_label() -> (&'static str, String) {
    match detect() {
        AgyStatus::Missing => ("missing", remediation(&AgyStatus::Missing)),
        s @ AgyStatus::PresentUnauthed { .. } => ("present_unauthed", remediation(&s)),
        s @ AgyStatus::Ready { .. } => ("ready", remediation(&s)),
    }
}

/// (task, model, timeout_secs, gates). Gates may be empty (⇒ unverified/partial).
pub fn pipeline_validate(
    args: &serde_json::Value,
) -> Result<(String, Option<String>, u64, Vec<Gate>), String> {
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return Err("Missing non-empty 'task'.".into());
    }
    let model = args.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    let gates: Vec<Gate> = match args.get("gates") {
        Some(g) => serde_json::from_value(g.clone())
            .map_err(|e| format!("'gates' must be [{{name, program, args, env}}]: {e}"))?,
        None => Vec::new(),
    };
    Ok((task, model, timeout_secs, gates))
}

/// `vox_agy_pipeline` — Stage 2.
pub async fn vox_agy_pipeline(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs, gates) = match pipeline_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };

    let (label, rem) = doctor_status_label();
    if label != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {label})."),
            rem,
        )
        .to_json();
    }

    let repo_root = state.repository.root.clone();
    let slug = fresh_slug(&task);
    let wt = match crate::agy_worktree::DelegationWorktree::create(&repo_root, &slug).await {
        Ok(w) => w,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("could not create delegation worktree: {e}"),
                "Ensure the repo is a git work tree with a committed HEAD.",
            )
            .to_json()
        }
    };

    // Delegate with quota/timeout retry (same policy as vox_agy_delegate).
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
    let (exit_code, timed_out, elapsed_ms) = match &out {
        Ok(o) => (o.exit_code, o.timed_out, o.elapsed_ms),
        Err(_) => (-1, false, 0),
    };

    // Capture the EFFECT.
    let (diff, files_changed) = wt.capture().await.unwrap_or_else(|_| (String::new(), 0));
    let gate_results: Vec<GateResult> = run_gates(&wt.path, &gates, timeout_secs).await;
    let outcome = classify_outcome(files_changed, &gate_results, timed_out);

    let gate_summary = if gate_results.is_empty() {
        "unverified (no gates specified)".to_string()
    } else {
        gate_results
            .iter()
            .map(|g| format!("{}: {}", g.name, if g.passed { "pass" } else { "fail" }))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // PROVISIONAL ledger entry (verdict pending; render already sets request-changes
    // + "pending human review"). Recorded even on failure for the flywheel.
    let id = append_entry_locked(
        &repo_root,
        LedgerEntry::new(
            "agy-pipeline", &task, outcome, timed_out, exit_code, files_changed, timeout_secs, &today(),
        )
        .with_verification(gate_summary.clone()),
    )
    .await
    .unwrap_or_else(|_| "AGH-unwritten".into());

    // Cleanup the jail only when nothing was produced (no dead worktrees pile up).
    if files_changed == 0 {
        let _ = wt.cleanup(&repo_root).await;
    }

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": if files_changed == 0 { String::new() } else { wt.path.to_string_lossy().to_string() },
        "branch": wt.branch,
        "outcome": outcome,
        "files_changed": files_changed,
        "gates": gate_results,
        "verification": gate_summary,
        "spend_proxy": {
            "elapsed_ms": elapsed_ms,
            "attempts": attempt + 1,
            "timed_out": timed_out,
            "exit_code": exit_code,
            "billing": "antigravity-credits",
            "note": "Credits are not queryable headlessly; this is a proxy, not a balance."
        },
        "diff": diff,
        "next_step": match outcome {
            "green" => "Run the Stage-3 adversarial review (code-reviewer agent vs the jailed diff), record it with vox_agy_review, then take the jailed branch to the human merge gate.",
            "partial" => "A gate failed or no gate ran. Review the gate output_tail; distill a correction and re-delegate ONCE (two-strike), or add scoped gates.",
            _ => "No changes / timeout (jail cleaned). Re-author a smaller atomic launch statement and re-delegate ONCE.",
        },
    }))
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agy_gates::GateResult;

    fn gate(passed: bool) -> GateResult {
        GateResult { name: "g".into(), passed, exit_code: if passed { 0 } else { 1 }, output_tail: String::new(), elapsed_ms: 0 }
    }

    #[test]
    fn timeout_is_failed() {
        assert_eq!(classify_outcome(5, &[gate(true)], true), "failed");
    }
    #[test]
    fn no_changes_is_failed() {
        assert_eq!(classify_outcome(0, &[gate(true)], false), "failed");
    }
    #[test]
    fn changes_with_no_gates_is_partial_not_green() {
        assert_eq!(classify_outcome(3, &[], false), "partial");
    }
    #[test]
    fn changes_with_all_gates_passing_is_green() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(true)], false), "green");
    }
    #[test]
    fn changes_with_a_failing_gate_is_partial() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(false)], false), "partial");
    }

    #[test]
    fn pipeline_validate_requires_task_and_parses_gates() {
        assert!(pipeline_validate(&serde_json::json!({})).is_err());
        let (task, model, t, gates) = pipeline_validate(&serde_json::json!({
            "task": "do X",
            "gates": [{"name": "build", "program": "cargo", "args": ["build", "-p", "foo"]}]
        })).unwrap();
        assert_eq!(task, "do X");
        assert!(model.is_none());
        assert_eq!(t, 900);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].program, "cargo");
    }
}
