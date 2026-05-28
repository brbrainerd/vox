//! CR-L8 corpus-feedback loop — end-to-end proof.
//!
//! Drives a real Vox fixture with three retired patterns through
//! [`vox_code_audit::ToestubEngine`], captures the emitted `LintFinding`
//! events via a `BufferedRecorder`, runs them through the CR-L8 aggregator,
//! and asserts the resulting `CorpusFeedbackReport` carries the expected
//! per-rule rollup with the correct counts.
//!
//! This is the single test that proves the *whole loop* lands a real
//! measurement: emitter → event → aggregator → report. Lives in `tests/` so
//! the `set_global_recorder` `OnceLock` starts fresh for this binary, freeing
//! us from interference with `vox-code-audit`'s own integration tests.
//!
//! Single `#[test]` rather than multiple — `set_global_recorder` is
//! first-writer-wins via `OnceLock`, so we register exactly once per binary
//! and exercise every assertion sequentially.
//!
//! Council-ratified 2026-05-15 (A3 in the v1-llm-target-implementation-plan
//! Tier-A batch closing the P2.1c work).

use std::path::PathBuf;
use std::sync::Arc;

use vox_audit::aggregator::aggregate;
use vox_audit::recorder::BufferedRecorder;
use vox_code_audit::engine::{ToestubConfig, ToestubEngine, ToestubRunMode};
use vox_code_audit::report::OutputFormat;
use vox_code_audit::rules::{Language, Severity};
use vox_telemetry::{TelemetryEvent, set_global_recorder};

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn config_for(roots: Vec<PathBuf>) -> ToestubConfig {
    ToestubConfig {
        roots,
        min_severity: Severity::Info,
        format: OutputFormat::Json,
        languages: Some(vec![Language::Vox]),
        rule_filter: Some(vec!["retired/decorator-usage".to_string()]),
        run_mode: ToestubRunMode::Audit,
        repository_id: Some("cr-l8-e2e".to_string()),
        ..ToestubConfig::default()
    }
}

#[test]
fn cr_l8_full_loop_emit_capture_aggregate_report() {
    // 1. Register the capturing recorder ONCE for this test binary.
    let recorder = Arc::new(BufferedRecorder::new());
    set_global_recorder(recorder.clone());

    // 2. Fixture with three retired patterns → expect 3 findings, 3 events.
    let tmp = tempfile::tempdir().expect("tempdir");
    // Use the RETIRED forms (not the canonical ones):
    //   @component fn  →  retired (use bare `component` keyword instead)
    //   @endpoint(kind: server)   →  retired (use `@server` instead)
    //   @endpoint(kind: mutation) →  retired (use `@mutation` instead)
    // The bare `@server` / `@mutation` forms ARE the canonical post-2026-05-24 surface.
    write_fixture(
        &tmp,
        "loop_e2e.vox",
        "@component fn Dashboard() {}\n\
         @endpoint(kind: server) fn list_items() {}\n\
         @endpoint(kind: mutation) fn add_item() {}\n",
    );

    // 3. Run the engine. P2.1b's `emit_lint_finding_telemetry` emits one
    //    `TelemetryEvent::LintFinding` per finding through `record_event!`,
    //    which the registered global recorder forwards into our buffer.
    let engine = ToestubEngine::new(config_for(vec![tmp.path().to_path_buf()]));
    let result = engine.run();
    assert!(
        result.findings.len() >= 3,
        "expected at least 3 findings; got {}",
        result.findings.len()
    );

    // 4. Snapshot captured events.
    let captured = recorder.drain();
    let lint_findings: Vec<_> = captured
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::LintFinding(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(
        lint_findings.len(),
        result.findings.len(),
        "one LintFinding event per emitted finding ({} findings, {} events)",
        result.findings.len(),
        lint_findings.len()
    );

    // 5. A1 verification: repository_id threaded through every event.
    for ev in &lint_findings {
        assert_eq!(
            ev.repository_id.as_deref(),
            Some("cr-l8-e2e"),
            "ToestubConfig.repository_id must thread into LintFindingEvent"
        );
    }

    // 6. Aggregate captured events into a substantive CR-L8 report.
    let report = aggregate(&captured, "2026-05-15T00:00:00Z");

    assert_eq!(report.schema_version, 1);
    assert_eq!(
        report.total_lint_findings,
        result.findings.len() as u64,
        "aggregator total matches engine emission count"
    );

    // 7. retired/decorator-usage should be the sole top-50 entry.
    assert!(
        !report.top_50_diagnostics.is_empty(),
        "top-50 must contain at least one entry"
    );
    let top = &report.top_50_diagnostics[0];
    assert_eq!(top.rule_id, "retired/decorator-usage");
    assert_eq!(top.finding_count, result.findings.len() as u64);
    assert_eq!(
        top.diagnostic_id.as_deref(),
        Some("vox/retired/decorator-usage"),
        "diagnostic_id must round-trip from the catalog through the event into the rollup"
    );
    assert_eq!(top.autofix_applied_count, 0);
    assert_eq!(top.autofix_rejected_count, 0);
    assert_eq!(top.autofix_applied_rate, None);

    // 8. No repair sessions ran during this fixture.
    assert_eq!(report.total_repair_sessions, 0);
    assert_eq!(report.repair_outcomes.total_sessions, 0);
    assert_eq!(report.repair_outcomes.success_rate, None);

    // 9. JSON round-trip — proves the report is wire-compatible with what the
    //    on-disk `corpus-feedback/<quarter>.json` file would carry.
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    let back: vox_audit::aggregator::CorpusFeedbackReport =
        serde_json::from_str(&json).expect("deserialize report");
    assert_eq!(report, back);
}
