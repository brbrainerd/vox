use anyhow::{Result, anyhow};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cargo_bin;

pub fn run(root: &Path) -> Result<()> {
    println!("Running determinism audit on examples/golden...");
    let cargo = cargo_bin();
    let golden_dir = root.join("examples/golden");

    if !golden_dir.is_dir() {
        return Err(anyhow!("examples/golden directory not found"));
    }

    let mut entries: Vec<_> = fs::read_dir(golden_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "vox"))
        .collect();
    entries.sort_by_key(|e| e.path());

    let temp_dir1 = root.join("target/determinism-audit-1");
    let temp_dir2 = root.join("target/determinism-audit-2");

    if temp_dir1.exists() {
        fs::remove_dir_all(&temp_dir1)?;
    }
    if temp_dir2.exists() {
        fs::remove_dir_all(&temp_dir2)?;
    }
    fs::create_dir_all(&temp_dir1)?;
    fs::create_dir_all(&temp_dir2)?;

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        println!("  Checking {}...", stem);

        // Run build 1
        let out1 = temp_dir1.join(format!("{}.ts", stem));
        let status1 = Command::new(&cargo)
            .current_dir(root)
            .args([
                "run",
                "-p",
                "vox-cli",
                "--",
                "build",
                path.to_str().unwrap(),
                "-o",
                out1.parent().unwrap().to_str().unwrap(),
            ])
            .status()?;
        if !status1.success() {
            return Err(anyhow!("Build 1 failed for {}", stem));
        }

        // Run build 2
        let out2 = temp_dir2.join(format!("{}.ts", stem));
        let status2 = Command::new(&cargo)
            .current_dir(root)
            .args([
                "run",
                "-p",
                "vox-cli",
                "--",
                "build",
                path.to_str().unwrap(),
                "-o",
                out2.parent().unwrap().to_str().unwrap(),
            ])
            .status()?;
        if !status2.success() {
            return Err(anyhow!("Build 2 failed for {}", stem));
        }

        // Compare
        let content1 = fs::read(&out1)?;
        let content2 = fs::read(&out2)?;
        if content1 != content2 {
            return Err(anyhow!(
                "Nondeterministic output detected for {}. Outputs differ between runs.",
                stem
            ));
        }
    }

    println!(
        "Determinism audit passed: all golden examples produce byte-identical output across runs."
    );
    Ok(())
}
