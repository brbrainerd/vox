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

        // Each example gets its own subdirectory per run — `vox build` writes a
        // fixed bundle of files (types.ts, schemas.ts, etc., not a single
        // <stem>.ts), so sharing one flat directory across every golden example
        // would let later examples silently overwrite earlier ones' output.
        let dir1 = temp_dir1.join(stem);
        let dir2 = temp_dir2.join(stem);
        fs::create_dir_all(&dir1)?;
        fs::create_dir_all(&dir2)?;

        // Run build 1
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
                dir1.to_str().unwrap(),
            ])
            .status()?;
        if !status1.success() {
            return Err(anyhow!("Build 1 failed for {}", stem));
        }

        // Run build 2
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
                dir2.to_str().unwrap(),
            ])
            .status()?;
        if !status2.success() {
            return Err(anyhow!("Build 2 failed for {}", stem));
        }

        // Compare every file written to dir1 against its counterpart in dir2.
        let mut files1: Vec<_> = fs::read_dir(&dir1)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files1.sort();
        if files1.is_empty() {
            return Err(anyhow!("Build produced no output files for {}", stem));
        }
        for f1 in &files1 {
            let fname = f1.file_name().unwrap();
            let f2 = dir2.join(fname);
            let content1 = fs::read(f1)?;
            let content2 =
                fs::read(&f2).map_err(|e| anyhow!("second run missing {}: {e}", f2.display()))?;
            if content1 != content2 {
                return Err(anyhow!(
                    "Nondeterministic output detected for {} ({}). Outputs differ between runs.",
                    stem,
                    fname.to_string_lossy()
                ));
            }
        }
    }

    println!(
        "Determinism audit passed: all golden examples produce byte-identical output across runs."
    );
    Ok(())
}
