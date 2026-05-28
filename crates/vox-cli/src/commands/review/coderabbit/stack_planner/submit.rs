//! Run stack-submit: manifest + optional stacked PR creation.

use std::path::Path;

use anyhow::{Context, Result};
use vox_forge::GitForgeProvider;
use vox_forge::github::GitHubProvider;
use vox_git::GitBridge;

use super::super::{config, github, path_policy};
use super::types::{StackPlanConfig, StackPlanner};

/// Generate a semantic stacked-PR manifest and optionally submit each chunk as a GitHub PR.
///
/// Writes `.coderabbit-stack-manifest.json` to `path` and prints a dry-run summary
/// or actually creates PRs when `dry_run = false`.
pub async fn run_stack_submit(
    path: &Path,
    max_files_per_pr: u32,
    tier: Option<&str>,
    delay_between_prs_secs: u64,
    dry_run: bool,
) -> Result<()> {
    // 1. Gather files via git bridge or CLI wrapper
    let output = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["ls-files"])
        .current_dir(path)
        .output()
        .await
        .context("Failed to run git ls-files")?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut all_files: Vec<String> = output_str.lines().map(|s| s.to_string()).collect();

    if all_files.is_empty() {
        anyhow::bail!("No files found or not in a git repository.");
    }

    let vox_cfg = config::load_from_dir(path);
    if !vox_cfg.exclude_prefixes.is_empty() {
        all_files.retain(|f| !path_policy::is_excluded_by_prefixes(f, &vox_cfg.exclude_prefixes));
    }

    let guard = super::super::git::WorkspaceGuard::new(path).await?;
    let res = run_stack_submit_core(
        path,
        max_files_per_pr,
        tier,
        delay_between_prs_secs,
        dry_run,
        all_files,
    )
    .await;
    guard.restore().await?;
    res
}

async fn run_stack_submit_core(
    path: &Path,
    max_files_per_pr: u32,
    tier: Option<&str>,
    delay_between_prs_secs: u64,
    dry_run: bool,
    all_files: Vec<String>,
) -> Result<()> {
    use super::super::limits;

    // 2. Plan semantic chunks
    let plan_cfg = StackPlanConfig { max_files_per_pr };
    let planner = StackPlanner::new(plan_cfg);
    let stack_manifest = planner.plan(all_files);

    let tier_info = tier
        .and_then(|s| s.parse::<limits::CodeRabbitTier>().ok())
        .unwrap_or(limits::CodeRabbitTier::Pro);

    let manifest_path = path.join(".coderabbit-stack-manifest.json");
    let json = serde_json::to_string_pretty(&stack_manifest).context("Serialize stack manifest")?;
    std::fs::write(&manifest_path, &json).context("Write stack manifest generated JSON")?;

    eprintln!("============ STACKED PR PLANNER ============");
    eprintln!(
        "[Mode]     : {}",
        if dry_run {
            "DRY RUN (No mutation)"
        } else {
            "LIVE"
        }
    );
    eprintln!("[Total]    : {} valid files", stack_manifest.total_files);
    eprintln!("[Chunks]   : {}", stack_manifest.chunks.len());
    eprintln!(
        "[Tier]     : {} ({} files/PR, {} reviews/hour)",
        tier_info,
        tier_info.files_per_review(),
        tier_info.reviews_per_hour()
    );
    eprintln!(
        "[Min delay]: {}s between PR triggers (stay under rate limit)",
        tier_info.min_delay_between_prs_secs()
    );
    eprintln!("============================================");

    for chunk in &stack_manifest.chunks {
        eprintln!(
            "  -> Chunk: {:<20} ({} files)",
            chunk.name,
            chunk.files.len()
        );
    }

    eprintln!("\nArtifact written to: {}", manifest_path.display());

    if dry_run {
        eprintln!("\n[DRY RUN] Orchestration Simulation logic:");
        let mut current_base = "cr-empty-baseline".to_string();

        eprintln!("  1. CREATE orphan branch: `{}`", current_base);
        eprintln!("  2. COMMIT boilerplate README to `{}`", current_base);

        for chunk in &stack_manifest.chunks {
            let new_branch = format!("cr-review-{}", chunk.name);
            eprintln!("  3. BRANCH `{}` from `{}`", new_branch, current_base);
            eprintln!("     a. CHECKOUT {} files from `main`", chunk.files.len());
            eprintln!(
                "     b. COMMIT 'feat: CodeRabbit review chunk {}'",
                chunk.name
            );
            eprintln!("     c. PUSH `{}`", new_branch);
            eprintln!(
                "     d. PR CREATE: base=`{}` head=`{}`",
                current_base, new_branch
            );

            current_base = new_branch;
        }

        eprintln!("\nDone. Pass `--execute` on `stack-submit` to run these commands.");
        return Ok(());
    }

    // Save current branch before destructive orphan checkout; restore even if chunk loop fails.
    let current_branch = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(path)
        .output()
        .await
        .context("git rev-parse HEAD")?;
    let current_branch = String::from_utf8_lossy(&current_branch.stdout)
        .trim()
        .to_string();

    let live_result = async {
        let bridge = GitBridge::open(path).context("Open git repo")?;
        let remote_url = bridge.remote_url().context("Get remote URL")?;
        let (owner, repo_name) = github::parse_github_owner_repo(&remote_url)
            .context("Parse owner/repo from remote URL")?;
        let token = github::forge_token()?;
        let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;
        let repo_info = provider
            .repo_info(&owner, &repo_name)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let default_branch = repo_info.default_branch;

        let baseline = "cr-empty-baseline";
        eprintln!("Creating orphan baseline: {}", baseline);
        github::create_orphan_baseline(path, baseline).await?;

        let mut pr_numbers = Vec::new();
        let mut current_base = baseline.to_string();

        for (i, chunk) in stack_manifest.chunks.iter().enumerate() {
            let new_branch = format!("cr-review-{}", chunk.name);
            eprintln!(
                "\nChunk {}/{}: {} ({} files)",
                i + 1,
                stack_manifest.chunks.len(),
                chunk.name,
                chunk.files.len()
            );

            let pr = github::create_stack_chunk_pr(
                path,
                &default_branch,
                &current_base,
                &new_branch,
                &chunk.files,
                true,
            )
            .await?;

            pr_numbers.push(pr);
            current_base = new_branch;

            if i + 1 < stack_manifest.chunks.len() && delay_between_prs_secs > 0 {
                eprintln!(
                    "Waiting {}s before next chunk (rate limit)...",
                    delay_between_prs_secs
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay_between_prs_secs)).await;
            }
        }
        Ok::<Vec<u64>, anyhow::Error>(pr_numbers)
    }
    .await;

    let restore_st = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["checkout", &current_branch])
        .current_dir(path)
        .status()
        .await
        .context("git checkout to restore branch")?;
    if !restore_st.success() {
        eprintln!(
            "[warn] could not restore branch {}: exit {:?}. Run `git checkout {}` manually.",
            current_branch,
            restore_st.code(),
            current_branch
        );
    }

    let pr_numbers = live_result?;

    eprintln!(
        "\nDone. {} PR(s) created: {:?}",
        pr_numbers.len(),
        pr_numbers
    );
    Ok(())
}
