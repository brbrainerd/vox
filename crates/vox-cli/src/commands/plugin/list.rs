//! `vox plugin list` — print all catalog entries with installed/available status.

use anyhow::Result;
use std::path::PathBuf;

/// Resolve the plugin install root:
/// `$VOX_PLUGINS_DIR` env override, else `<data_local_dir>/vox/plugins`.
///
/// Delegates to [`vox_plugin_host::resolve_plugins_root`] so the logic lives
/// in one place and `DefaultVoxHost` and the CLI always agree on the root.
pub fn plugins_root() -> PathBuf {
    vox_plugin_host::resolve_plugins_root()
}

/// Returns the versioned install dir for a given id, using the first version
/// directory found under `<root>/<id>/`, or `<root>/<id>/<version>` when
/// `version` is known.
pub fn installed_version(root: &std::path::Path, id: &str) -> Option<String> {
    let id_dir = root.join(id);
    if !id_dir.is_dir() {
        return None;
    }
    // Walk immediate children; return the first directory name (= version).
    std::fs::read_dir(&id_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
}

pub fn run() -> Result<()> {
    let root = plugins_root();
    let plugins = vox_plugin_catalog::all_plugins();

    // Header
    println!("{:<30} {:<11} {:<12} DESCRIPTION", "ID", "KIND", "STATUS");
    println!("{}", "-".repeat(90));

    for p in plugins {
        let kind = format!("{:?}", p.payload_kind).to_lowercase();
        let status = match installed_version(&root, &p.id) {
            Some(v) => format!("installed ({})", v),
            None => {
                // Check if this host OS/arch is covered by any artifact declared in the catalog.
                // For catalog entries we don't have full payload data, so just report "available".
                "available".to_string()
            }
        };
        println!("{:<30} {:<11} {:<12} {}", p.id, kind, status, p.description);
    }
    println!();
    println!("Install root: {}", root.display());
    Ok(())
}
