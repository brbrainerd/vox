//! Golden JSON for `vox_cli::pipeline::format_check_for_llm_json`.

use std::path::{Path, PathBuf};

#[test]
fn check_for_llm_envelope_shape_rust_import_fixture() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let fixture = fixture_dir.join("golden_rust_import_lowering.vox");
    let source = std::fs::read_to_string(&fixture).expect("read fixture");
    let file_label = Path::new("tests/fixtures/golden_rust_import_lowering.vox");

    let raw = vox_cli::pipeline::format_check_for_llm_json(&source, file_label);
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    // Normalize file_path for Windows CI.
    if let Some(fp) = v.get_mut("file_path").and_then(|x| x.as_str()) {
        *v.get_mut("file_path").unwrap() = serde_json::json!(fp.replace('\\', "/"));
    }

    insta::assert_json_snapshot!(v);
}

// ---------------------------------------------------------------------------
// Lint findings integration (requires `stub-check` feature)
// ---------------------------------------------------------------------------

/// Verify that `format_check_for_llm_json` surfaces `lint_findings` for a Vox
/// file that contains a known lint violation when the `stub-check` feature is on.
///
/// The fixture uses an `@endpoint` without `@auth` or `@public`, which is caught
/// by `AuthEndpointDetector` (`vox/auth/endpoint-missing-decorator`).
#[cfg(feature = "stub-check")]
#[test]
fn lint_findings_populated_for_auth_endpoint_violation() {
    // Minimal Vox file: @endpoint without @auth or @public is a lint violation.
    let source = r#"
@endpoint
fn get_users() -> List[User] {
    db.query_all()
}
"#;
    let file = Path::new("test_auth_check.vox");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let lint_findings = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .expect("lint_findings array must be present in stub-check build");

    assert!(
        !lint_findings.is_empty(),
        "expected at least one lint finding for @endpoint without @auth; got none"
    );

    // At least one finding should be from the auth-endpoint detector.
    let auth_finding = lint_findings.iter().find(|f| {
        f.get("rule_id")
            .and_then(|r| r.as_str())
            .map(|r| r.contains("auth"))
            .unwrap_or(false)
    });
    assert!(
        auth_finding.is_some(),
        "expected an auth-related lint finding; rule_ids present: {:?}",
        lint_findings
            .iter()
            .filter_map(|f| f.get("rule_id").and_then(|r| r.as_str()))
            .collect::<Vec<_>>()
    );

    let f = auth_finding.unwrap();

    // Finding should carry a rationale string.
    assert!(
        f.get("rationale").and_then(|r| r.as_str()).is_some(),
        "auth finding must include a rationale field"
    );

    // Finding should carry an explain_url.
    let explain_url = f.get("explain_url").and_then(|u| u.as_str()).unwrap_or("");
    assert!(
        explain_url.starts_with("https://vox-lang.org/diag/vox/auth/"),
        "explain_url should point to the auth diagnostic page; got: {explain_url}"
    );

    // Severity must be a recognized string.
    let severity = f.get("severity").and_then(|s| s.as_str()).unwrap_or("");
    assert!(
        matches!(severity, "info" | "warning" | "error" | "critical"),
        "unexpected severity value: {severity}"
    );
}

/// When `stub-check` is compiled in but the source has no lint violations,
/// the `lint_findings` field should be absent (omitted by `skip_serializing_if`).
#[cfg(feature = "stub-check")]
#[test]
fn lint_findings_absent_when_no_violations() {
    // Clean Vox file: @public @endpoint satisfies the auth-endpoint rule.
    let source = r#"
@public
@endpoint
fn health() -> Status {
    Status::Ok
}
"#;
    let file = Path::new("test_clean.vox");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    // `lint_findings` should either be absent or empty when there are no violations.
    let findings_count = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    // The clean file may still trigger other warnings (e.g. magic-value, line-endings)
    // so we can't assert exactly zero.  What we CAN assert is that any auth finding
    // is absent for this file.
    let auth_violation = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter().any(|f| {
                f.get("rule_id")
                    .and_then(|r| r.as_str())
                    .map(|r| r.contains("auth"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    assert!(
        !auth_violation,
        "clean file should not trigger auth-endpoint lint (found {findings_count} total findings)"
    );
}

/// Verify that `minimal_repro` is populated on lint findings where the detector
/// provides a reproduction snippet.
///
/// `AuthEndpointDetector` ships a `minimal_repro()` implementation, so any
/// finding from that rule must carry the field in the JSON envelope.
#[cfg(feature = "stub-check")]
#[test]
fn lint_finding_includes_minimal_repro_when_detector_provides_one() {
    let source = r#"
@endpoint
fn get_orders() -> List[Order] {
    db.query_all()
}
"#;
    let file = std::path::Path::new("test_repro.vox");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let lint_findings = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .expect("lint_findings array must be present");

    // Locate the auth-endpoint finding.
    let auth_finding = lint_findings
        .iter()
        .find(|f| {
            f.get("rule_id")
                .and_then(|r| r.as_str())
                .map(|r| r.contains("auth"))
                .unwrap_or(false)
        })
        .expect("expected an auth-endpoint finding for @endpoint without @auth");

    // The `minimal_repro` field must be present and non-empty.
    let repro = auth_finding
        .get("minimal_repro")
        .and_then(|r| r.as_str())
        .unwrap_or("");
    assert!(
        !repro.is_empty(),
        "auth-endpoint finding must carry a minimal_repro snippet; got empty/absent"
    );
    // The snippet should mention both the violation and the fix keyword.
    assert!(
        repro.contains("@endpoint"),
        "minimal_repro snippet should show @endpoint usage; got: {repro}"
    );
    assert!(
        repro.contains("@auth") || repro.contains("@public"),
        "minimal_repro snippet should show the fix (@auth or @public); got: {repro}"
    );
}

/// Verify the lint finding JSON structure matches what `vox repair` parses.
///
/// The repair loop reads `rule_id`, `line`, `message`, `suggestion`, and
/// `minimal_repro` directly from the `serde_json::Value` envelope.  This
/// test pins those field names so a breaking rename is caught immediately.
#[cfg(feature = "stub-check")]
#[test]
fn lint_finding_json_fields_match_repair_loop_expectations() {
    let source = r#"
@endpoint
fn get_billing() -> List[Invoice] {
    db.query_all()
}
"#;
    let file = std::path::Path::new("billing.vox");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let findings = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .expect("lint_findings present");

    let f = findings
        .iter()
        .find(|f| {
            f.get("rule_id")
                .and_then(|r| r.as_str())
                .map(|r| r.contains("auth"))
                .unwrap_or(false)
        })
        .expect("auth-endpoint finding");

    // Fields the repair loop accesses via .get("field") + .as_str()/.as_u64():
    assert!(
        f.get("rule_id").and_then(|v| v.as_str()).is_some(),
        "rule_id must be a string"
    );
    assert!(
        f.get("line").and_then(|v| v.as_u64()).is_some(),
        "line must be a non-negative integer"
    );
    assert!(
        f.get("message").and_then(|v| v.as_str()).is_some(),
        "message must be a string"
    );
    // suggestion and minimal_repro are optional but when present must be strings
    if let Some(s) = f.get("suggestion") {
        assert!(s.as_str().is_some(), "suggestion must be a string when present");
    }
    if let Some(r) = f.get("minimal_repro") {
        assert!(r.as_str().is_some(), "minimal_repro must be a string when present");
    }
}
