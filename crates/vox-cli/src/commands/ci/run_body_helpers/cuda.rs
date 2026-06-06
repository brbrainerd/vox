use anyhow::{Result, anyhow};
use std::process::Command;

use crate::commands::ci::{cargo_bin, nvcc_available, repo_root};

pub(crate) fn run_cuda_features() -> Result<()> {
    if std::env::var("SKIP_CUDA_FEATURE_CHECK").unwrap_or_default() == "1" {
        println!("CUDA feature checks skipped (SKIP_CUDA_FEATURE_CHECK=1)");
        return Ok(());
    }
    let nvcc_ok = nvcc_available();
    if !nvcc_ok {
        println!(
            "CUDA feature checks skipped (nvcc not found — use PATH or CUDA_PATH/CUDA_HOME to toolkit root)"
        );
        return Ok(());
    }
    let root = repo_root();
    let cargo = cargo_bin();
    let st1 = Command::new(&cargo)
        .current_dir(&root)
        .args(["check", "-p", "vox-speech", "--features", "cuda"])
        .status()?;
    if !st1.success() {
        return Err(anyhow!("cargo check -p vox-speech --features cuda failed"));
    }
    let st2 = Command::new(&cargo)
        .current_dir(&root)
        .args([
            "check",
            "-p",
            "vox-cli",
            "--features",
            "gpu,mens-candle-cuda",
        ])
        .status()?;
    if !st2.success() {
        return Err(anyhow!(
            "cargo check -p vox-cli --features gpu,mens-candle-cuda failed"
        ));
    }
    println!("CUDA feature checks OK");
    Ok(())
}
