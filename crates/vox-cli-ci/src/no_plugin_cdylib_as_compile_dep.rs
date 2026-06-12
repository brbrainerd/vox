//! `vox ci no-plugin-cdylib-as-compile-dep` — forbid static compile-time deps on cdylib plugins.

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

const CDYLIB_PLUGINS: &[&str] = &[
    "vox-plugin-nvml-probe",
    "vox-plugin-runtime-container",
    "vox-plugin-runtime-wasm",
    "vox-plugin-publication",
];

const ALLOWLIST: &[&str] = &["vox-plugin-host", "vox-integration-tests"];

/// Scan `crates/*/Cargo.toml` and fail when a non-allowlisted crate lists a cdylib plugin dep.
pub fn run(repo_root: &Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    let mut violations: Vec<String> = Vec::new();

    for entry in fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();

        if ALLOWLIST.contains(&dir_name.as_str()) || dir_name.starts_with("vox-plugin-") {
            continue;
        }

        let toml_path = entry.path().join("Cargo.toml");
        if !toml_path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&toml_path)?;
        for plugin in CDYLIB_PLUGINS {
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if trimmed.contains(plugin) {
                    violations.push(format!(
                        "crate '{dir_name}' lists cdylib plugin '{plugin}' as a compile-time dependency \
                         (Cargo.toml: {}).\n  \
                         Plugin crates must be loaded at runtime via vox-plugin-host, not linked statically.",
                        toml_path.display()
                    ));
                }
            }
        }
    }

    if violations.is_empty() {
        println!("no-plugin-cdylib-as-compile-dep OK.");
        Ok(())
    } else {
        bail!(
            "no-plugin-cdylib-as-compile-dep: {} violation(s) found:\n\n{}",
            violations.len(),
            violations.join("\n\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_includes_plugin_host() {
        assert!(ALLOWLIST.contains(&"vox-plugin-host"));
        assert!(ALLOWLIST.contains(&"vox-integration-tests"));
    }

    #[test]
    fn cdylib_plugin_names_are_stable() {
        assert!(CDYLIB_PLUGINS.contains(&"vox-plugin-publication"));
    }
}
