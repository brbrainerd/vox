//! Historical CodeRabbit PR planner.
//!
//! Generates a chain of CodeRabbit PRs comparing local state (WIP) against an older commit.

use std::path::Path;

use anyhow::{Context, Result};
use vox_forge::GitForgeProvider;
use vox_forge::github::GitHubProvider;
use vox_git::GitBridge;

use super::{
    path_policy,
    semantic_planner::{SemanticPlanner, SemanticSubmitConfig, resolve_semantic_rule_set},
};

/// Run historical submit: stash local state in a WIP commit, get diff against `hist_sha`,
/// generate semantic chunks, create worktree PRs against a baseline at `hist_sha`, and reset the WIP commit.
pub async fn run_historical_submit(
    repo: &Path,
    commit_id: &str,
    cfg: &SemanticSubmitConfig,
) -> Result<()> {
    eprintln!(
        "[historical-submit] Preparing historical review against {}...",
        commit_id
    );

    let guard = super::git::WorkspaceGuard::new(repo).await?;
    let local_sha = guard.local_sha.clone();

    // 2. Discover diff between `commit_id` and `local_sha`.
    eprintln!(
        "[historical-submit] Resolving diff: {}...{}",
        commit_id, local_sha
    );
    let diff_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args([
            "-c",
            "core.autocrlf=false",
            "diff",
            "--name-only",
            "--diff-filter=ACDMRT",
            commit_id,
            &local_sha,
        ])
        .current_dir(repo)
        .output()
        .await
        .context("git diff --name-only")?;

    if !diff_out.status.success() {
        guard.restore().await?;
        anyhow::bail!("Failed to collect diff against {}.", commit_id);
    }

    let diff_str = String::from_utf8_lossy(&diff_out.stdout);
    let mut files = parse_git_diff_name_only(&diff_str);

    // Filter tools and exclusions.
    let dropped_tool = path_policy::retain_non_coderabbit_tool_paths(&mut files);
    if dropped_tool > 0 {
        eprintln!(
            "[historical-submit] Dropped {} path(s) under `.coderabbit/`.",
            dropped_tool
        );
    }

    let vox_cfg = super::config::load_from_dir(repo);
    if !vox_cfg.exclude_prefixes.is_empty() {
        files.retain(|f| !path_policy::is_excluded_by_prefixes(f, &vox_cfg.exclude_prefixes));
    }

    files.retain(|f| !SemanticPlanner::is_ignored_with(f, &vox_cfg.allow_markdown_prefixes));

    if files.is_empty() {
        eprintln!("[historical-submit] No valid files found in diff. Restoring working tree.");
        guard.restore().await?;
        anyhow::bail!("No changed files found in the historical diff.");
    }

    eprintln!(
        "[historical-submit] Processable file count: {}",
        files.len()
    );
    files.sort();

    // 3. Plan groups.
    let baseline_branch = format!("cr-histbase-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let tier_cap = cfg.tier.files_per_review() as usize;
    let max_per = cfg.max_files_per_pr.max(1).min(tier_cap);
    let inject = vox_cfg.semantic_workspace_crates;
    let groups = vox_cfg.groups_config.as_deref().map(std::path::Path::new);
    let rule_set = resolve_semantic_rule_set(repo, groups, inject)
        .context("load semantic group rules for historical-submit")?;
    let planner = SemanticPlanner::new(max_per, rule_set, vox_cfg.legacy_chunk_split)
        .with_allow_markdown_prefixes(vox_cfg.allow_markdown_prefixes.clone());
    let mut manifest = planner.plan(files, &baseline_branch);

    if let Some(ref filter) = cfg.group_filter {
        manifest.chunks.retain(|c| c.name.contains(filter.as_str()));
    }

    eprintln!("\n══════════════════════════════════════════════");
    eprintln!("  HISTORICAL-SUBMIT plan");
    eprintln!("══════════════════════════════════════════════");
    eprintln!("  Historical : {}", commit_id);
    eprintln!(
        "  Mode       : {}",
        if cfg.execute { "EXECUTE" } else { "PLAN ONLY" }
    );
    eprintln!("  Files      : {}", manifest.total_files);
    eprintln!("  Chunks     : {}", manifest.chunks.len());
    eprintln!("──────────────────────────────────────────────");
    for chunk in &manifest.chunks {
        eprintln!("  {:.<30} {} files", chunk.name, chunk.files.len());
    }
    eprintln!("══════════════════════════════════════════════\n");

    let cr_dir = repo.join(".coderabbit");
    std::fs::create_dir_all(&cr_dir).ok();
    let manifest_path = cr_dir.join("historical-manifest.json");
    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = std::fs::write(&manifest_path, json);
    }

    if !cfg.execute {
        eprintln!("\n[PLAN ONLY] Restoring your local status now.");
        guard.restore().await?;
        return Ok(());
    }

    // 4. Push baseline branch from the historical commit.
    eprintln!(
        "\n[historical-submit] Pushing historical baseline {} -> origin/{}",
        commit_id, baseline_branch
    );
    let push_base_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args([
            "push",
            "-f",
            "origin",
            &format!("{}:refs/heads/{}", commit_id, baseline_branch),
        ])
        .current_dir(repo)
        .status()
        .await
        .context("git push historical baseline")?;

    if !push_base_out.success() {
        guard.restore().await?;
        anyhow::bail!("Failed to push baseline branch.");
    }

    let bridge = GitBridge::open(repo).context("open git repo")?;
    let remote_url = bridge.remote_url().context("get remote URL")?;
    let (owner, repo_name) = super::github::parse_github_owner_repo(&remote_url)
        .context("parse owner/repo from remote URL")?;
    let token = super::github::forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;

    let repo_info = provider
        .repo_info(&owner, &repo_name)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let default_branch = repo_info.default_branch.clone();

    // 5. Worktree Work.
    let delay = cfg.delay_secs;
    for (i, chunk) in manifest.chunks.iter().enumerate() {
        let review_branch = format!("cr-review-hist-{}", chunk.name);
        eprintln!(
            "\n[historical-submit] Chunk {}/{}: {} ({} files)",
            i + 1,
            manifest.chunks.len(),
            chunk.name,
            chunk.files.len()
        );

        let res = super::github::create_chunk_pr_via_worktree(
            repo,
            repo, // Overlay from current local working directory (which is now clean and committed securely in our wip commit)
            &default_branch,
            &baseline_branch,
            &review_branch,
            &chunk.files,
            false,
        )
        .await;

        match res {
            Ok(pr) => {
                eprintln!(
                    "[historical-submit] Created PR #{} for chunk {}",
                    pr, chunk.name
                );
            }
            Err(e) => {
                eprintln!(
                    "[warn] Failed to create PR for chunk {}: {}. Continuing.",
                    chunk.name, e
                );
            }
        }

        if i + 1 < manifest.chunks.len() && delay > 0 {
            eprintln!("Waiting {}s before next chunk (rate limit)...", delay);
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }

    // 6. Restore Local State.
    eprintln!(
        "\n[historical-submit] Finished execution. Restoring WIP state back to outstanding changes..."
    );
    guard.restore().await?;

    Ok(())
}

pub fn parse_git_diff_name_only(diff_str: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in diff_str.lines() {
        let p = line.trim().replace('\\', "/");
        if !p.is_empty() {
            files.push(p);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_diff() {
        let raw = "src/main.rs\ncrates/foo/lib.rs\n\r\n";
        let files = parse_git_diff_name_only(raw);
        assert_eq!(files, vec!["src/main.rs", "crates/foo/lib.rs"]);
    }
}
