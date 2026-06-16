use std::path::Path;
use std::process::Command;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
}

#[test]
fn backend_status_prints_vault_health_lines() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .current_dir(workspace_root())
        .args(["secrets", "backend-status"])
        .output()
        .expect("spawn vox");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("secrets backend mode"),
        "expected backend mode line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("vault health:"),
        "expected vault health line, got:\n{stdout}"
    );
}
