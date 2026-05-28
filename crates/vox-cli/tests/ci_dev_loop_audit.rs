//! Smoke test for `vox ci dev-loop-audit`.

use std::process::Command;

#[test]
fn dev_loop_audit_json_smoke() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["ci", "dev-loop-audit", "--json"])
        .output()
        .expect("spawn vox");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"schema_version\": 1"),
        "expected schema_version in:\n{stdout}"
    );
    assert!(
        stdout.contains("\"fragmentation_risk\""),
        "expected fragmentation_risk in:\n{stdout}"
    );
}
