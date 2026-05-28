use anyhow::{Result, anyhow};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

use crate::commands::ci::cargo_bin;

pub fn run(root: &Path, cap: usize) -> Result<()> {
    println!(
        "Running dependency sprawl guard (cap: {} direct dependencies)...",
        cap
    );
    let cargo = cargo_bin();

    let output = Command::new(&cargo)
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("cargo metadata failed"));
    }

    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("invalid metadata format"))?;

    let mut violations = Vec::new();
    let frozen_core = vec![
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

    for pkg in packages {
        let name = pkg["name"].as_str().unwrap();
        if !frozen_core.contains(&name) {
            continue;
        }

        let dependencies = pkg["dependencies"].as_array().unwrap_or(&vec![]).len();
        println!("  {}: {} direct dependencies", name, dependencies);

        if dependencies > cap {
            violations.push(format!(
                "{} has {} direct dependencies (cap: {})",
                name, dependencies, cap
            ));
        }
    }

    if !violations.is_empty() {
        for v in &violations {
            eprintln!("ERROR: {}", v);
        }
        return Err(anyhow!(
            "Dependency sprawl check failed with {} violations",
            violations.len()
        ));
    }

    println!("Dependency sprawl check passed.");
    Ok(())
}
