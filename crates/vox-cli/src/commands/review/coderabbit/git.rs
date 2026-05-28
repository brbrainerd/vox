//! Git helpers for batch planner (`git diff` / numstat).

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

/// One changed file with a sort key (insertions+deletions from `git diff --numstat`).
pub struct DiffEntry {
    pub path: PathBuf,
    pub weight: u64,
}

/// Collect changed paths vs `base_ref` (default `HEAD` = working tree vs last commit).
///
/// Uses `git diff --numstat` so the batch planner can sort by change size.
pub fn collect_git_diffs(repo: &std::path::Path, base_ref: Option<&str>) -> Result<Vec<DiffEntry>> {
    let base = base_ref.unwrap_or("HEAD");
    let out = // vox-arch-check: allow git-exec
        Command::new("git")
        .args(["-c", "core.autocrlf=false", "diff", "--numstat", base])
        .current_dir(repo)
        .output()
        .context("git diff --numstat")?;

    if !out.status.success() {
        anyhow::bail!("git diff failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("").replace('\\', "/");
        if path.is_empty() {
            continue;
        }
        // Binary or rename lines may show `-`
        let add = a.parse::<u64>().unwrap_or(1);
        let del = b.parse::<u64>().unwrap_or(1);
        entries.push(DiffEntry {
            path: PathBuf::from(path),
            weight: add.saturating_add(del),
        });
    }
    Ok(entries)
}

/// Safely wraps uncommitted changes in a WIP commit, captures the SHA, and restores them later.
pub struct WorkspaceGuard {
    repo: PathBuf,
    pub local_sha: String,
    pub was_dirty: bool,
}

impl WorkspaceGuard {
    /// Initialize the guard. If working tree is dirty, creates a WIP commit. Captures `local_sha`.
    pub async fn new(repo: &std::path::Path) -> Result<Self> {
        let status_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo)
            .output()
            .await
            .context("git status --porcelain")?;

        let was_dirty = !status_out.stdout.is_empty();

        if was_dirty {
            let add_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
                .args(["add", "-A"])
                .current_dir(repo)
                .status()
                .await
                .context("git add -A")?;
            if !add_out.success() {
                anyhow::bail!("WorkspaceGuard: Failed to stage current changes for WIP commit.");
            }

            let commit_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
                .args([
                    "commit",
                    "-m",
                    "wip: coderabbit safeguard snapshot",
                    "--no-verify",
                ])
                .current_dir(repo)
                .status()
                .await
                .context("git commit")?;
            if !commit_out.success() {
                anyhow::bail!("WorkspaceGuard: Failed to create WIP commit.");
            }
        }

        let head_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo)
            .output()
            .await
            .context("git rev-parse HEAD")?;
        let local_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

        Ok(Self {
            repo: repo.to_path_buf(),
            local_sha,
            was_dirty,
        })
    }

    /// Restore the working tree to uncommitted state if it was dirty initially.
    pub async fn restore(self) -> Result<()> {
        if self.was_dirty {
            let reset_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
                .args(["reset", "HEAD~1"])
                .current_dir(&self.repo)
                .status()
                .await
                .context("git reset HEAD~1")?;
            if !reset_out.success() {
                eprintln!(
                    "[warn] WorkspaceGuard failed to reset HEAD~1. Your uncommitted changes are safely committed as 'wip: coderabbit safeguard snapshot'."
                );
            }
        }
        Ok(())
    }
}
