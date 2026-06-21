/// Integration guard: the SpoolSink must ONLY write clean OTLP JSON to disk —
/// never raw TelemetryEvent fields with free-form strings (metadata_json, path, etc.).
use vox_cli::telemetry_sink::SpoolSink;
use vox_telemetry::{LintFindingEvent, ModelCallEvent, TelemetryEvent, TelemetryRecorder};

fn make_spool() -> (tempfile::TempDir, SpoolSink) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let sink = SpoolSink::new(tmp.path().to_path_buf());
    (tmp, sink)
}

/// A LintFinding event has `relative_path` (free-form) — that must NOT appear in the spool.
#[test]
fn lint_finding_drops_relative_path() {
    let (tmp, sink) = make_spool();
    let event = TelemetryEvent::LintFinding(LintFindingEvent {
        rule_id: "rule/x".into(),
        diagnostic_id: None,
        severity: "info".into(),
        relative_path: "SENSITIVE/path/to/file.vox".into(),
        line: 1,
        autofix_available: false,
        confidence: None,
        repository_id: Some("repo-abc".into()),
    });
    sink.record(&event);

    // Enumerate what was spooled.
    let pending = vox_cli::telemetry_spool::list_pending(tmp.path()).expect("list_pending");
    assert!(
        !pending.is_empty(),
        "LintFinding must produce a spool entry (it maps to 'build' category)"
    );
    let body = std::fs::read_to_string(&pending[0]).expect("read spool file");
    assert!(
        !body.contains("SENSITIVE"),
        "spool must NOT contain the free-form relative_path: {body}"
    );
    assert!(
        !body.contains("repo-abc"),
        "spool must NOT contain repository_id: {body}"
    );
}

/// A ModelCall event has `selection_rationale` (free-form) — must not appear.
#[test]
fn model_call_drops_selection_rationale() {
    let (tmp, sink) = make_spool();
    let event = TelemetryEvent::ModelCall(ModelCallEvent {
        model: "claude-3-opus".into(),
        provider: "anthropic".into(),
        route_profile: None,
        selection_rationale: Some("SUPER_SENSITIVE reasoning here".into()),
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        latency_ms: 1200,
        cost_usd: 0.01,
        cost_source: "direct".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: None,
        parent_task_id: None,
        trace_id: None,
        caller_agent_id: None,
    });
    sink.record(&event);

    let pending = vox_cli::telemetry_spool::list_pending(tmp.path()).expect("list_pending");
    assert!(!pending.is_empty(), "ModelCall must produce a spool entry");
    let body = std::fs::read_to_string(&pending[0]).expect("read spool file");
    assert!(
        !body.contains("SUPER_SENSITIVE"),
        "spool must NOT contain free-form selection_rationale: {body}"
    );
    // But the safe fields must be present.
    assert!(
        body.contains("claude-3-opus") || body.contains("anthropic"),
        "spool SHOULD contain model/provider enum fields: {body}"
    );
}

/// Events that project_event returns None for must NOT create spool entries.
#[test]
fn lint_autofix_does_not_spool() {
    use vox_telemetry::LintAutofixEvent;
    let (tmp, sink) = make_spool();
    let event = TelemetryEvent::LintAutofix(LintAutofixEvent {
        rule_id: "rule/x".into(),
        diagnostic_id: None,
        outcome: "applied".into(),
        reason: None,
        relative_path: "x.vox".into(),
        line: 1,
        repository_id: None,
    });
    sink.record(&event);

    let pending = vox_cli::telemetry_spool::list_pending(tmp.path()).expect("list_pending");
    assert!(
        pending.is_empty(),
        "LintAutofix has no mapping in project_event and must NOT spool anything"
    );
}

/// Spooled files must be valid OTLP JSON (resourceLogs structure).
#[test]
fn spool_file_is_valid_otlp_json() {
    let (tmp, sink) = make_spool();
    let event = TelemetryEvent::LintFinding(LintFindingEvent {
        rule_id: "arch/forbidden".into(),
        diagnostic_id: None,
        severity: "error".into(),
        relative_path: "src/foo.rs".into(),
        line: 42,
        autofix_available: true,
        confidence: None,
        repository_id: None,
    });
    sink.record(&event);

    let pending = vox_cli::telemetry_spool::list_pending(tmp.path()).expect("list_pending");
    let body = std::fs::read_to_string(&pending[0]).expect("read spool file");
    let v: serde_json::Value = serde_json::from_str(&body).expect("must be valid JSON");
    assert!(
        v["resourceLogs"].is_array(),
        "spool entry must have resourceLogs array: {v}"
    );
    let log_record = &v["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert!(
        log_record["body"]["stringValue"].is_string(),
        "logRecord must have a body.stringValue: {log_record}"
    );
}
