//! Isolated worktrees and semantic chunk PR creation.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use vox_forge::github::GitHubProvider;
use vox_forge::{GitForgeProvider, NewChangeRequest};

use super::super::super::path_policy;
use super::super::api::{forge_token, owner_repo_from_path};
use super::super::comments::trigger_coderabbit;

pub fn worktree_dir(repo: &Path, review_branch: &str) -> PathBuf {
    let safe = review_branch.replace(['/', '\\'], "__");
    repo.join(".coderabbit").join("worktrees").join(safe)
}

/// Remove a registered git worktree and delete its directory; retry once on failure.
pub async fn git_worktree_remove(repo: &Path, path: &Path) -> Result<()> {
    for attempt in 1..=2u8 {
        let st = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(path)
            .current_dir(repo)
            .status()
            .await
            .with_context(|| format!("git worktree remove {}", path.display()))?;
        if st.success() {
            break;
        }
        if attempt == 2 {
            anyhow::bail!(
                "git worktree remove failed for {} after retry (exit {:?}). \
                 Try closing programs using that folder, then `git worktree remove --force {}` from the repo root.",
                path.display(),
                st.code(),
                path.display()
            );
        }
        eprintln!(
            "[warn] git worktree remove attempt {attempt} failed for {}; retrying…",
            path.display()
        );
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
    if path.exists() {
        tokio::fs::remove_dir_all(path)
            .await
            .with_context(|| format!("remove worktree dir {}", path.display()))?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    use walkdir::WalkDir;
    for entry in WalkDir::new(src)
        .follow_links(false)
        .max_depth(20) // safety: prevent infinite recursion if a dir somehow nests back into itself
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if entry.file_type().is_symlink() {
            continue;
        }
        let rel = p.strip_prefix(src).unwrap_or(p);
        let rel_str = rel.to_string_lossy();
        if path_policy::should_skip_overlay_copy_path(&rel_str) {
            continue;
        }
        // Also skip any path component named `.coderabbit`.  When `src` is already
        // inside `.coderabbit/`, rel paths like `worktrees/…` wouldn't match the
        // `.coderabbit/` prefix check above — this per-component guard catches them.
        if rel
            .components()
            .any(|c| c.as_os_str().to_string_lossy().as_ref() == ".coderabbit")
        {
            continue;
        }
        let target = dst.join(rel);
        if p.is_dir() {
            fs::create_dir_all(&target)?;
        } else if p.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(p, &target)
                .with_context(|| format!("copy {} -> {}", p.display(), target.display()))?;
        }
    }
    Ok(())
}

/// Create a review PR from an isolated worktree: baseline → worktree, overlay files from `source_tree`, push, open PR.
///
/// Does not modify the main working tree other than registering a git worktree under `.coderabbit/worktrees/`.
/// This function is **fully idempotent** — safe to call after a prior crash or partial run.
pub async fn create_chunk_pr_via_worktree(
    repo_root: &Path,
    source_tree: &Path,
    _default_branch: &str,
    baseline_branch: &str,
    review_branch: &str,
    files: &[String],
    full_review: bool,
) -> Result<u64> {
    let (owner, repo_name) = owner_repo_from_path(repo_root)?;
    let token = forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;

    // ── 1. Prune stale git worktree metadata (survives directory deletion) ────
    let _ = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo_root)
        .status()
        .await;

    let wt = worktree_dir(repo_root, review_branch);

    // ── 2. Remove pre-existing worktree directory (and its git ref) ───────────
    if wt.exists() {
        // Try graceful removal first (updates git metadata)
        let _ = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&wt)
            .current_dir(repo_root)
            .status()
            .await;
        // Forcibly wipe directory even if git command failed
        if wt.exists() {
            let _ = tokio::fs::remove_dir_all(&wt).await;
        }
    }

    // ── 3. Force-delete stale local branch (may be "checked out" in deleted wt) ─
    let _ = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["branch", "-D", review_branch])
        .current_dir(repo_root)
        .output()
        .await;

    // Re-prune after forced dir deletion so git doesn't see stale refs
    let _ = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo_root)
        .status()
        .await;

    // ── 4. Ensure worktrees parent directory exists ───────────────────────────
    if let Some(parent) = wt.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    // ── 5. Fetch baseline branch ──────────────────────────────────────────────
    let fetch_baseline = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["fetch", "origin", baseline_branch])
        .current_dir(repo_root)
        .status()
        .await
        .context("git fetch baseline branch")?;
    if !fetch_baseline.success() {
        anyhow::bail!("git fetch origin {baseline_branch} failed");
    }

    // ── 6. Create fresh worktree ──────────────────────────────────────────────
    let wt_str = wt.to_str().with_context(|| "worktree path utf-8")?;
    let status = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args([
            "worktree",
            "add",
            "-B",
            review_branch,
            wt_str,
            &format!("origin/{baseline_branch}"),
        ])
        .current_dir(repo_root)
        .status()
        .await
        .context("git worktree add")?;
    if !status.success() {
        anyhow::bail!("git worktree add failed for {review_branch}");
    }

    // ── 7. Overlay files from source tree ────────────────────────────────────
    for rel in files {
        let rel_norm = rel.replace('\\', "/");
        if path_policy::is_coderabbit_local_tool_path(&rel_norm) {
            continue;
        }
        let src = source_tree.join(&rel_norm);
        let dst = wt.join(&rel_norm);
        if src.is_file() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst).with_context(|| format!("overlay {}", rel_norm))?;
        } else if src.is_dir() {
            if dst.exists() {
                fs::remove_dir_all(&dst)?;
            }
            copy_dir_recursive(&src, &dst)?;
        } else if dst.exists() {
            if dst.is_dir() {
                fs::remove_dir_all(&dst)?;
            } else {
                fs::remove_file(&dst)?;
            }
        }
    }

    // ── 8. Stage & commit ─────────────────────────────────────────────────────
    if !files.is_empty() {
        for batch in files.chunks(80) {
            let mut args: Vec<&str> = vec!["add", "--"];
            let batch_str: Vec<String> = batch.iter().map(|s| s.replace('\\', "/")).collect();
            let refs: Vec<&str> = batch_str.iter().map(|s| s.as_str()).collect();
            args.extend(refs);
            let st = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
                .args(&args)
                .current_dir(&wt)
                .status()
                .await
                .context("git add")?;
            if !st.success() {
                eprintln!("[warn] git add batch had failures (paths may be deleted)");
            }
        }
    }

    let commit_msg = format!("feat: CodeRabbit chunk {review_branch}");
    let cst = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(&wt)
        .status()
        .await
        .context("git commit")?;
    if !cst.success() {
        anyhow::bail!("git commit failed in worktree (nothing staged?)");
    }

    // ── 9. Force push (safe — these are ephemeral review branches) ────────────
    let push_st = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .args(["push", "-uf", "origin", review_branch])
        .current_dir(&wt)
        .status()
        .await
        .context("git push from worktree")?;
    if !push_st.success() {
        anyhow::bail!("git push failed for {review_branch}");
    }

    // ── 10. Open PR ───────────────────────────────────────────────────────────
    let pr_title = format!("CodeRabbit review: {review_branch}");
    let body = format!(
        "Automated semantic PR for CodeRabbit.\n\n@coderabbitai {}",
        if full_review { "full review" } else { "review" }
    );
    let cr = provider
        .create_change_request(
            &owner,
            &repo_name,
            NewChangeRequest {
                title: &pr_title,
                body: body.as_str(),
                source_branch: review_branch,
                target_branch: baseline_branch,
                draft: false,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let _ = trigger_coderabbit(&token, &owner, &repo_name, cr.number, full_review).await;
    eprintln!(
        "PR #{} opened: {} (base={})",
        cr.number, cr.web_url, baseline_branch
    );
    Ok(cr.number)
}
