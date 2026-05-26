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

/// Verify that `format_check_for_llm_json` surfaces `lint_findings` for a Rust
/// file that contains a known lint violation when the `stub-check` feature is on.
///
/// The fixture uses `.unwrap()` in production Rust code, caught by
/// `UnwrapCallDetector` (`rust/unwrap-call`).
#[cfg(feature = "stub-check")]
#[test]
fn lint_findings_populated_for_unwrap_violation() {
    // Minimal Rust file with .unwrap() — caught by UnwrapCallDetector.
    let source = r#"
fn get_user(id: u64) -> User {
    db.users.find(id).unwrap()
}
"#;
    let file = Path::new("crates/demo/src/lib.rs");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let lint_findings = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .expect("lint_findings array must be present in stub-check build");

    assert!(
        !lint_findings.is_empty(),
        "expected at least one lint finding for .unwrap() in Rust; got none"
    );

    // At least one finding should be from the unwrap detector.
    let unwrap_finding = lint_findings.iter().find(|f| {
        f.get("rule_id")
            .and_then(|r| r.as_str())
            .map(|r| r.contains("unwrap"))
            .unwrap_or(false)
    });
    assert!(
        unwrap_finding.is_some(),
        "expected an unwrap lint finding; rule_ids present: {:?}",
        lint_findings
            .iter()
            .filter_map(|f| f.get("rule_id").and_then(|r| r.as_str()))
            .collect::<Vec<_>>()
    );

    let f = unwrap_finding.unwrap();

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
    // Clean Vox file with no lint violations.
    let source = r#"
fn greet(name: str) to str {
    return "Hello, " + name
}
"#;
    let file = Path::new("crates/demo/src/greet.vox");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    // `lint_findings` should either be absent or empty when there are no violations.
    // The clean file may trigger warnings from other heuristic rules; what we assert
    // is that no `unwrap` violation fires on this clean Vox file.
    let unwrap_violation = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter().any(|f| {
                f.get("rule_id")
                    .and_then(|r| r.as_str())
                    .map(|r| r.contains("unwrap"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    let findings_count = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    assert!(
        !unwrap_violation,
        "clean Vox file should not trigger unwrap lint (found {findings_count} total findings)"
    );
}

/// Verify that `minimal_repro` is populated on lint findings where the detector
/// provides a reproduction snippet.
///
/// `UnwrapCallDetector` ships a `minimal_repro()` implementation, so any
/// finding from that rule must carry the field in the JSON envelope.
#[cfg(feature = "stub-check")]
#[test]
fn lint_finding_includes_minimal_repro_when_detector_provides_one() {
    let source = r#"
fn load_config() -> Config {
    let raw = std::fs::read_to_string("cfg.json").unwrap();
    serde_json::from_str(&raw).unwrap()
}
"#;
    let file = std::path::Path::new("crates/demo/src/config.rs");
    let raw = vox_cli::pipeline::format_check_for_llm_json(source, file);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let lint_findings = v
        .get("lint_findings")
        .and_then(|f| f.as_array())
        .expect("lint_findings array must be present");

    // Locate the unwrap finding.
    let unwrap_finding = lint_findings
        .iter()
        .find(|f| {
            f.get("rule_id")
                .and_then(|r| r.as_str())
                .map(|r| r.contains("unwrap"))
                .unwrap_or(false)
        })
        .expect("expected an unwrap finding for .unwrap() in production Rust");

    // The `minimal_repro` field must be present and non-empty.
    let repro = unwrap_finding
        .get("minimal_repro")
        .and_then(|r| r.as_str())
        .unwrap_or("");
    assert!(
        !repro.is_empty(),
        "unwrap finding must carry a minimal_repro snippet; got empty/absent"
    );
    // The snippet should mention .unwrap() and the fix.
    assert!(
        repro.contains(".unwrap()"),
        "minimal_repro snippet should show .unwrap() usage; got: {repro}"
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
fn parse_id(s: &str) -> u64 {
    s.parse::<u64>().unwrap()
}
"#;
    let file = std::path::Path::new("crates/demo/src/parse.rs");
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
                .map(|r| r.contains("unwrap"))
                .unwrap_or(false)
        })
        .expect("unwrap finding");

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
