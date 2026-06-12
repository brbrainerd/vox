//! `vox ci dep-sprawl` — frozen-core crates must stay under a direct-dependency cap.

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace spine crates whose direct dependency count is capped.
const FROZEN_CORE: &[&str] = &[
    "vox-compiler",
    "vox-cli",
    "vox-actor-runtime",
    "vox-db",
    "vox-secrets",
    "vox-orchestrator",
    "vox-populi",
    "vox-ml-cli",
    "vox-gamify",
];

fn cargo_bin() -> PathBuf {
    if let Ok(h) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let win = PathBuf::from(&h).join(".cargo/bin/cargo.exe");
        if win.is_file() {
            return win;
        }
    }
    PathBuf::from("cargo")
}

/// Collect cap violations from a `cargo metadata` packages array.
pub(crate) fn violations_from_packages(packages: &[Value], cap: usize) -> Vec<String> {
    let mut violations = Vec::new();
    for pkg in packages {
        let Some(name) = pkg["name"].as_str() else {
            continue;
        };
        if !FROZEN_CORE.contains(&name) {
            continue;
        }
        let dependencies = pkg["dependencies"].as_array().map_or(0, |d| d.len());
        if dependencies > cap {
            violations.push(format!(
                "{name} has {dependencies} direct dependencies (cap: {cap})"
            ));
        }
    }
    violations
}

/// Run dependency sprawl guard against workspace `cargo metadata`.
pub fn run(repo_root: &Path, cap: usize) -> Result<()> {
    println!("Running dependency sprawl guard (cap: {cap} direct dependencies)...");
    let cargo = cargo_bin();

    let output = Command::new(&cargo)
        .current_dir(repo_root)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("cargo metadata failed"));
    }

    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("invalid metadata format"))?;

    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or_default();
        if !FROZEN_CORE.contains(&name) {
            continue;
        }
        let dependencies = pkg["dependencies"].as_array().map_or(0, |d| d.len());
        println!("  {name}: {dependencies} direct dependencies");
    }

    let violations = violations_from_packages(packages, cap);
    if !violations.is_empty() {
        for v in &violations {
            eprintln!("ERROR: {v}");
        }
        bail!(
            "Dependency sprawl check failed with {} violations",
            violations.len()
        );
    }

    println!("Dependency sprawl check passed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frozen_core_list_is_stable() {
        assert!(FROZEN_CORE.contains(&"vox-compiler"));
        assert!(FROZEN_CORE.contains(&"vox-cli"));
        assert!(FROZEN_CORE.contains(&"vox-orchestrator"));
    }

    #[test]
    fn violations_from_packages_flags_over_cap_only_for_frozen_core() {
        let packages = vec![
            json!({
                "name": "vox-compiler",
                "dependencies": [{}, {}, {}]
            }),
            json!({
                "name": "vox-other",
                "dependencies": [{}, {}, {}, {}, {}]
            }),
            json!({
                "name": "vox-cli",
                "dependencies": [{}]
            }),
        ];
        let violations = violations_from_packages(&packages, 2);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("vox-compiler"));
        assert!(violations[0].contains("cap: 2"));
    }

    #[test]
    fn violations_from_packages_passes_when_under_cap() {
        let packages = vec![json!({
            "name": "vox-db",
            "dependencies": [{}]
        })];
        assert!(violations_from_packages(&packages, 50).is_empty());
    }

    #[test]
    fn dep_sprawl_passes_on_real_repo() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");
        run(repo_root, 200).expect("dep-sprawl must pass on current repo");
    }
}
