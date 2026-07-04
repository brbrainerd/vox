//! Runs the real `vox` binary through `commands::build::run()` (not just the
//! standalone envelope-formatting helpers in `pipeline.rs`) under global
//! `--json`, for both a clean build and a codegen-stage failure, asserting
//! stdout is *exactly one* parseable JSON line in each case.
//!
//! Complements `build_lane_envelope.rs` (which only exercises
//! `format_build_lane_envelope_json` / `format_command_result_envelope_json`
//! directly) and `check_diagnostics_json_golden.rs` (which covers `vox check`,
//! not `vox build`). `CARGO_BIN_EXE_vox` guarantees a freshly built binary.

use std::fs;
use std::process::Command;

fn run_json_build(vox_file: &std::path::Path, out_dir: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "--json",
            "build",
            vox_file.to_str().expect("utf8 vox path"),
            "--out-dir",
            out_dir.to_str().expect("utf8 out-dir path"),
        ])
        // `vox_foundation::tracing::try_init_cli_default_info_fallback` now writes
        // to stderr (not stdout), so ambient `info`/`warn` spans — e.g.
        // `vox_codegen::codegen_ts::emitter`'s "admin registry unavailable" notice
        // when run from a fresh tempdir with no `.vox` admin registry file — no
        // longer race the envelope on stdout. Silencing via `RUST_LOG` is kept as
        // defense-in-depth so this test stays isolated to what it actually owns
        // (the `--json` envelope channel) even if that default ever regresses.
        .env("RUST_LOG", "off")
        .output()
        .expect("spawn vox --json build")
}

fn single_stdout_json_line(out: &std::process::Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "expected exactly one non-empty stdout line under --json, got {}: stdout={stdout:?} stderr={}",
        lines.len(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(lines[0])
        .unwrap_or_else(|e| panic!("stdout line must be valid JSON: {e}; line={:?}", lines[0]))
}

#[test]
fn json_build_success_emits_single_ok_true_envelope() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("ok.vox");
    fs::write(&vox_file, "fn main() {}\n").expect("write vox fixture");
    let out_dir = tmp.path().join("out");

    let out = run_json_build(&vox_file, &out_dir);
    assert!(
        out.status.success(),
        "expected a clean build to succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = single_stdout_json_line(&out);
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "build");
    assert_eq!(v["ok"], true, "envelope: {v}");
    assert_eq!(v["error_count"], 0);
}

#[test]
fn json_build_codegen_stage_failure_still_emits_single_envelope() {
    // `@v0` named-export contract violation is a codegen-stage failure (after
    // the frontend/typecheck pass already succeeded) — exactly the class of
    // error that used to propagate with zero stdout output under `--json`.
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("bad_v0.vox");
    let out_dir = tmp.path().join("out");
    fs::create_dir_all(&out_dir).expect("create out dir");

    // Pre-seed a malformed @v0 component file so the post-frontend
    // named-export contract check fails deterministically without a network call.
    fs::write(
        out_dir.join("Widget.tsx"),
        "export default function NotNamedExport() { return null; }\n",
    )
    .expect("write malformed v0 component");
    fs::write(&vox_file, "@v0 \"chat123\" Widget {}\nfn main() {}\n").expect("write vox fixture");

    let out = run_json_build(&vox_file, &out_dir);
    assert!(
        !out.status.success(),
        "expected the malformed @v0 export to fail the build"
    );

    let v = single_stdout_json_line(&out);
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "build");
    assert_eq!(v["ok"], false, "envelope: {v}");
}

#[test]
fn json_build_parse_error_still_emits_single_envelope() {
    // A genuine PARSE (not typecheck) error takes a different code path in
    // `pipeline.rs::run_frontend_str_with_options` than a typecheck failure —
    // it used to self-print a pretty multi-line diagnostics array to stdout
    // under `--json` *and* let `build::run`'s own envelope print on top,
    // producing two contradicting outputs instead of one (see the fix in the
    // commit that added this test). Guard against that regressing.
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("bad_syntax.vox");
    let out_dir = tmp.path().join("out");
    fs::write(&vox_file, "fn main( {\n").expect("write vox fixture");

    let out = run_json_build(&vox_file, &out_dir);
    assert!(
        !out.status.success(),
        "expected the parse error to fail the build"
    );

    let v = single_stdout_json_line(&out);
    assert_eq!(v["envelope_version"], 1);
    assert_eq!(v["command"], "build");
    assert_eq!(v["ok"], false, "envelope: {v}");
}
