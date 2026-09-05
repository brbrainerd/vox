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

/// Every PNG icon must be 8 bits per channel.
///
/// Tauri/tao read the default window icon as 8-bit RGBA (4 bytes per pixel). A
/// 16-bit PNG carries 8 bytes per pixel, so tao computes twice the expected pixel
/// count, rejects the icon, and panics inside the macOS `did_finish_launching`
/// delegate — a *non-unwinding* panic, so the process aborts with SIGABRT before a
/// window appears. Windows never hits this (it loads `icon.ico`), which is exactly
/// why a 16-bit icon set shipped undetected and crashed every macOS launch.
#[test]
fn png_icons_are_8_bit() {
    let icons_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons");
    let mut checked = 0usize;

    for entry in std::fs::read_dir(&icons_dir).expect("icons/ is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read icon");
        // PNG: 8-byte signature, then the IHDR chunk (4 len + 4 type + payload).
        // Bit depth is payload byte 8, i.e. offset 24.
        assert!(
            bytes.len() > 25 && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{} is not a valid PNG",
            path.display()
        );
        let bit_depth = bytes[24];
        assert_eq!(
            bit_depth,
            8,
            "{} is {bit_depth}-bit; Tauri requires 8-bit PNGs or macOS aborts at launch. \
             Re-encode, e.g. `magick {} -depth 8 {}`",
            path.display(),
            path.display(),
            path.display()
        );
        checked += 1;
    }

    assert!(checked > 0, "no PNG icons found in {}", icons_dir.display());
}
