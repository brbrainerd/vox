//! CR-L8 corpus-feedback **flywheel** — production-shaped end-to-end proof.
//!
//! Where `telemetry_corpus_feedback_e2e.rs` proves the sink registration alone
//! and `vox-audit::cr_l8_loop_e2e` proves engine→aggregator with an in-memory
//! recorder, **this test proves the full deployment-shaped path**:
//!
//!   1. Point `VOX_CORPUS_FEEDBACK_EVENTS_DIR` at a tempdir.
//!   2. Call [`vox_cli::init_telemetry_sinks`] to register the production
//!      sink stack (`SpoolSink` + `CorpusFeedbackJsonlSink`) via the global
//!      recorder.
//!   3. Run [`vox_code_audit::ToestubEngine`] on a fixture with three retired
//!      patterns. `emit_lint_finding_telemetry` (P2.1b) emits via
//!      `record_event!` → global recorder → JSONL file on disk.
//!   4. Load the JSONL via [`vox_audit::recorder::load_events_from_dir`].
//!   5. Aggregate via [`vox_audit::aggregator::aggregate`] and assert the
//!      report carries the expected rollup.
//!
//! This is the **last link** in the flywheel: events emitted by production-
//! shaped emit-sites land on disk via production-shaped sinks, and the
//! aggregator picks them up. CI runs of this test produce real CR-L8
//! measurement evidence.
//!
//! Council-ratified 2026-05-15 (A6 closing the P2.1c integration story).

use std::path::PathBuf;

use vox_audit::aggregator::aggregate;
use vox_audit::recorder::load_events_from_dir;
use vox_code_audit::engine::{ToestubConfig, ToestubEngine, ToestubRunMode};
use vox_code_audit::report::OutputFormat;
use vox_code_audit::rules::{Language, Severity};
use vox_telemetry::TelemetryEvent;

const EVENTS_DIR_ENV: &str = "VOX_CORPUS_FEEDBACK_EVENTS_DIR";

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

// `#[tokio::test]` because `SpoolSink` (one of the sinks registered by
// `init_telemetry_sinks`) uses `tokio::spawn` when a runtime is available
// (the A5 guard makes the sync fallback safe too, but having a runtime
// matches production behavior).
#[tokio::test(flavor = "current_thread")]
async fn production_sinks_persist_engine_emissions_to_jsonl_readable_by_aggregator() {
    // 1. Tempdir for the JSONL events. The env-var override is honored by
    //    `vox_cli::telemetry_corpus_feedback_sink::resolve_events_root`.
    let tmp = tempfile::tempdir().expect("tempdir");
    let events_dir = tmp.path().to_path_buf();

    // SAFETY: integration test binary; single test function in this binary, so
    // env-var mutation is effectively single-threaded. tokio::test uses a
    // current_thread scheduler for the same reason.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(EVENTS_DIR_ENV, &events_dir);
    }

    // 2. Register the full production sink stack. DB=None so the
    //    ResearchMetricsSink doesn't run; SpoolSink + CorpusFeedbackJsonlSink
    //    both register.
    vox_cli::init_telemetry_sinks(None);

    // 3. Drive ToestubEngine on a fixture with three retired patterns. P2.1b's
    //    `emit_lint_finding_telemetry` will fire one `TelemetryEvent::LintFinding`
    //    per emitted finding.
    //
    // Use the RETIRED forms (not the canonical ones):
    //   @component fn  →  retired (use bare `component` keyword instead)
    //   @endpoint(kind: server)   →  retired (use `@server` instead)
    //   @endpoint(kind: mutation) →  retired (use `@mutation` instead)
    // The bare `@server` / `@mutation` forms ARE the canonical post-2026-05-24 surface.
    let fixtures_root = tempfile::tempdir().expect("tempdir");
    write_fixture(
        &fixtures_root,
        "flywheel.vox",
        "@component fn Dashboard() {}\n\
         @endpoint(kind: server) fn list_items() {}\n\
         @endpoint(kind: mutation) fn add_item() {}\n",
    );
    let cfg = ToestubConfig {
        roots: vec![fixtures_root.path().to_path_buf()],
        min_severity: Severity::Info,
        format: OutputFormat::Json,
        languages: Some(vec![Language::Vox]),
        rule_filter: Some(vec!["retired/decorator-usage".to_string()]),
        run_mode: ToestubRunMode::Audit,
        repository_id: Some("cr-l8-flywheel".to_string()),
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(cfg);
    let result = engine.run();
    assert!(
        result.findings.len() >= 3,
        "expected ≥3 retired-pattern findings; got {}",
        result.findings.len()
    );

    // SpoolSink's spawned tokio task may have JSONL writes in flight; let it drain.
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;

    // 4. The CR-L8 JSONL sink writes synchronously inside its `record` impl,
    //    so by now the file exists. Load via the aggregator's reader.
    let loaded = load_events_from_dir(&events_dir).expect("load events");
    assert!(
        !loaded.is_empty(),
        "events_dir {} should contain at least one JSONL with emitted events",
        events_dir.display()
    );
    let lint_count = loaded
        .iter()
        .filter(|e| matches!(e, TelemetryEvent::LintFinding(_)))
        .count();
    assert_eq!(
        lint_count,
        result.findings.len(),
        "JSONL on disk has {} LintFinding events; engine emitted {}",
        lint_count,
        result.findings.len()
    );

    // 5. Aggregate and verify the substantive report has the expected rollup.
    let report = aggregate(&loaded, "2026-05-15T00:00:00Z");
    assert_eq!(report.total_lint_findings, result.findings.len() as u64);
    assert!(!report.top_50_diagnostics.is_empty());
    let top = &report.top_50_diagnostics[0];
    assert_eq!(top.rule_id, "retired/decorator-usage");
    assert_eq!(top.finding_count, result.findings.len() as u64);
    assert_eq!(
        top.diagnostic_id.as_deref(),
        Some("vox/retired/decorator-usage")
    );

    // 6. Repository_id threaded through every emitted event end-to-end.
    for event in &loaded {
        if let TelemetryEvent::LintFinding(p) = event {
            assert_eq!(
                p.repository_id.as_deref(),
                Some("cr-l8-flywheel"),
                "ToestubConfig.repository_id must thread through emission \
                 and survive the JSONL round-trip on disk"
            );
        }
    }

    // Cleanup env var.
    // SAFETY: see set_var comment above.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var(EVENTS_DIR_ENV);
    }
}
