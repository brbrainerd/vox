//! Execute semantic-submit: plan, manifest, optional baseline + worktree PRs.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use vox_db::VoxDb;
use vox_db::store::types::ExternalReviewRunParams;
use vox_forge::GitForgeProvider;
use vox_forge::github::GitHubProvider;
use vox_git::GitBridge;

use super::super::run_state as cr_run_state;
use super::super::{config, github, path_policy};
use super::collector::{collect_all_files, collect_changed_files};
use super::manifest::write_semantic_manifest;
use super::rules::{resolve_semantic_rule_set, unassigned_prefix_histogram};
use super::types::{SemanticPlanner, SemanticSubmitConfig};

/// Entry point: collect files, plan groups, optionally push baseline + isolated worktree PRs.
pub async fn run_semantic_submit(repo: &Path, cfg: &SemanticSubmitConfig) -> Result<()> {
    // ── 1. Collect files ────────────────────────────────────────────────────
    let mode_label = if cfg.full_repo {
        "full-repo (git ls-files)"
    } else {
        "diff-based (changes since HEAD)"
    };
    eprintln!("[semantic-submit] Collecting files ({mode_label})…");
    let mut all_files = if cfg.full_repo {
        collect_all_files(repo)
            .await
            .context("collect all tracked files")?
    } else {
        collect_changed_files(repo)
            .await
            .context("collect changed files")?
    };
    let dropped_tool = path_policy::retain_non_coderabbit_tool_paths(&mut all_files);
    if dropped_tool > 0 {
        eprintln!(
            "[semantic-submit] Dropped {dropped_tool} path(s) under `.coderabbit/` (tooling worktrees/state; not reviewed)."
        );
    }
    if all_files.is_empty() {
        if cfg.full_repo {
            anyhow::bail!("No tracked files found in the repository.");
        } else {
            anyhow::bail!("No changed or untracked files found. Working tree is already clean.");
        }
    }
    eprintln!(
        "[semantic-submit] Raw file count (before ignore filter): {}",
        all_files.len()
    );

    let vox_cfg = config::load_from_dir(repo);
    let mut merged_exclude = vox_cfg.exclude_prefixes.clone();
    merged_exclude.extend(cfg.extra_exclude_prefixes.iter().cloned());
    if !merged_exclude.is_empty() {
        all_files.retain(|f| !path_policy::is_excluded_by_prefixes(f, &merged_exclude));
        eprintln!(
            "[semantic-submit] After merged exclude_prefixes (Vox.toml + CLI): {} paths",
            all_files.len()
        );
    }

    let mut plan_snapshot = all_files.clone();
    plan_snapshot.sort();

    // Secure the workspace before messing with branches and run_state
    let guard = super::super::git::WorkspaceGuard::new(repo).await?;

    let res = run_semantic_submit_core(repo, cfg, all_files, plan_snapshot).await;

    guard.restore().await?;
    res
}

fn summarize_ignored_reasons(paths: &[String], planner: &SemanticPlanner) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for path in paths {
        if let Some(reason) = planner.path_ignored_reason(path) {
            *counts.entry(reason).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

fn maybe_write_ignored_paths_json(
    out: &Path,
    candidates: &[String],
    planner: &SemanticPlanner,
) -> Result<()> {
    #[derive(serde::Serialize)]
    struct IgnoredPathRow {
        path: String,
        reason: String,
    }
    let mut rows: Vec<IgnoredPathRow> = Vec::new();
    for p in candidates {
        if let Some(r) = planner.path_ignored_reason(p) {
            rows.push(IgnoredPathRow {
                path: p.clone(),
                reason: r.to_string(),
            });
        }
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    let json = serde_json::to_string_pretty(&rows).context("serialize ignored paths")?;
    std::fs::write(out, json).with_context(|| format!("write {}", out.display()))?;
    eprintln!(
        "[semantic-submit] Wrote {} ignored path(s) to {}",
        rows.len(),
        out.display()
    );
    Ok(())
}

async fn run_semantic_submit_core(
    repo: &Path,
    cfg: &SemanticSubmitConfig,
    all_files: Vec<String>,
    plan_snapshot: Vec<String>,
) -> Result<()> {
    // ── 2. Build semantic plan ──────────────────────────────────────────────
    let baseline_branch = if cfg.resume && !cfg.force_chunks {
        let prev = cr_run_state::CoderabbitRunState::load(repo)?.ok_or_else(|| {
            anyhow::anyhow!(
                "`--resume` requires `.coderabbit/run-state.json` in the repo root. \
                 Run once with `--execute`, or pass `--baseline-branch` matching a prior run and use a compatible run-state."
            )
        })?;
        match &cfg.baseline_branch {
            None => {
                eprintln!(
                    "[resume] reusing baseline branch `{}` from run-state",
                    prev.baseline_branch
                );
                prev.baseline_branch.clone()
            }
            Some(want) if want != &prev.baseline_branch => {
                anyhow::bail!(
                    "`--resume` run-state baseline is `{}` but `--baseline-branch` is `{}`. \
                     Omit `--baseline-branch` to reuse the run-state baseline, or align the flag with the saved baseline.",
                    prev.baseline_branch,
                    want
                );
            }
            Some(want) => want.clone(),
        }
    } else if let Some(b) = cfg.baseline_branch.clone() {
        b
    } else {
        format!("cr-baseline-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
    };
    let tier_cap = cfg.tier.files_per_review() as usize;
    let max_per = cfg.max_files_per_pr.max(1).min(tier_cap);
    if cfg.max_files_per_pr > tier_cap {
        eprintln!(
            "[semantic-submit] Tier {}: clamped max_files_per_pr {} → {}",
            cfg.tier, cfg.max_files_per_pr, max_per
        );
    }
    let rule_set = resolve_semantic_rule_set(
        repo,
        cfg.groups_config.as_deref(),
        cfg.semantic_workspace_crates,
    )
    .context("load semantic group rules")?;
    let planner = SemanticPlanner::new(max_per, rule_set, cfg.legacy_chunk_split)
        .with_allow_markdown_prefixes(cfg.allow_markdown_prefixes.clone());
    let mut sem_manifest = planner.plan(all_files, &baseline_branch);
    let ignored_reasons = summarize_ignored_reasons(&plan_snapshot, &planner);

    let u_name = planner.unassigned_name();
    let part_prefix = format!("{u_name}_part");
    let unassigned_count: usize = sem_manifest
        .chunks
        .iter()
        .filter(|c| c.name == u_name || c.name.starts_with(&part_prefix))
        .map(|c| c.files.len())
        .sum();
    if let Some(max_ratio) = cfg.max_unassigned_ratio {
        let denom = sem_manifest.coverage.included_files.max(1);
        let frac = unassigned_count as f64 / denom as f64;
        if frac > max_ratio {
            anyhow::bail!(
                "unassigned ratio {:.2}% exceeds max_unassigned_ratio {:.2}% ({} / {} files). \
                 Extend `contracts/review/coderabbit-semantic-groups.v1.yaml` or adjust `[review.coderabbit] max_unassigned_ratio`.",
                frac * 100.0,
                max_ratio * 100.0,
                unassigned_count,
                denom
            );
        }
    }

    // Apply group filter if requested
    if let Some(ref filter) = cfg.group_filter {
        sem_manifest
            .chunks
            .retain(|c| c.name.contains(filter.as_str()));
    }

    // ── 3. Print plan summary ───────────────────────────────────────────────
    eprintln!("\n══════════════════════════════════════════════");
    eprintln!("  SEMANTIC-SUBMIT plan");
    eprintln!("══════════════════════════════════════════════");
    eprintln!(
        "  Mode         : {}",
        if cfg.execute { "EXECUTE" } else { "PLAN ONLY" }
    );
    eprintln!(
        "  Tier         : {} ({} files/PR, {}s delay)",
        cfg.tier,
        cfg.tier.files_per_review(),
        cfg.tier.min_delay_between_prs_secs()
    );
    eprintln!(
        "  Files/PR cap : {} (effective {})",
        cfg.max_files_per_pr, max_per
    );
    eprintln!("  Baseline     : {}", baseline_branch);
    eprintln!(
        "  Coverage     : {} candidate, {} included, {} ignored",
        sem_manifest.coverage.candidate_files,
        sem_manifest.coverage.included_files,
        sem_manifest.coverage.ignored_files
    );
    eprintln!("  Chunks       : {}", sem_manifest.chunks.len());
    if !ignored_reasons.is_empty() {
        eprintln!("  Ignore rules :");
        for (reason, count) in ignored_reasons {
            eprintln!("    - {reason}: {count}");
        }
    }
    if unassigned_count > 0 {
        eprintln!(
            "  Unassigned   : {} file(s) → group `{}` (top path prefixes):",
            unassigned_count, u_name
        );
        for (pfx, n) in unassigned_prefix_histogram(&plan_snapshot, planner.rule_set())
            .into_iter()
            .take(10)
        {
            eprintln!("    - {pfx}: {n}");
        }
    }
    eprintln!("──────────────────────────────────────────────");
    for chunk in &sem_manifest.chunks {
        eprintln!("  {:.<30} {} files", chunk.name, chunk.files.len());
    }
    eprintln!("══════════════════════════════════════════════\n");

    let manifest_path = write_semantic_manifest(repo, &sem_manifest)?;
    eprintln!("[manifest] Written to: {}", manifest_path.display());
    if let Some(ref out) = cfg.write_ignored_paths {
        maybe_write_ignored_paths_json(out, &plan_snapshot, &planner)?;
    }
    if let Err(err) = persist_semantic_lineage_run(repo, &baseline_branch, &sem_manifest).await {
        eprintln!("[semantic-submit] warning: could not persist lineage run to VoxDB: {err}");
    }

    if !cfg.execute {
        eprintln!(
            "\n[PLAN ONLY] No git mutations. Re-run with `--execute` to push baseline + open PRs from isolated worktrees."
        );
        return Ok(());
    }

    // ── 4. Resolve default branch + publish baseline at origin tip ───────────
    let bridge = GitBridge::open(repo).context("open git repo")?;
    let remote_url = bridge.remote_url().context("get remote URL")?;
    let (owner, repo_name) =
        github::parse_github_owner_repo(&remote_url).context("parse owner/repo from remote URL")?;
    let token = github::forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;
    let repo_info = provider
        .repo_info(&owner, &repo_name)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let default_branch = repo_info.default_branch.clone();

    eprintln!(
        "\n[phase-1] Publishing baseline `{}` at origin/{default_branch} tip",
        baseline_branch
    );
    let _baseline_sha =
        github::push_baseline_from_origin(repo, &baseline_branch, &default_branch, cfg.full_repo)
            .context("push baseline from origin")?;

    // Optional legacy: commit everything to default branch (broad `git add -u`).
    if cfg.commit_main {
        eprintln!("\n[legacy] --commit-main: staging all manifest paths + push {default_branch}");
        let _ = // vox-arch-check: allow git-exec
        Command::new("git")
            .args(["rm", "--cached", "-r", "--ignore-unmatch", "docs/book/"])
            .current_dir(repo)
            .status();
        let add_all = // vox-arch-check: allow git-exec
        Command::new("git")
            .args(["add", "-u"])
            .current_dir(repo)
            .status()
            .context("git add -u")?;
        if !add_all.success() {
            anyhow::bail!("git add -u failed");
        }
        let all_new: Vec<_> = sem_manifest
            .chunks
            .iter()
            .flat_map(|c| c.files.iter().cloned())
            .collect();
        for batch in all_new.chunks(200) {
            let mut args = vec!["add", "--"];
            args.extend(batch.iter().map(|s| s.as_str()));
            let _ = // vox-arch-check: allow git-exec
        Command::new("git").args(&args).current_dir(repo).status();
        }
        let timestamp = chrono::Utc::now().format("%Y-%m-%d");
        let commit_msg = format!(
            "feat: batch-commit {timestamp} — {} files across {} semantic groups\n\n\
             Committed via `vox review coderabbit semantic-submit --commit-main`.",
            sem_manifest.total_files,
            sem_manifest.chunks.len(),
        );
        let status = // vox-arch-check: allow git-exec
        Command::new("git")
            .args(["commit", "-m", &commit_msg])
            .current_dir(repo)
            .status()
            .context("git commit")?;
        if !status.success() {
            anyhow::bail!("git commit failed — nothing staged or commit error");
        }
        let push = // vox-arch-check: allow git-exec
        Command::new("git")
            .args(["push", "origin", &default_branch])
            .current_dir(repo)
            .status()
            .context("git push origin default branch")?;
        if !push.success() {
            anyhow::bail!("git push origin {default_branch} failed");
        }
        eprintln!("[legacy] ✓ pushed {default_branch}");
    }

    // ── 5. Resume state ─────────────────────────────────────────────────────
    let source_tree = repo.to_path_buf();
    let mut run_state = cr_run_state::CoderabbitRunState {
        baseline_branch: baseline_branch.clone(),
        default_branch: default_branch.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        chunks: sem_manifest
            .chunks
            .iter()
            .map(|c| cr_run_state::ChunkRunRecord {
                name: c.name.clone(),
                branch: format!("cr/review-{}", c.name),
                pr_number: None,
                status: "pending".to_string(),
                error: None,
            })
            .collect(),
    };

    if cfg.resume && !cfg.force_chunks {
        if let Some(prev) = cr_run_state::CoderabbitRunState::load(repo)? {
            if prev.baseline_branch == baseline_branch {
                for rec in &mut run_state.chunks {
                    if let Some(o) = prev
                        .chunks
                        .iter()
                        .find(|p| p.name == rec.name && p.status == "completed")
                    {
                        *rec = o.clone();
                    }
                }
            }
        }
    }
    run_state.save(repo).context("write initial run-state")?;

    // ── 6. Isolated worktree PRs (independent base = baseline) ─────────────

    eprintln!(
        "\n[phase-2] Creating {} review PR(s) via worktrees…",
        sem_manifest.chunks.len()
    );
    let delay = if cfg.delay_secs > 0 {
        cfg.delay_secs
    } else {
        cfg.tier.min_delay_between_prs_secs()
    };

    for (i, chunk) in sem_manifest.chunks.iter().enumerate() {
        if !cfg.force_chunks && run_state.chunks[i].status == "completed" {
            eprintln!(
                "[resume] skip completed chunk {} (PR #{})",
                chunk.name,
                run_state.chunks[i].pr_number.unwrap_or(0)
            );
            continue;
        }

        let cr_branch = format!("cr/review-{}", chunk.name);
        eprintln!(
            "\n[{}/{}] {} ({} files) → branch: {}",
            i + 1,
            sem_manifest.chunks.len(),
            chunk.name,
            chunk.files.len(),
            cr_branch
        );

        let res = github::create_chunk_pr_via_worktree(
            repo,
            &source_tree,
            &default_branch,
            &baseline_branch,
            &cr_branch,
            &chunk.files,
            true,
        )
        .await;

        match res {
            Ok(pr_num) => {
                run_state.chunks[i].status = "completed".to_string();
                run_state.chunks[i].pr_number = Some(pr_num);
                run_state.chunks[i].error = None;
            }
            Err(e) => {
                run_state.chunks[i].status = "failed".to_string();
                run_state.chunks[i].error = Some(format!("{e:#}"));
                run_state.save(repo)?;
                return Err(e).with_context(|| format!("chunk {}", chunk.name));
            }
        }
        run_state.save(repo)?;

        if i + 1 < sem_manifest.chunks.len() && delay > 0 {
            eprintln!("[rate-limit] waiting {delay}s before next PR…");
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }

    eprintln!("\n══════════════════════════════════════════════");
    let n_done = run_state
        .chunks
        .iter()
        .filter(|c| c.pr_number.is_some())
        .count();
    eprintln!("  DONE — {n_done} PR(s) with numbers recorded");
    for (chunk, rec) in sem_manifest.chunks.iter().zip(&run_state.chunks) {
        if let Some(pr_num) = rec.pr_number {
            eprintln!("  PR #{pr_num:>6}  {}", chunk.name);
        } else if rec.status == "failed" {
            eprintln!("  (failed)         {}", chunk.name);
        }
    }
    eprintln!("══════════════════════════════════════════════");
    eprintln!("Next: monitor PRs, then ingest findings:");
    eprintln!("  vox review coderabbit ingest <pr_number>");
    eprintln!("  vox review coderabbit tasks <pr_number> --format markdown");

    Ok(())
}

async fn persist_semantic_lineage_run(
    repo: &Path,
    baseline_branch: &str,
    sem_manifest: &super::types::SemanticManifest,
) -> Result<()> {
    let bridge = GitBridge::open(repo).context("open git repo for lineage")?;
    let remote_url = bridge.remote_url().context("get remote URL for lineage")?;
    let (owner, repo_name) =
        github::parse_github_owner_repo(&remote_url).context("parse owner/repo for lineage")?;
    let repository_id = format!("{owner}/{repo_name}");
    let metadata = serde_json::json!({
        "baseline_branch": baseline_branch,
        "generated_at": sem_manifest.generated_at,
        "total_files": sem_manifest.total_files,
        "coverage": sem_manifest.coverage,
        "chunk_count": sem_manifest.chunks.len(),
        "chunks": sem_manifest.chunks.iter().map(|c| {
            serde_json::json!({
                "name": c.name,
                "file_count": c.files.len(),
            })
        }).collect::<Vec<_>>(),
    })
    .to_string();

    let db = VoxDb::connect_default().await?;
    let _ = db
        .insert_external_review_run(ExternalReviewRunParams {
            provider: "coderabbit",
            repository_id: &repository_id,
            owner: &owner,
            repo: &repo_name,
            pr_number: 0,
            commit_sha: None,
            trigger_kind: "semantic_submit_plan",
            idempotency_key: Some(baseline_branch),
            item_count: sem_manifest.coverage.included_files as i64,
            metadata_json: Some(&metadata),
        })
        .await?;
    Ok(())
}
