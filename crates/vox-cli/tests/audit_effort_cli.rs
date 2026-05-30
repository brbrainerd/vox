//! F1: `vox audit effort --help` integration smoke.
//!
//! Locks the subcommand surface so flag renames are explicit. The exact
//! pipeline behavior is exercised by `vox-effort-audit` unit + E2E tests; this
//! test only proves the CLI wiring is intact end-to-end (clap parse + help).

#[test]
fn audit_effort_help_includes_since_and_limit_flags() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["audit", "effort", "--help"])
        .output()
        .expect("spawn vox audit effort --help");
    assert!(
        out.status.success(),
        "vox audit effort --help exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--since"), "--since missing from help:\n{s}");
    assert!(s.contains("--limit"), "--limit missing from help:\n{s}");
    assert!(s.contains("--model"), "--model missing from help:\n{s}");
    assert!(s.contains("--out-dir"), "--out-dir missing from help:\n{s}");
}
