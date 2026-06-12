//! `vox ci plugin-dep-boundary` — cdylib plugins must not statically link the heavy Vox spine.
//!
//! Plugin crates are loaded at runtime via `dlopen` in `vox-plugin-host`. A plugin
//! that takes a compile-time dependency on the compiler / database / orchestrator /
//! CLI / actor-runtime spine pulls that entire machinery into a dynamically-loaded
//! artifact — defeating the lightweight-plugin purpose and bloating the `.dll`/`.so`.
//!
//! Light domain deps (`vox-config`, `vox-secrets`, `vox-http-client`, `vox-container`,
//! `vox-wasm-engine`, …) and any third-party crate are fine — the boundary polices
//! **spine coupling only**, not crate count.
//!
//! Known existing violators are *warned* (not failed) so the gate lands green and the
//! debt is drained crate-by-crate; any NEW spine linkage hard-fails the build.

use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Heavy spine crates a cdylib plugin must not statically link.
const SPINE: &[&str] = &[
    "vox-compiler",
    "vox-codegen",
    "vox-db",
    "vox-scientia",
    "vox-publisher",
    "vox-search",
    "vox-orchestrator",
    "vox-orchestrator-mcp",
    "vox-orchestrator-queue",
    "vox-cli",
    "vox-cli-core",
    "vox-actor-runtime",
    "vox-populi",
];

/// `vox-plugin-*` crates that are plugin INFRASTRUCTURE, not plugins themselves.
const INFRA: &[&str] = &[
    "vox-plugin-api",
    "vox-plugin-types",
    "vox-plugin-host",
    "vox-plugin-catalog",
    "vox-plugin-test-harness",
    "vox-plugin-sdk",
];

/// Plugins with a KNOWN, accepted spine dependency. Warn, don't fail.
/// Each is tracked debt to re-home behind `vox-plugin-api` / a thin facade.
/// Goal: drain this list to empty.
const KNOWN_VIOLATORS: &[&str] = &[
    "vox-plugin-publication", // vox-db, vox-scientia, vox-publisher, vox-search
    "vox-plugin-populi-mesh", // vox-db, vox-populi
    "vox-plugin-mens-candle-cuda", // vox-db, vox-compiler
    "vox-plugin-mens-candle-metal", // vox-db, vox-compiler
];

/// Scan `crates/vox-plugin-*` for spine crate linkage in `[dependencies]`.
pub fn run(repo_root: &Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");
    let mut hard: Vec<String> = Vec::new();
    let mut warned: Vec<String> = Vec::new();
    let mut scanned = 0usize;

    for entry in fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("vox-plugin-") || INFRA.contains(&name.as_str()) {
            continue;
        }
        let toml_path = entry.path().join("Cargo.toml");
        if !toml_path.exists() {
            continue; // skill-only plugins carry no Cargo.toml
        }
        scanned += 1;
        let offending = spine_deps(&fs::read_to_string(&toml_path)?);
        if offending.is_empty() {
            continue;
        }
        let msg = format!(
            "plugin '{}' statically links spine crate(s): {}.\n  \
             cdylib plugins are dlopen-loaded by vox-plugin-host and must not link the \
             compiler/db/orchestrator/cli spine. Reach core functionality through \
             vox-plugin-api or a thin facade.\n  Cargo.toml: {}",
            name,
            offending.iter().cloned().collect::<Vec<_>>().join(", "),
            toml_path.display(),
        );
        if KNOWN_VIOLATORS.contains(&name.as_str()) {
            warned.push(msg);
        } else {
            hard.push(msg);
        }
    }

    for w in &warned {
        eprintln!("warning: plugin-dep-boundary (known debt): {w}");
    }

    if hard.is_empty() {
        println!(
            "plugin-dep-boundary OK ({scanned} plugin(s) scanned, {} known-debt warning(s)).",
            warned.len()
        );
        Ok(())
    } else {
        bail!(
            "plugin-dep-boundary: {} new spine-linkage violation(s):\n\n{}",
            hard.len(),
            hard.join("\n\n")
        )
    }
}

/// Spine crate names appearing as keys in the `[dependencies]` table only
/// (dev/build deps don't link into the shipped cdylib, so they're out of scope).
fn spine_deps(cargo_toml: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut in_deps = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]";
            continue;
        }
        if !in_deps || t.is_empty() || t.starts_with('#') {
            continue;
        }
        let key = t
            .split(['=', '.', ' '])
            .next()
            .unwrap_or("")
            .trim_matches('"')
            .trim();
        if SPINE.contains(&key) {
            out.insert(key.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_spine_dep_not_light_dep() {
        let toml = "[dependencies]\n\
                    vox-config = { workspace = true }\n\
                    vox-db = { workspace = true }\n\
                    serde = \"1\"\n";
        let s = spine_deps(toml);
        assert!(s.contains("vox-db"));
        assert!(!s.contains("vox-config"));
        assert!(!s.contains("serde"));
    }

    #[test]
    fn ignores_dev_and_build_deps() {
        let toml = "[dependencies]\n\
                    vox-config = { workspace = true }\n\
                    [dev-dependencies]\n\
                    vox-db = { workspace = true }\n\
                    [build-dependencies]\n\
                    vox-compiler = { workspace = true }\n";
        assert!(spine_deps(toml).is_empty());
    }

    #[test]
    fn skips_comment_lines() {
        let toml = "[dependencies]\n# vox-db = { workspace = true }\nserde = \"1\"\n";
        assert!(spine_deps(toml).is_empty());
    }

    #[test]
    fn known_violators_list_is_stable() {
        assert!(KNOWN_VIOLATORS.contains(&"vox-plugin-publication"));
        assert!(KNOWN_VIOLATORS.contains(&"vox-plugin-populi-mesh"));
    }

    #[test]
    fn plugin_dep_boundary_passes_on_real_repo() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root");
        run(repo_root).expect("plugin-dep-boundary must pass on current repo");
    }
}
