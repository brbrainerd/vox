use anyhow::{Context, Result};
use std::process::Command;

pub fn run() -> Result<()> {
    // Start the vox-lsp server process, connecting its stdio to ours.
    // This allows the CLI to act as a proxy or direct launcher for the LSP.

    let mut child = Command::new("vox-lsp")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("Failed to spawn vox-lsp binary. ensure 'vox-lsp' is in your PATH.")?;

    let status = child.wait()?;

    if !status.success() {
        anyhow::bail!("vox-lsp exited with status: {}", status);
    }

    Ok(())
}
