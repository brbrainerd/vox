//! Local smoke for `vox compile` wiring — mirrors `.github/workflows/compile-matrix.yml`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn compile_help_candidates(repo_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(cur) = std::env::current_exe() {
        out.push(cur);
    }
    #[cfg(windows)]
    {
        out.push(repo_root.join("target/debug/vox.exe"));
        out.push(repo_root.join("target/release/vox.exe"));
    }
    #[cfg(not(windows))]
    {
        out.push(repo_root.join("target/debug/vox"));
        out.push(repo_root.join("target/release/vox"));
    }

    out.into_iter().fold(Vec::new(), |mut acc, p| {
        if !acc.iter().any(|q: &PathBuf| q == &p) {
            acc.push(p);
        }
        acc
    })
}

fn try_compile_help_via_binary(repo_root: &Path, exe: &Path) -> Option<std::process::ExitStatus> {
    if !exe.is_file() {
        return None;
    }
    Command::new(exe)
        .current_dir(repo_root)
        .args(["compile", "--help"])
        .status()
        .ok()
}

/// Smoke `vox compile --help` from `repo_root`.
///
/// - **Preferred:** run an existing `vox` binary (`current_exe`, then `target/{debug,release}/vox`)
///   so Windows dev shells avoid `cargo run` relinking while `vox.exe` is locked.
/// - **Fallback:** `cargo run -p vox-cli -- compile --help` — matches `.github/workflows/compile-matrix.yml`.
pub fn run(repo_root: &Path) -> Result<()> {
    for exe in compile_help_candidates(repo_root) {
        if let Some(status) = try_compile_help_via_binary(repo_root, &exe) {
            if status.success() {
                println!("✓ compile-matrix: `vox compile --help` OK");
                return Ok(());
            }
        }
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .current_dir(repo_root)
        .args(["run", "-p", "vox-cli", "--", "compile", "--help"])
        .status()
        .context("spawn `cargo run -p vox-cli -- compile --help`")?;
    anyhow::ensure!(
        status.success(),
        "`cargo run -p vox-cli -- compile --help` failed with status {status}"
    );
    println!("✓ compile-matrix: `vox compile --help` OK");
    Ok(())
}
