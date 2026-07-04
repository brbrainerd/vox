//! Shape tests for the build-lane `--json` envelope (`vox --json build/test/run`).

use std::path::{Path, PathBuf};

#[test]
fn build_lane_envelope_reports_errors_and_diagnostics() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden_rust_import_lowering.vox");
    let source = std::fs::read_to_string(&fixture).expect("read fixture");
    let file_label = Path::new("tests/fixtures/golden_rust_import_lowering.vox");
    let result = vox_cli::pipeline::run_frontend_str(&source, file_label, false).expect("frontend");
    assert!(
        result.has_errors(),
        "fixture must produce error diagnostics"
    );

    let raw =
        vox_cli::pipeline::format_build_lane_envelope_json("build", file_label, &result, None);
    assert!(
        !raw.contains('\n'),
        "envelope must be single-line JSONL: {raw}"
    );
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "build");
    assert_eq!(v["ok"], false);
    assert!(v["error_count"].as_u64().expect("error_count") >= 1);
    assert!(
        !v["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .is_empty(),
        "diagnostics must carry VoxCompilerDiagnosticPayload entries"
    );
    assert!(
        v.get("exit_code").is_none(),
        "exit_code omitted when None: {raw}"
    );
}

#[test]
fn command_result_envelope_carries_exit_code() {
    let raw = vox_cli::pipeline::format_command_result_envelope_json(
        "test",
        Path::new("app.vox"),
        false,
        Some(101),
    );
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "test");
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit_code"], 101);
    assert_eq!(v["diagnostics"].as_array().expect("array").len(), 0);
}
