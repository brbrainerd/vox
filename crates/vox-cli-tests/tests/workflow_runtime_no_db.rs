//! Gate: `vox-workflow-runtime` must build successfully without its
//! default features. Captures the SQL-tracker boundary: any future change
//! that smuggles `vox-db` into the non-default modules breaks the gate.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root resolvable")
        .to_path_buf()
}

/// `cargo build --no-default-features -p vox-workflow-runtime` must succeed.
///
/// This proves the `sql` feature is the only thing that pulls `vox-db` into
/// the dependency graph — when it's off, the interpreter + tracker trait +
/// in-memory tracker + file journal wrapper compile without it.
#[test]
fn vox_workflow_runtime_compiles_without_default_features() {
    let root = workspace_root();
    let output = Command::new("cargo")
        .args([
            "build",
            "-p",
            "vox-workflow-runtime",
            "--no-default-features",
        ])
        .current_dir(&root)
        .output()
        .expect("spawn cargo build");
    assert!(
        output.status.success(),
        "cargo build --no-default-features -p vox-workflow-runtime FAILED\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
