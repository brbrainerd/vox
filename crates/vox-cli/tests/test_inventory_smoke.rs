//! Integration smoke: workspace inventory builds on the real repo checkout.

use std::path::PathBuf;

use vox_cli::commands::ci::test_inventory::build_inventory;

#[test]
fn workspace_inventory_nonzero_crates() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vox-cli crate dir")
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let report = build_inventory(&root).expect("inventory scan");
    assert!(
        report.summary.workspace_crate_count >= 5,
        "expected multiple workspace crates; got {}",
        report.summary.workspace_crate_count
    );
    assert!(
        report.summary.rust_files_scanned > 50,
        "expected many Rust files; got {}",
        report.summary.rust_files_scanned
    );
}
