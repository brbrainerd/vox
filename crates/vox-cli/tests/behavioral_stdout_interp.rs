//! Pattern #6 (structural-only goldens): a behavioral-output (stdout) golden.
//!
//! The semcov suite asserts in-process return values of leaf functions; it never
//! executes a compiled/interpreted program and asserts its OBSERVABLE output. This
//! test closes that gap: it runs a real `.vox` program through `vox run --mode interp`
//! (the tree-walking HIR interpreter — no native compile step) and asserts the exact
//! token printed by the `print` builtin reaches process stdout.
//!
//! `print` writes via `println!` to real process stdout (vox-compiler eval/builtins.rs),
//! so the only faithful capture is spawning the `vox` binary and reading its stdout —
//! which is what this test does. `CARGO_BIN_EXE_vox` is auto-populated by cargo for this
//! integration test, guaranteeing a freshly-built binary (hermetic in CI).

use std::fs;
use std::process::Command;

#[test]
fn vox_program_prints_deterministic_stdout_under_interp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("behavioral_stdout_min.vox");
    fs::write(
        &vox_file,
        "fn main() {\n    print(\"VOX_STDOUT_OK_42\")\n}\n",
    )
    .expect("write vox");

    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "run",
            "--mode",
            "interp",
            vox_file.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("spawn vox run --mode interp");

    assert!(
        out.status.success(),
        "interp run failed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("VOX_STDOUT_OK_42"),
        "expected the printed token on stdout (real behavioral output), got stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
