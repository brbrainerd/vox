//! End-to-end integration: `init_telemetry_sinks` registers the CR-L8
//! corpus-feedback JSONL sink (P2.1c), so `record_event!` calls from anywhere
//! in the workspace persist `LintFinding` / `LintAutofix` / `RepairAttempt` /
//! `RepairOutcome` events to disk where the `vox audit corpus-feedback`
//! aggregator can read them.
//!
//! Lives in `tests/` so the `set_global_recorder` OnceLock starts fresh.

use std::path::PathBuf;

use vox_telemetry::{
    LintAutofixEvent, LintFindingEvent, ModelCallEvent, RepairOutcomeEvent, TelemetryEvent,
    record_event,
};

const EVENTS_DIR_ENV: &str = "VOX_CORPUS_FEEDBACK_EVENTS_DIR";

/// A13: sink rotates by quarter (`YYYY-QN.jsonl`), not by day.
fn current_quarter_jsonl(root: &std::path::Path) -> PathBuf {
    let now = chrono::Utc::now();
    let year = now.format("%Y");
    let month_one_based: u32 = now.format("%m").to_string().parse().unwrap_or(1);
    let quarter = (month_one_based.saturating_sub(1)) / 3 + 1;
    root.join(format!("{year}-Q{quarter}.jsonl"))
}

fn lint(rule: &str) -> TelemetryEvent {
    TelemetryEvent::LintFinding(LintFindingEvent {
        rule_id: rule.into(),
        diagnostic_id: Some(format!("vox/{rule}")),
        severity: "warning".into(),
        relative_path: "test.vox".into(),
        line: 1,
        autofix_available: true,
        confidence: Some("high".into()),
        repository_id: Some("vox-e2e".into()),
    })
}

fn autofix(rule: &str, outcome: &str) -> TelemetryEvent {
    TelemetryEvent::LintAutofix(LintAutofixEvent {
        rule_id: rule.into(),
        diagnostic_id: Some(format!("vox/{rule}")),
        outcome: outcome.into(),
        reason: None,
        relative_path: "test.vox".into(),
        line: 1,
        repository_id: Some("vox-e2e".into()),
    })
}

fn repair_outcome(state: &str) -> TelemetryEvent {
    TelemetryEvent::RepairOutcome(RepairOutcomeEvent {
        final_state: state.into(),
        attempts_used: 2,
        attempts_budget: 3,
        total_cost_usd: 0.0,
        total_duration_ms: 1500,
        residual_diagnostics: 0,
        note: None,
        repository_id: Some("vox-e2e".into()),
    })
}

fn model_call() -> TelemetryEvent {
    TelemetryEvent::ModelCall(ModelCallEvent {
        model: "x".into(),
        provider: "x".into(),
        route_profile: None,
        prompt_tokens: 0,
        completion_tokens: 0,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        latency_ms: 0,
        cost_usd: 0.0,
        cost_source: "x".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: None,
        parent_task_id: None,
        trace_id: None,
        caller_agent_id: None,
    })
}

// `#[tokio::test]` because `SpoolSink` (one of the sinks `init_telemetry_sinks`
// registers) uses `tokio::spawn` for async file writes. Without a runtime the
// composite recorder fan-out panics on the first event.
#[tokio::test(flavor = "current_thread")]
async fn init_telemetry_sinks_persists_cr_l8_events_to_jsonl_on_disk() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let events_root = tmp.path().to_path_buf();

    // Steer the corpus-feedback sink at our tempdir.
    // SAFETY: Integration test binary has its own process; env-var
    // mutation is single-threaded within this test.
    unsafe { std::env::set_var(EVENTS_DIR_ENV, &events_root) };

    // Register the full telemetry stack (DB=None: only Spool + CR-L8 sinks).
    vox_cli::init_telemetry_sinks(None);

    // Emit a representative mix: 2 lint findings, 1 autofix-applied,
    // 1 autofix-rejected, 1 repair outcome, plus an unrelated ModelCall event
    // that the CR-L8 sink MUST filter out.
    record_event!(&lint("retired/decorator-usage"));
    record_event!(&lint("retired/decorator-usage"));
    record_event!(&autofix("retired/decorator-usage", "applied"));
    record_event!(&autofix("retired/decorator-usage", "rejected"));
    record_event!(&repair_outcome("success"));
    record_event!(&model_call());

    // The sink writes synchronously inside `record`, so by the time
    // `record_event!` returns the file should be visible.
    let jsonl = current_quarter_jsonl(&events_root);
    assert!(
        jsonl.exists(),
        "expected JSONL at {} after recording events",
        jsonl.display()
    );

    let contents = std::fs::read_to_string(&jsonl).expect("read jsonl");
    let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        5,
        "expected 5 CR-L8 events (2 lint + 2 autofix + 1 repair-outcome), got {}: \
         {contents}",
        lines.len()
    );

    // Round-trip every line as TelemetryEvent and confirm variants are correct.
    let parsed: Vec<TelemetryEvent> = lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("parse JSONL line"))
        .collect();
    let lint_count = parsed
        .iter()
        .filter(|e| matches!(e, TelemetryEvent::LintFinding(_)))
        .count();
    let autofix_count = parsed
        .iter()
        .filter(|e| matches!(e, TelemetryEvent::LintAutofix(_)))
        .count();
    let outcome_count = parsed
        .iter()
        .filter(|e| matches!(e, TelemetryEvent::RepairOutcome(_)))
        .count();
    assert_eq!(lint_count, 2);
    assert_eq!(autofix_count, 2);
    assert_eq!(outcome_count, 1);

    // The model-call event must NOT be in the file (CR-L8 filter).
    assert!(
        !contents.contains("\"model_call\""),
        "model_call events must not reach the CR-L8 sink"
    );

    // Cleanup env-var.
    // SAFETY: see top of test.
    unsafe { std::env::remove_var(EVENTS_DIR_ENV) };
}
