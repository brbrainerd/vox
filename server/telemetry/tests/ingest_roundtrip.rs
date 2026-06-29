//! Integration test: OTLP payload → allowlist filter → correct field isolation.
//!
//! These tests do NOT require a live ClickHouse connection.  They exercise the
//! parsing + server-side redaction logic in isolation using `axum-test`.
//! The ClickHouse insert path is skipped via the mock state below.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{routing::post, Router};
use serde_json::{json, Value};
use vox_server::ingest::AppState;
use vox_server::redact::{build_allowlist, filter_record};
use vox_server::schema::load_taxonomy;

// ── allowlist helpers ─────────────────────────────────────────────────────────

fn allowlist() -> HashMap<String, HashSet<String>> {
    let t = load_taxonomy().expect("taxonomy");
    build_allowlist(&t)
}

// ── unit tests: server-side allowlist filter ──────────────────────────────────

#[test]
fn command_usage_record_accepted_and_filtered() {
    let al = allowlist();
    let mut raw = HashMap::new();
    raw.insert("verb".into(), Value::String("build".into()));
    raw.insert("exit_class".into(), Value::String("success".into()));
    raw.insert("duration_bucket".into(), Value::String("lt1s".into()));
    // Inject a field that MUST be dropped.
    raw.insert("user_name".into(), Value::String("alice".into()));
    raw.insert(
        "cwd".into(),
        Value::String("/home/alice/projects/secret".into()),
    );

    let rec = filter_record("install_xyz", "vox.command", 1_700_000_000_000, raw, &al)
        .expect("known category must be accepted");

    assert_eq!(rec.event_name, "vox.command");
    assert_eq!(rec.attrs.get("verb").unwrap(), "build");
    assert!(
        !rec.attrs.contains_key("user_name"),
        "user_name must be dropped by server-side filter"
    );
    assert!(
        !rec.attrs.contains_key("cwd"),
        "cwd must be dropped by server-side filter"
    );
}

#[test]
fn skill_activation_hash_field_accepted() {
    let al = allowlist();
    let mut raw = HashMap::new();
    raw.insert("skill_id_hash".into(), Value::String("a1b2c3d4e5f6".into()));
    raw.insert("trigger_source".into(), Value::String("pinned".into()));
    raw.insert("accepted".into(), Value::Bool(true));
    raw.insert("surface".into(), Value::String("mcp".into()));
    // Inject fields that MUST be dropped.
    raw.insert(
        "skill_name".into(),
        Value::String("my-private-skill".into()),
    );
    raw.insert("user_id".into(), Value::String("user-123".into()));

    let rec =
        filter_record("install_xyz", "vox.skill", 0, raw, &al).expect("vox.skill must be accepted");

    assert!(
        rec.attrs.contains_key("skill_id_hash"),
        "hash field must survive"
    );
    assert!(
        !rec.attrs.contains_key("skill_name"),
        "skill_name must be dropped"
    );
    assert!(
        !rec.attrs.contains_key("user_id"),
        "user_id must be dropped"
    );
}

#[test]
fn unknown_category_is_rejected() {
    let al = allowlist();
    let mut raw = HashMap::new();
    raw.insert("anything".into(), Value::String("value".into()));
    let rec = filter_record("install_xyz", "vox.phishing_category", 0, raw, &al);
    assert!(rec.is_none(), "unknown category must be rejected at server");
}

#[test]
fn default_decision_integer_magnitude_bucket_accepted() {
    let al = allowlist();
    let mut raw = HashMap::new();
    raw.insert(
        "decision_id".into(),
        Value::String("llm_max_concurrent".into()),
    );
    raw.insert("chosen".into(), Value::String("medium_8".into()));
    raw.insert("outcome".into(), Value::String("comfortable".into()));
    raw.insert("magnitude_bucket".into(), Value::Number(1.into()));
    // Inject raw numeric value that MUST be dropped.
    raw.insert("raw_value".into(), Value::Number(8.into()));

    let rec = filter_record("install_xyz", "vox.default_decision", 0, raw, &al)
        .expect("default_decision must be accepted");

    assert!(
        rec.attrs.contains_key("magnitude_bucket"),
        "magnitude_bucket must survive"
    );
    assert!(
        !rec.attrs.contains_key("raw_value"),
        "raw_value must be dropped — only enum slugs allowed"
    );
}

#[test]
fn all_taxonomy_otlp_event_names_accepted() {
    let t = load_taxonomy().expect("taxonomy");
    let al = build_allowlist(&t);
    for cat in &t.categories {
        let rec = filter_record("install_xyz", &cat.otlp_event_name, 0, HashMap::new(), &al);
        assert!(
            rec.is_some(),
            "category '{}' (event '{}') must be accepted",
            cat.name,
            cat.otlp_event_name
        );
    }
}

// ── OTLP JSON parse test ──────────────────────────────────────────────────────

#[test]
fn otlp_json_parses_correctly() {
    let payload = json!({
        "resourceLogs": [{
            "resource": {
                "attributes": [
                    {"key": "install_id", "value": {"stringValue": "install_abc123"}},
                    {"key": "vox_version", "value": {"stringValue": "0.6.0"}}
                ]
            },
            "scopeLogs": [{
                "logRecords": [{
                    "timeUnixNano": "1700000000000000000",
                    "attributes": [
                        {"key": "event_type", "value": {"stringValue": "vox.command"}},
                        {"key": "verb", "value": {"stringValue": "build"}},
                        {"key": "exit_class", "value": {"stringValue": "success"}},
                        {"key": "duration_bucket", "value": {"stringValue": "lt1s"}}
                    ]
                }]
            }]
        }]
    });

    let parsed: vox_server::ingest::OtlpLogsPayload =
        serde_json::from_value(payload).expect("OTLP payload must parse");

    assert_eq!(parsed.resource_logs.len(), 1);
    let rl = &parsed.resource_logs[0];
    let install_id = rl
        .resource
        .as_ref()
        .unwrap()
        .attributes
        .iter()
        .find(|kv| kv.key == "install_id")
        .and_then(|kv| kv.value.string_value.as_deref())
        .unwrap();
    assert_eq!(install_id, "install_abc123");

    let lr = &rl.scope_logs[0].log_records[0];
    assert_eq!(lr.time_unix_nano.as_deref(), Some("1700000000000000000"));
    let verb = lr
        .attributes
        .iter()
        .find(|kv| kv.key == "verb")
        .and_then(|kv| kv.value.string_value.as_deref())
        .unwrap();
    assert_eq!(verb, "build");
}

#[test]
fn server_side_filter_is_independent_of_client_sending_extra_fields() {
    // Simulate a (possibly malicious) client that sends fields not in the taxonomy.
    let al = allowlist();
    let mut raw = HashMap::new();
    raw.insert("verb".into(), Value::String("build".into()));
    raw.insert("exit_class".into(), Value::String("success".into()));
    raw.insert("duration_bucket".into(), Value::String("lt1s".into()));
    // Malicious extra fields:
    raw.insert("api_key".into(), Value::String("sk-live-secret".into()));
    raw.insert(
        "file_contents".into(),
        Value::String("password: hunter2".into()),
    );
    raw.insert("home_dir".into(), Value::String("/home/alice".into()));

    let rec = filter_record("install_xyz", "vox.command", 0, raw, &al).unwrap();

    for secret_key in &["api_key", "file_contents", "home_dir"] {
        assert!(
            !rec.attrs.contains_key(*secret_key),
            "server-side filter must drop '{secret_key}' even when client sends it"
        );
    }
    assert_eq!(
        rec.attrs.len(),
        3,
        "exactly the 3 allowlisted fields must survive"
    );
}
