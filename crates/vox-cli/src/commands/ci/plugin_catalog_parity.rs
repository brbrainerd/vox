//! `vox ci plugin-catalog-parity`
//!
//! Enforces that every in-tree `Plugin.toml` corresponds to a catalog entry
//! and every catalog entry referencing a local path resolves. In SP1 the
//! tree contains no Plugin.toml files yet — guard passes trivially. SP3+
//! adds real plugins and the guard starts checking.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ManifestHead {
    plugin: PluginHead,
}

#[derive(Deserialize)]
struct PluginHead {
    id: String,
}

pub fn run() -> Result<()> {
    let mut errors: Vec<String> = Vec::new();
    let catalog_ids: std::collections::HashSet<&str> = vox_plugin_catalog::all_plugins()
        .iter()
        .map(|p| p.id.as_str())
        .collect();

    // Scan for Plugin.toml under crates/.
    let crates_root = Path::new("crates");
    if crates_root.is_dir() {
        for entry in walkdir::WalkDir::new(crates_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "Plugin.toml")
        {
            let path = entry.path();
            // Skip test fixtures (e.g. vox-plugin-host/tests/fixtures/noop-*) — they are
            // not catalog plugins, matching the other plugin gates' `tests/` exclusion.
            if path.components().any(|c| c.as_os_str() == "tests") {
                continue;
            }
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let head: ManifestHead = match toml::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{}: parse error: {e}", path.display()));
                    continue;
                }
            };
            if !catalog_ids.contains(head.plugin.id.as_str()) {
                errors.push(format!(
                    "{}: plugin id '{}' is not in the catalog (add to crates/vox-plugin-catalog/catalog.toml)",
                    path.display(),
                    head.plugin.id
                ));
            }
        }
    }

    if errors.is_empty() {
        println!(
            "✓ plugin catalog parity ok ({} entries in catalog)",
            catalog_ids.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("✗ {e}");
        }
        anyhow::bail!(
            "plugin catalog parity failed with {} error(s)",
            errors.len()
        )
    }
}
