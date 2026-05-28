//! D-2: Guard that no non-plugin crate takes a compile-time dependency on a cdylib plugin.
//!
//! Plugin crates with `crate-type = ["cdylib"]` (or `["cdylib", "rlib"]`) are loaded at
//! runtime via `dlopen`-style dispatch in `vox-plugin-host`. Allowing non-plugin crates to
//! list them as compile-time `[dependencies]` entries would silently link the cdylib into
//! the dependent crate's object graph — defeating the isolation boundary and bloating binaries
//! with plugin code that should never be statically linked.
//!
//! The four plugin crates currently exposed in `[workspace.dependencies]` are:
//!   `vox-plugin-nvml-probe`, `vox-plugin-runtime-container`,
//!   `vox-plugin-runtime-wasm`, `vox-plugin-publication`
//!
//! Allowlist:
//!   - `vox-plugin-host` (owns the loader; intentionally takes rlib deps for integration tests)
//!   - `vox-integration-tests` (workspace-level smoke tests, same rationale)
//!   - crates under `crates/vox-plugin-host/tests/` (test fixtures)
//!   - Plugin crates themselves (`vox-plugin-*` prefix) — they may share util via rlib

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

/// Plugin crate names that are in `[workspace.dependencies]` and carry a cdylib target.
const CDYLIB_PLUGINS: &[&str] = &[
    "vox-plugin-nvml-probe",
    "vox-plugin-runtime-container",
    "vox-plugin-runtime-wasm",
    "vox-plugin-publication",
];

/// Crates allowed to depend on cdylib plugin crates (the plugin host and its tests).
const ALLOWLIST: &[&str] = &["vox-plugin-host", "vox-integration-tests"];

pub fn check(repo_root: &Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    let mut violations: Vec<String> = Vec::new();

    for entry in fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();

        // Skip the loader itself and integration-test crate.
        if ALLOWLIST.contains(&dir_name.as_str()) {
            continue;
        }
        // Skip plugin crates — they may legitimately share rlib surfaces with each other.
        if dir_name.starts_with("vox-plugin-") {
            continue;
        }
        // Skip non-Cargo directories (e.g. fixture subdirs without their own Cargo.toml).
        let toml_path = entry.path().join("Cargo.toml");
        if !toml_path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&toml_path)?;
        for plugin in CDYLIB_PLUGINS {
            // Match `plugin-name =`, `plugin-name.workspace`, `"plugin-name" =`, etc.
            // Exclude lines that are only comments (`#`).
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                // Simple substring check is sufficient: plugin names are unique enough.
                if trimmed.contains(plugin) {
                    violations.push(format!(
                        "crate '{}' lists cdylib plugin '{}' as a compile-time dependency \
                         (Cargo.toml: {}).\n  \
                         Plugin crates must be loaded at runtime via vox-plugin-host, not linked statically.\n  \
                         See D-2 in crate-audit-and-plan-2026.md.",
                        dir_name, plugin, toml_path.display()
                    ));
                }
            }
        }
    }

    if violations.is_empty() {
        tracing::info!(
            "no-plugin-cdylib-as-compile-dep OK ({} crates scanned).",
            count_crates(&crates_dir)
        );
        Ok(())
    } else {
        bail!(
            "no-plugin-cdylib-as-compile-dep: {} violation(s) found:\n\n{}",
            violations.len(),
            violations.join("\n\n")
        )
    }
}

fn count_crates(crates_dir: &Path) -> usize {
    fs::read_dir(crates_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().join("Cargo.toml").exists())
                .count()
        })
        .unwrap_or(0)
}
