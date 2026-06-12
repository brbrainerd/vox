//! Build-time prerequisites for the Tauri shell: catch misconfigured sidecar paths
//! and missing Windows icons before `cargo build -p vox-gui` fails deep in tauri-build.

use std::path::PathBuf;

#[test]
fn external_bin_paths_stay_under_workspace_target() {
    let gui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let bins = conf["bundle"]["externalBin"]
        .as_array()
        .expect("bundle.externalBin array");
    assert!(
        !bins.is_empty(),
        "at least one externalBin entry is required (vox sidecar)"
    );

    for entry in bins {
        let rel = entry.as_str().expect("externalBin path string");
        // Historical bug: `../../../target` escaped the repo and broke local dev builds.
        assert!(
            rel.starts_with("../../target/"),
            "externalBin must use `../../target/...` from crates/vox-gui, not `{rel}`"
        );
        assert!(
            !rel.starts_with("../../../"),
            "externalBin must not traverse above the workspace root: `{rel}`"
        );
        let resolved = gui_dir.join(rel);
        let under_target = resolved.components().any(|c| c.as_os_str() == "target");
        assert!(
            under_target,
            "externalBin `{rel}` must land under a `target/` directory (got {})",
            resolved.display()
        );
    }
}

#[test]
fn windows_icon_asset_exists() {
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/icon.ico");
    assert!(
        icon.is_file(),
        "icons/icon.ico is required for tauri-build on Windows — run `cargo tauri icon` in crates/vox-gui"
    );
}
