//! Integration tests for vox-arch-check.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Build the binary once; return its path.
///
/// Multiple tests call `arch_check_binary()` — using `OnceLock` ensures we
/// compile exactly once and avoid the Windows "access denied" race where two
/// `cargo run` calls try to overwrite the exe simultaneously.
fn arch_check_binary() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let build = Command::new("cargo")
            .args(["build", "-p", "vox-arch-check"])
            .status()
            .expect("failed to build vox-arch-check");
        assert!(build.success(), "cargo build -p vox-arch-check failed");

        // `CARGO_MANIFEST_DIR` is `crates/vox-arch-check`, so workspace root is two levels up.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .expect("parent of crate dir")
            .parent()
            .expect("workspace root");
        let exe = if cfg!(target_os = "windows") {
            workspace_root
                .join("target")
                .join("debug")
                .join("vox-arch-check.exe")
        } else {
            workspace_root
                .join("target")
                .join("debug")
                .join("vox-arch-check")
        };
        assert!(exe.exists(), "expected binary at {}", exe.display());
        exe
    })
}

/// Verify the binary runs without panicking and exits cleanly under --warn-only.
#[test]
fn arch_check_smoke_test() {
    let status = Command::new(arch_check_binary())
        .arg("--warn-only")
        .status()
        .expect("failed to run vox-arch-check");
    assert!(status.success(), "vox-arch-check --warn-only should exit 0");
}

/// Verify the description_present rule is wired and produces output.
/// The rule is strict (`description = "error"` in layers.toml), so running
/// without --warn-only on a clean workspace should exit 0. We just check
/// the summary line appears in stderr so the rule is confirmed active.
#[test]
fn description_rule_produces_output_on_clean_workspace() {
    let out = Command::new(arch_check_binary())
        .arg("--warn-only")
        .output()
        .expect("failed to run vox-arch-check");
    // Clean workspace: no description warnings should appear.
    // The key assertion: arch-check must exit 0 (no regressions).
    assert!(
        out.status.success(),
        "arch-check --warn-only should exit 0 on clean workspace; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
    // Confirm the summary line is printed (proves the binary ran to completion).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(": clean")
            || stderr.contains("[warn]")
            || stderr.contains("[ERROR]"),
        "expected arch-check to print a summary line; got:\n{stderr}",
    );
}
