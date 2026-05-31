//! D3 gate: `vox audit effort-route --help` must advertise `--findings`.

#[test]
fn audit_route_help_includes_findings_flag() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["audit", "effort-route", "--help"])
        .output()
        .unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("--findings"), "help:\n{s}");
}
