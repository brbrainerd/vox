//! `vox ci gui-visual-review` — advisory wrapper around the GUI visual AI review.
//!
//! Always warns + exits 0; never gates CI. Shells the `gui-visual-review` binary
//! in `vox-orchestrator-mcp`. By default it runs the AI review (`--ai`); pass
//! `--no-ai` to skip model calls for offline / structural-only use.

use std::path::Path;
use std::process::Command;

use anyhow::Result;

/// Run the advisory GUI visual review. ALWAYS returns `Ok(())`.
pub fn run(repo_root: &Path, no_ai: bool) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root)
        .args([
            "run",
            "-p",
            "vox-orchestrator-mcp",
            "--bin",
            "gui-visual-review",
        ])
        .arg("--");
    if !no_ai {
        cmd.arg("--ai");
    }

    match cmd.status() {
        Ok(status) => {
            println!(
                "gui-visual-review: advisory review {} (exit {}) — never gates CI",
                if status.success() { "completed" } else { "ran" },
                status.code().unwrap_or(0)
            );
        }
        Err(e) => {
            eprintln!(
                "::warning::gui-visual-review: could not launch reviewer ({e}) — advisory, ignoring"
            );
        }
    }
    Ok(())
}
