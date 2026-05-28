//! Stacked PR chunks and orphan baselines.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use vox_forge::github::GitHubProvider;
use vox_forge::{GitForgeProvider, NewChangeRequest};

use super::super::api::{forge_token, owner_repo_from_path};
use super::super::comments::trigger_coderabbit;

/// Stacked chunk PR: branch from `base_branch`, checkout files from `default_branch` tip, commit, push, PR into `base_branch`.
pub async fn create_stack_chunk_pr(
    path: &Path,
    default_branch: &str,
    base_branch: &str,
    new_branch: &str,
    files: &[String],
    full_review: bool,
) -> Result<u64> {
    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["checkout", "-b", new_branch, base_branch])
        .current_dir(path)
        .status()
        .await
        .context("git checkout -b for stack chunk")?;
    if !status.success() {
        anyhow::bail!("git checkout -b {new_branch} {base_branch} failed");
    }

    if !files.is_empty() {
        let mut args = vec!["checkout", default_branch, "--"];
        args.extend(files.iter().map(|s| s.as_str()));
        let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(&args)
            .current_dir(path)
            .status()
            .await
            .context("git checkout files from default branch")?;
        if !status.success() {
            anyhow::bail!("git checkout {default_branch} -- <files> failed");
        }

        let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(path)
            .status()
            .await
            .context("git add -A")?;
        if !status.success() {
            anyhow::bail!("git add -A failed");
        }

        let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args([
                "commit",
                "-m",
                &format!(
                    "feat: CodeRabbit review chunk {}",
                    new_branch.trim_start_matches("cr-review-")
                ),
            ])
            .current_dir(path)
            .status()
            .await
            .context("git commit")?;
        if !status.success() {
            anyhow::bail!("git commit failed (nothing to commit or commit error)");
        }
    }

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["push", "-u", "origin", new_branch])
        .current_dir(path)
        .status()
        .await
        .context("git push")?;
    if !status.success() {
        anyhow::bail!("git push failed");
    }

    let (owner, repo) = owner_repo_from_path(path)?;
    let token = forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;

    let pr_title = format!("CodeRabbit review: {new_branch}");
    let body = format!(
        "Automated stacked PR for CodeRabbit review.\n\n@coderabbitai {}",
        if full_review { "full review" } else { "review" }
    );
    let cr = provider
        .create_change_request(
            &owner,
            &repo,
            NewChangeRequest {
                title: &pr_title,
                body: body.as_str(),
                source_branch: new_branch,
                target_branch: base_branch,
                draft: false,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let _ = trigger_coderabbit(&token, &owner, &repo, cr.number, full_review).await;

    eprintln!(
        "PR #{} opened: {} (base={})",
        cr.number, cr.web_url, base_branch
    );
    Ok(cr.number)
}

/// Create an empty orphan branch for stack baseline (requires **clean** working tree).
pub async fn create_orphan_baseline(path: &Path, branch_name: &str) -> Result<()> {
    let out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .await
        .context("git status --porcelain")?;
    if !out.stdout.is_empty() {
        anyhow::bail!(
            "Working tree must be clean for `stack-submit` orphan baseline (destructive checkout).\n\
             Commit your work first, or use `vox review coderabbit semantic-submit` (worktrees; no orphan baseline).\n\
             Uncommitted changes:\n{}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    eprintln!(
        "[warn] stack-submit: `git checkout --orphan {branch_name}` will replace the current checkout with a minimal tree until the command finishes and restores your branch."
    );

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["checkout", "--orphan", branch_name])
        .current_dir(path)
        .status()
        .await
        .context("git checkout --orphan")?;
    if !status.success() {
        anyhow::bail!("git checkout --orphan {branch_name} failed");
    }

    let _ = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["read-tree", "--empty"])
        .current_dir(path)
        .status()
        .await;

    let readme = "# CodeRabbit stack baseline\n\nEmpty baseline for stacked review PRs.\n";
    fs::write(path.join("README.md"), readme).context("Write baseline README")?;

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["add", "README.md"])
        .current_dir(path)
        .status()
        .await
        .context("git add README.md")?;
    if !status.success() {
        anyhow::bail!("git add README.md failed");
    }

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["commit", "-m", "chore: empty baseline for CodeRabbit stack"])
        .current_dir(path)
        .status()
        .await
        .context("git commit")?;
    if !status.success() {
        anyhow::bail!("git commit failed");
    }

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["push", "-u", "origin", branch_name])
        .current_dir(path)
        .status()
        .await
        .context("git push")?;
    if !status.success() {
        anyhow::bail!("git push failed");
    }

    Ok(())
}
