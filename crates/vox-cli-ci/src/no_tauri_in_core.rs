//! `vox ci no-tauri-in-core` — forbid Tauri deps outside GUI/codegen crates (ADR-037).

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

const TAURI_EXEMPT_CRATES: &[&str] = &["vox-gui", "vox-tauri-codegen", "vox-tauri-stt"];

/// Scan `crates/*/Cargo.toml` and fail when a non-exempt crate lists a Tauri dependency.
pub fn run(repo_root: &Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    for entry in fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_exempt_crate(&name) {
            continue;
        }
        let toml_path = entry.path().join("Cargo.toml");
        if !toml_path.exists() {
            continue;
        }
        let contents = fs::read_to_string(&toml_path)?;
        if cargo_toml_has_tauri_dep(&contents) {
            bail!(
                "Rule violation: crate '{}' depends on tauri, which is forbidden outside of vox-gui and codegen crates. See ADR-037.",
                name
            );
        }
    }
    println!("no-tauri-in-core OK.");
    Ok(())
}

fn is_exempt_crate(name: &str) -> bool {
    TAURI_EXEMPT_CRATES.contains(&name)
}

fn cargo_toml_has_tauri_dep(contents: &str) -> bool {
    contents.contains("tauri =")
        || contents.contains("tauri-build =")
        || contents.contains("tauri-plugin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exempt_crates_are_gui_and_codegen_only() {
        assert!(is_exempt_crate("vox-gui"));
        assert!(is_exempt_crate("vox-tauri-codegen"));
        assert!(!is_exempt_crate("vox-orchestrator"));
    }

    #[test]
    fn detects_tauri_dependency_lines() {
        assert!(cargo_toml_has_tauri_dep("tauri = { workspace = true }"));
        assert!(cargo_toml_has_tauri_dep("tauri-build = \"2\""));
        assert!(cargo_toml_has_tauri_dep("tauri-plugin-shell = \"2\""));
        assert!(!cargo_toml_has_tauri_dep("serde = { workspace = true }"));
    }
}
