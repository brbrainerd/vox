//! `vox ci plugin-abi-parity`
//!
//! Walks `crates/` for any `Plugin.toml` declaring a code or composite
//! payload, attempts to load each via `vox-plugin-host::Loader`, and
//! asserts ABI matches. Skipped (not failed):
//!   - Plugin ids starting with `noop-bad-` (intentionally broken fixtures).
//!   - Any manifest under a `tests/` directory (test fixtures live in isolated
//!     workspaces, built at test time, never into the scanned `target/`).
//!   - Plugins that declare no artifact for the current target triple (e.g. a
//!     macOS-only Metal backend on Windows/Linux) — they aren't meant to load here.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use vox_plugin_host::{Loader, errors::LoadError};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ManifestHead {
    plugin: PluginHead,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PluginHead {
    id: String,
    #[allow(dead_code)]
    name: String,
    version: String,
    payload: PayloadHead,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
enum PayloadHead {
    Code {
        #[serde(default)]
        artifacts: std::collections::BTreeMap<String, String>,
    },
    Skill {},
    Composite {
        #[serde(default)]
        code: CodeHead,
    },
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct CodeHead {
    #[serde(default)]
    artifacts: std::collections::BTreeMap<String, String>,
}

fn target_triple_key() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x86_64"
    } else {
        "unknown"
    }
}

fn workspace_target_dir() -> std::path::PathBuf {
    std::env::var("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("target"))
}

fn try_locate_dylib(crate_name: &str, artifact_filename: &str) -> Option<std::path::PathBuf> {
    let target = workspace_target_dir();
    for profile in ["debug", "release"] {
        let p = target.join(profile).join(artifact_filename);
        if p.exists() {
            return Some(p);
        }
    }
    let _ = crate_name;
    None
}

pub fn run() -> Result<()> {
    let mut errors: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut skipped = 0usize;

    let crates_root = Path::new("crates");
    if !crates_root.is_dir() {
        println!("✓ no crates/ dir; nothing to check");
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(crates_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Plugin.toml")
    {
        let path = entry.path();
        // Skip test fixtures: they live in isolated workspaces and are built at
        // test time (see vox-plugin-host tests), never into the scanned target/.
        if path.components().any(|c| c.as_os_str() == "tests") {
            skipped += 1;
            continue;
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let head: ManifestHead = match toml::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}: parse error: {e}", path.display()));
                continue;
            }
        };
        if head.plugin.id.starts_with("noop-bad-") {
            skipped += 1;
            continue;
        }
        let artifacts = match &head.plugin.payload {
            PayloadHead::Code { artifacts } => artifacts.clone(),
            PayloadHead::Composite { code } => code.artifacts.clone(),
            PayloadHead::Skill {} => continue,
        };
        let triple = target_triple_key();
        let Some(filename) = artifacts.get(triple) else {
            // Plugin does not target the current platform (e.g. a macOS-only
            // Metal backend on Windows/Linux). Not an ABI concern here — skip.
            skipped += 1;
            continue;
        };
        let crate_name = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let Some(dylib_path) = try_locate_dylib(&crate_name, filename) else {
            errors.push(format!(
                "{}: dylib '{}' not built; run `cargo build -p {}`",
                path.display(),
                filename,
                crate_name,
            ));
            continue;
        };
        match Loader::load(&head.plugin.id, &head.plugin.version, &dylib_path) {
            Ok(_) => {
                checked += 1;
            }
            Err(LoadError::AbiMismatch(e)) => {
                errors.push(format!(
                    "{}: ABI mismatch — plugin_abi={}, host_abi={}",
                    path.display(),
                    e.plugin_abi,
                    e.host_abi,
                ));
            }
            Err(other) => {
                errors.push(format!("{}: load failed: {other}", path.display()));
            }
        }
    }
    if errors.is_empty() {
        println!(
            "✓ plugin-abi-parity ok ({} checked, {} skipped: fixtures + off-platform)",
            checked, skipped,
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("✗ {e}");
        }
        anyhow::bail!("plugin-abi-parity failed with {} error(s)", errors.len())
    }
}
