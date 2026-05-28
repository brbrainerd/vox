//! Single-branch CodeRabbit submit (git checkout -b in the main repo).

use std::path::Path;

use anyhow::{Context, Result};
use vox_forge::github::GitHubProvider;
use vox_forge::{GitForgeProvider, NewChangeRequest};

use super::super::api::{forge_token, owner_repo_from_path};
use super::super::comments::trigger_coderabbit;

pub async fn submit(
    path: &Path,
    branch: Option<&str>,
    title: Option<&str>,
    no_commit: bool,
) -> Result<u64> {
    let (owner, repo) = owner_repo_from_path(path)?;

    let branch_name = branch
        .map(String::from)
        .unwrap_or_else(|| format!("coderabbit/{}", chrono::Utc::now().format("%Y%m%d-%H%M%S")));

    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(path)
        .status()
        .await
        .context("git checkout -b")?;
    if !status.success() {
        let status2 = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["checkout", &branch_name])
            .current_dir(path)
            .status()
            .await
            .context("git checkout")?;
        if !status2.success() {
            anyhow::bail!("git checkout -b failed");
        }
    }

    if !no_commit {
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
            .args(["commit", "-m", "chore: batch for CodeRabbit review"])
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
        .args(["push", "-uf", "origin", &branch_name])
        .current_dir(path)
        .status()
        .await
        .context("git push")?;
    if !status.success() {
        anyhow::bail!("git push failed for {}", branch_name);
    }

    let token = forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;

    let repo_info = provider
        .repo_info(&owner, &repo)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let target_branch = repo_info.default_branch;

    let pr_title = title
        .map(String::from)
        .unwrap_or_else(|| format!("CodeRabbit review: {branch_name}"));
    let body_str = "Automated PR for CodeRabbit review.\n\n@coderabbitai review";
    let cr = provider
        .create_change_request(
            &owner,
            &repo,
            NewChangeRequest {
                title: &pr_title,
                body: body_str,
                source_branch: &branch_name,
                target_branch: &target_branch,
                draft: false,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let _ = trigger_coderabbit(&token, &owner, &repo, cr.number, false).await;

    eprintln!("PR #{} opened: {}", cr.number, cr.web_url);
    Ok(cr.number)
}
