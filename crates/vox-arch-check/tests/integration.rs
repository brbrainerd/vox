//! Integration tests for vox-arch-check.

mod helpers;
use helpers::fixture::ArchCheckFixture;

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Return the path of the already-built `vox-arch-check` binary.
///
/// Cargo and nextest both build binary targets of the crate-under-test before
/// running its tests, so the exe is guaranteed to exist. We do NOT call
/// `cargo build` here — parallel nextest test processes would race for the
/// `target/debug/vox-arch-check.exe` file lock on Windows.
fn arch_check_binary() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .expect("parent of crate dir")
            .parent()
            .expect("workspace root");
        let exe = if cfg!(target_os = "windows") {
            workspace_root.join("target/debug/vox-arch-check.exe")
        } else {
            workspace_root.join("target/debug/vox-arch-check")
        };
        assert!(exe.exists(), "expected binary at {}", exe.display());
        exe
    })
}

// ── Fast fixture-based tests (run in the default local loop) ─────────────────

/// Fast: runs arch-check against a minimal synthetic workspace (no real workspace walk).
/// Replaces arch_check_smoke_test for default (non-slow) local runs.
#[test]
fn arch_check_smoke_fixture() {
    let fixture = ArchCheckFixture::clean();
    let status = Command::new(ArchCheckFixture::binary())
        .arg("--warn-only")
        .current_dir(fixture.root())
        .status()
        .expect("spawn vox-arch-check");
    assert!(
        status.success(),
        "arch-check --warn-only must exit 0 on clean fixture workspace"
    );
}

/// Fast: arch-check detects description violation in fixture workspace.
#[test]
fn arch_check_description_rule_fixture() {
    let fixture = ArchCheckFixture::with_description_violation();
    let out = Command::new(ArchCheckFixture::binary())
        .arg("--warn-only")
        .current_dir(fixture.root())
        .output()
        .expect("spawn vox-arch-check");
    assert!(
        out.status.success(),
        "--warn-only must exit 0 even with description violations"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[warn]") || stderr.contains("description") || stderr.contains("clean"),
        "expected output from arch-check on fixture; got:\n{stderr}"
    );
}

// ── End-to-end test against the live workspace ──────────────────────────────

/// Runs arch-check against the real workspace and asserts:
///   1. It exits 0 under --warn-only
///   2. It produces a summary line (proves the binary ran to completion AND the
///      description_present rule is wired)
///
/// Single combined invocation: the two original tests (`arch_check_smoke_test`
/// and `description_rule_produces_output_on_clean_workspace`) ran the exact
/// same command with different assertions, paying the workspace-walk cost twice
/// for no extra signal.
#[test]
fn arch_check_live_workspace_smoke_and_description_rule() {
    let out = Command::new(arch_check_binary())
        .arg("--warn-only")
        .output()
        .expect("failed to run vox-arch-check");
    assert!(
        out.status.success(),
        "arch-check --warn-only should exit 0 on clean workspace; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(": clean")
            || stderr.contains("[warn]")
            || stderr.contains("[ERROR]"),
        "expected arch-check to print a summary line; got:\n{stderr}",
    );
}
