use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

pub fn check(repo_root: &Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    for entry in fs::read_dir(crates_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "vox-gui" || name == "vox-tauri-codegen" || name == "vox-tauri-stt" {
                continue;
            }
            let toml_path = entry.path().join("Cargo.toml");
            if toml_path.exists() {
                let contents = fs::read_to_string(&toml_path)?;
                // Simple string match; could be more robust with toml parser, but sufficient for CI guard.
                if contents.contains("tauri =")
                    || contents.contains("tauri-build =")
                    || contents.contains("tauri-plugin")
                {
                    bail!(
                        "Rule violation: crate '{}' depends on tauri, which is forbidden outside of vox-gui and codegen crates. See ADR-037.",
                        name
                    );
                }
            }
        }
    }
    tracing::info!("no-tauri-in-core OK.");
    Ok(())
}
