//! Smoke tests: `vox ci pre-push --dry-run` profiles and `--report-json`.

use std::process::Command;

use tempfile::tempdir;

#[test]
fn pre_push_dry_run_quick_lists_fast_steps() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--quick"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in ["cargo fmt", "ci line-endings", "ci ssot-drift"] {
        assert!(stdout.contains(needle), "missing `{needle}` in:\n{stdout}");
    }
    assert!(
        stdout.contains("vox-doc-pipeline"),
        "expected scoped doc lint step in:\n{stdout}"
    );
    assert!(
        stdout.contains("doctest-md"),
        "expected scoped doctest step in:\n{stdout}"
    );
    assert!(
        stdout.contains("vox-drift-check"),
        "missing drift-check in:\n{stdout}"
    );
    assert!(
        !stdout.contains("cargo clippy"),
        "fast profile must not run workspace clippy; got:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_default_is_fast_profile() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("cargo clippy"),
        "default must be fast profile without clippy; got:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_complete_includes_clippy_not_nextest() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--complete"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("cargo clippy"),
        "complete profile must list clippy; got:\n{stdout}"
    );
    assert!(
        stdout.contains("doc-inventory"),
        "complete profile must list doc-inventory; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("cargo nextest"),
        "`--complete` alone must not run nextest; got:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_quick_writes_report_json_v4() {
    let dir = tempdir().expect("tempdir");
    let report = dir.path().join("pre-push-report.json");
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "ci",
            "pre-push",
            "--dry-run",
            "--quick",
            "--report-json",
            report.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let raw = std::fs::read_to_string(&report).expect("read report");
    assert!(
        raw.contains("\"schema_version\": 4"),
        "report missing schema_version 4:\n{raw}"
    );
    assert!(
        raw.contains("\"profile\": \"fast\""),
        "report missing profile fast:\n{raw}"
    );
    assert!(
        raw.contains("\"dry_run\": true"),
        "report missing dry_run:\n{raw}"
    );
    assert!(
        raw.contains("\"elapsed_ms\": null") || raw.contains("\"elapsed_ms\":null"),
        "dry-run steps should have null elapsed_ms:\n{raw}"
    );
}

#[test]
fn pre_push_dry_run_full_includes_nextest_ci_profile() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--full"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("DRY-RUN: cargo nextest run --workspace --profile ci --no-fail-fast"),
        "expected nextest label in:\n{stdout}"
    );
    assert!(
        stdout.contains("cargo clippy"),
        "`--full` implies complete static checks; missing clippy in:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_full_skip_complete_omits_clippy() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--full", "--skip-complete"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("DRY-RUN: cargo nextest run"),
        "expected nextest in:\n{stdout}"
    );
    assert!(
        !stdout.contains("cargo clippy"),
        "`--skip-complete` must not replay clippy; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("doc-inventory"),
        "`--skip-complete` must not replay doc-inventory; got:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_full_with_coverage_uses_llvm_cov() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--full", "--with-coverage"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("llvm-cov nextest"),
        "expected llvm-cov nextest step in:\n{stdout}"
    );
    assert!(
        stdout.contains("llvm-cov report"),
        "expected llvm-cov report step in:\n{stdout}"
    );
}

#[test]
fn pre_push_with_coverage_without_full_errors() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--with-coverage"])
        .output()
        .expect("spawn vox");
    assert!(
        !out.status.success(),
        "expected error for --with-coverage without --full"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--with-coverage") || stderr.contains("requires"),
        "expected error message about --with-coverage requiring --full:\n{stderr}"
    );
}

#[test]
fn pre_push_dry_run_full_include_slow_adds_slow_step() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--full", "--include-slow"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("slow partition"),
        "expected slow partition step in:\n{stdout}"
    );
}

#[test]
fn pre_push_dry_run_full_since_flag_accepted() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--full", "--since", "HEAD~3"])
        .output()
        .expect("spawn vox");
    // Dry-run builds steps (computing impacted crates), and HEAD~3 should work in
    // a git repo. The DRY-RUN label may vary (fallback/impacted), but the nextest
    // step should always appear.
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should contain either "impacted pkg(s)" or "workspace" (fallback)
    assert!(
        stdout.contains("nextest") || stdout.contains("llvm-cov"),
        "expected a nextest/coverage step in dry-run output:\n{stdout}"
    );
}

#[test]
fn pre_push_enforce_budgets_flag_accepted_in_dry_run() {
    // --enforce-budgets is skipped in --dry-run (no elapsed times); flag must parse cleanly.
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--enforce-budgets"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "--enforce-budgets with --dry-run must succeed;\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn pre_push_dry_run_act_lists_workflows() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "pre-push", "--dry-run", "--quick", "--act"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for workflow in [
        ".github/workflows/docs-quality.yml",
        ".github/workflows/link_checker.yml",
        ".github/workflows/ts-emit-noemit.yml",
    ] {
        assert!(
            stdout.contains(&format!("==> act: {workflow}")),
            "missing act workflow label `{workflow}` in:\n{stdout}"
        );
    }
    assert!(
        stdout.contains("DRY-RUN:") && stdout.contains("push --workflows"),
        "expected dry-run act command output in:\n{stdout}"
    );
}
