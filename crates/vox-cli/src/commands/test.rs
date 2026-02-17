use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::commands::build;

pub async fn run(file: &Path) -> Result<()> {
    // 1. Build
    let out_dir = PathBuf::from("dist");

    println!("Building for tests: {}...", file.display());
    build::run(file, &out_dir).await?;

    // 2. Run Tests
    let generated_dir = Path::new("target").join("generated");

    println!("Running tests in {}...", generated_dir.display());

    let status = Command::new("cargo")
        .arg("test")
        .current_dir(&generated_dir)
        .status()
        .context("Failed to execute cargo test")?;

    if !status.success() {
        anyhow::bail!("Tests failed with exit code: {:?}", status.code());
    }

    Ok(())
}
