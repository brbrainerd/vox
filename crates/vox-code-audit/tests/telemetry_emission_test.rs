//! Integration test for CR-L8 corpus-feedback telemetry emission (P2.1b).
//!
//! Lives in `tests/` so it gets its own test binary, which means our
//! `set_global_recorder` call doesn't collide with `OnceLock` writes in
//! sibling test binaries (per `vox-telemetry`'s "first-writer-wins"
//! contract). The lib unit-test suite (276 tests) continues to run with
//! no recorder, exercising the no-op path of `record_event!`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use vox_code_audit::engine::{ToestubConfig, ToestubEngine, ToestubRunMode};
use vox_code_audit::report::OutputFormat;
use vox_code_audit::rules::{Language, Severity};
use vox_telemetry::{TelemetryEvent, TelemetryRecorder, set_global_recorder};

/// In-memory recorder that captures every event for later assertion.
#[derive(Default)]
struct CapturingRecorder {
    events: Mutex<Vec<TelemetryEvent>>,
}

impl CapturingRecorder {
    fn drain(&self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut *self.events.lock().expect("mutex"))
    }
}

impl TelemetryRecorder for CapturingRecorder {
    fn record(&self, event: &TelemetryEvent) {
        self.events.lock().expect("mutex").push(event.clone());
    }
}

/// Process-wide handle: a clonable Arc to the capturing recorder + a
/// serialization mutex so that concurrent tests don't interleave events.
struct TestHarness {
    recorder: Arc<CapturingRecorder>,
    serialize: Mutex<()>,
}

fn harness() -> &'static TestHarness {
    static INNER: OnceLock<TestHarness> = OnceLock::new();
    INNER.get_or_init(|| {
        let recorder = Arc::new(CapturingRecorder::default());
        set_global_recorder(recorder.clone());
        TestHarness {
            recorder,
            serialize: Mutex::new(()),
        }
    })
}

/// Acquire the test-serialization lock and drain any pending events from
/// previous tests. Returns the guard (drop = release lock) so the caller's
/// engine.run() emits to a freshly-empty recorder.
fn enter_test() -> (MutexGuard<'static, ()>, Arc<CapturingRecorder>) {
    let h = harness();
    let guard = h.serialize.lock().unwrap_or_else(|e| e.into_inner());
    let _drained = h.recorder.drain();
    (guard, h.recorder.clone())
}

/// Write a `.vox` fixture with the given content into a tempdir.
fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

/// Build a [`ToestubConfig`] pointing at `roots` with Info-and-up severity.
fn config_for(roots: Vec<PathBuf>) -> ToestubConfig {
    ToestubConfig {
        roots,
        min_severity: Severity::Info,
        format: OutputFormat::Json,
        languages: Some(vec![Language::Vox]),
        rule_filter: Some(vec!["retired/decorator-usage".to_string()]),
        run_mode: ToestubRunMode::Audit,
        ..ToestubConfig::default()
    }
}

/// Variant that also sets `repository_id` for A1 verification.
fn config_for_with_repo(roots: Vec<PathBuf>, repository_id: &str) -> ToestubConfig {
    ToestubConfig {
        repository_id: Some(repository_id.to_string()),
        ..config_for(roots)
    }
}

#[test]
fn engine_emits_lint_finding_event_per_finding() {
    let (_guard, recorder) = enter_test();

    let tmp = tempfile::tempdir().expect("tempdir");
    // 2026-05-24 direction flip: retired-endpoint form is now
    // `@endpoint(kind: ...)`, not `@server`. Three retired patterns on
    // three lines → expect 3 LintFinding events.
    write_fixture(
        &tmp,
        "fixture.vox",
        "@component fn Dashboard() {}\n\
         @endpoint(kind: server) fn list() {}\n\
         @py.import os\n",
    );

    let engine = ToestubEngine::new(config_for(vec![tmp.path().to_path_buf()]));
    let result = engine.run();

    assert!(
        result.findings.len() >= 3,
        "expected at least 3 findings (component + endpoint(kind:server) + py.import), got {}",
        result.findings.len()
    );

    let captured = recorder.drain();
    let lint_events: Vec<_> = captured
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::LintFinding(payload) => Some(payload),
            _ => None,
        })
        .collect();

    assert_eq!(
        lint_events.len(),
        result.findings.len(),
        "expected one LintFinding event per emitted finding; got {} events for {} findings",
        lint_events.len(),
        result.findings.len()
    );

    for ev in &lint_events {
        assert_eq!(
            ev.rule_id, "retired/decorator-usage",
            "unexpected rule_id: {}",
            ev.rule_id
        );
        // Severity varies by pattern: `@component fn` and `@py.import` are
        // Warning; `@endpoint(kind: ...)` is escalated to Error in v0.6.0
        // because the `@endpoint` lexer keyword was removed and the parser
        // rejects it outright — the lint runs first as a friendly migration
        // hint.  Both are valid telemetry payloads for this detector.
        assert!(
            ev.severity == "warning" || ev.severity == "error",
            "severity must be 'warning' or 'error', got '{}' for rule {}",
            ev.severity,
            ev.rule_id
        );
        assert_eq!(
            ev.diagnostic_id.as_deref(),
            Some("vox/retired/decorator-usage"),
            "diagnostic_id should be the stable catalog ID"
        );
        assert!(
            ev.autofix_available,
            "retired_decorator detector always supplies a suggestion"
        );
        assert_eq!(ev.confidence.as_deref(), Some("high"));
        assert!(
            ev.relative_path.ends_with("fixture.vox"),
            "expected relative_path to end with fixture.vox, got {}",
            ev.relative_path
        );
        assert!(
            ev.line >= 1 && ev.line <= 3,
            "line {} should be in 1..=3 for the 3-line fixture",
            ev.line
        );
    }
}

#[test]
fn engine_emits_no_events_when_no_findings() {
    let (_guard, recorder) = enter_test();

    let tmp = tempfile::tempdir().expect("tempdir");
    // 2026-05-24 direction flip: canonical fixture uses bare-form decorators
    // (`@server`), not the now-retired `@endpoint(kind: server)`.
    write_fixture(
        &tmp,
        "clean.vox",
        "component Dashboard() {}\n\
         @server fn list_items() {}\n",
    );

    let engine = ToestubEngine::new(config_for(vec![tmp.path().to_path_buf()]));
    let result = engine.run();

    assert!(
        result.findings.is_empty(),
        "canonical fixture should produce zero findings, got {}",
        result.findings.len()
    );

    let captured = recorder.drain();
    let lint_events: Vec<_> = captured
        .iter()
        .filter(|e| matches!(e, TelemetryEvent::LintFinding(_)))
        .collect();
    assert!(
        lint_events.is_empty(),
        "no findings should produce no LintFinding events, got {}",
        lint_events.len()
    );
}

#[test]
fn engine_threads_repository_id_into_emitted_events() {
    // A1: ToestubConfig.repository_id flows into LintFindingEvent.repository_id.
    let (_guard, recorder) = enter_test();

    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(&tmp, "rep.vox", "@component fn Foo() {}\n");

    let engine = ToestubEngine::new(config_for_with_repo(
        vec![tmp.path().to_path_buf()],
        "test-repo",
    ));
    let _ = engine.run();

    let captured = recorder.drain();
    let lint_events: Vec<_> = captured
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::LintFinding(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(
        !lint_events.is_empty(),
        "expected at least one LintFinding event"
    );
    for ev in &lint_events {
        assert_eq!(
            ev.repository_id.as_deref(),
            Some("test-repo"),
            "repository_id should be threaded through from ToestubConfig"
        );
    }
}

#[test]
fn engine_emits_none_repository_id_when_unset() {
    // Default ToestubConfig leaves repository_id = None; events should
    // serialize without the field present.
    let (_guard, recorder) = enter_test();

    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(&tmp, "norep.vox", "@component fn Foo() {}\n");

    let engine = ToestubEngine::new(config_for(vec![tmp.path().to_path_buf()]));
    let _ = engine.run();

    let captured = recorder.drain();
    let ev = captured
        .iter()
        .find_map(|e| match e {
            TelemetryEvent::LintFinding(p) => Some(p),
            _ => None,
        })
        .expect("at least one LintFinding event");
    assert_eq!(ev.repository_id, None);

    // Forward-compat: skip_serializing_if = "Option::is_none" must hold.
    let json = serde_json::to_string(&TelemetryEvent::LintFinding(ev.clone())).expect("ser");
    assert!(
        !json.contains("\"repository_id\""),
        "None repository_id should be skipped in JSON; got {json}"
    );
}

#[test]
fn lint_finding_event_round_trips_through_json_at_emit_time() {
    // Regression guard: every event emitted by the engine must survive
    // JSON round-trip with no loss, since the export pipeline (P2.2/P2.3)
    // serializes events to disk before aggregation.
    let (_guard, recorder) = enter_test();

    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(&tmp, "rt.vox", "@component fn X() {}\n");
    let engine = ToestubEngine::new(config_for(vec![tmp.path().to_path_buf()]));
    let _ = engine.run();

    let captured = recorder.drain();
    let lint_event = captured
        .iter()
        .find(|e| matches!(e, TelemetryEvent::LintFinding(_)))
        .expect("at least one LintFinding event captured");

    let json = serde_json::to_string(lint_event).expect("serialize");
    let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
    match (lint_event, &back) {
        (TelemetryEvent::LintFinding(a), TelemetryEvent::LintFinding(b)) => {
            assert_eq!(a, b, "round-trip altered the payload");
        }
        _ => panic!("variant changed across round trip"),
    }
}
