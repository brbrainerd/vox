//! Local repository upgrade lane for `vox upgrade --source repo`.
//!
//! Check-only by default; `--apply` runs `git fetch`, fast-forward or explicit `--ref`, then
//! `cargo install --locked --path` the primary CLI crate ([`crate::utils::install_policy::SOURCE_INSTALL_CLI_REL_PATH`]).
//! Roll back `HEAD` on install failure.

use crate::cli_args::UpgradeToolchainArgs;
use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::utils::install_policy::{CARGO_INSTALL_CLI_FROM_SOURCE, SOURCE_INSTALL_CLI_REL_PATH};
use vox_repository::resolve_repo_root_for_ci;

const ROLLBACK_REL: &str = ".vox/toolchain-upgrade-rollback.json";
const ROLLBACK_SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct RollbackState {
    schema: u32,
    head_sha: String,
    /// `refs/heads/main` when on a branch; omitted when `HEAD` was detached.
    head_symbolic_ref: Option<String>,
}

pub fn run_repo_upgrade(args: &UpgradeToolchainArgs, json_output: bool) -> Result<()> {
    let root = resolve_repo_root(args)?;
    validate_repo_layout(&root)?;
    repo_semver_gate(args)?;

    if !args.apply {
        return emit_repo_check_only(&root, args, json_output);
    }

    ensure_git_available()?;
    let clean = worktree_clean(&root)?;
    if !clean && !args.allow_dirty {
        bail!(
            "repo upgrade refused: worktree is not clean (use `--allow-dirty` after review, or commit/stash)"
        );
    }

    let (remote_name, branch_name) = resolve_fetch_target(&root, args)?;
    let head_before = git_trimmed(&root, &["rev-parse", "HEAD"])?;
    let sym_before = git_symbolic_ref_head(&root).ok();

    write_rollback_marker(&root, &head_before, sym_before.as_deref())?;

    let apply_res = (|| -> Result<()> {
        run_git_fetch(&root, args, &remote_name, &branch_name)?;
        if let Some(ref gref) = args.git_ref {
            checkout_ref(&root, gref.trim())?;
        } else {
            fast_forward_current_branch(&root, &remote_name, &branch_name)?;
        }
        cargo_install_locked_cli(&root)?;
        Ok(())
    })();

    if let Err(e) = apply_res {
        if let Err(rb_err) = rollback_git(&root) {
            bail!(
                "repo upgrade failed: {e:#}\n\
                 rollback also failed: {rb_err:#}\n\
                 leave `.vox/toolchain-upgrade-rollback.json` for inspection and reset manually if needed."
            );
        }
        let _ = remove_rollback_marker(&root);
        bail!("repo upgrade failed: {e:#} (git tree was reset to the previous HEAD)");
    }

    remove_rollback_marker(&root)?;
    emit_repo_apply_success(&root, json_output, &head_before)?;
    Ok(())
}

fn resolve_repo_root(args: &UpgradeToolchainArgs) -> Result<PathBuf> {
    if let Some(p) = &args.repo_root {
        let abs =
            std::fs::canonicalize(p).with_context(|| format!("--repo-root {}", p.display()))?;
        return Ok(abs);
    }
    Ok(resolve_repo_root_for_ci())
}

fn validate_repo_layout(root: &Path) -> Result<()> {
    if !root.join(".git").exists() {
        bail!("not a git checkout (missing .git): {}", root.display());
    }
    let cli_manifest = root.join(SOURCE_INSTALL_CLI_REL_PATH).join("Cargo.toml");
    if !cli_manifest.is_file() {
        bail!(
            "expected a Vox workspace at {} (missing {})",
            root.display(),
            cli_manifest.display(),
        );
    }
    Ok(())
}

fn repo_semver_gate(args: &UpgradeToolchainArgs) -> Result<()> {
    let Some(gref) = args.git_ref.as_deref() else {
        return Ok(());
    };
    let gref = gref.trim();
    let verish = gref.strip_prefix('v').unwrap_or(gref);
    let Ok(target_ver) = Version::parse(verish) else {
        return Ok(());
    };
    let current_str = env!("CARGO_PKG_VERSION");
    let current_ver = Version::parse(current_str).map_err(|e| anyhow!("internal semver: {e}"))?;
    if target_ver <= current_ver {
        return Ok(());
    }
    let compatible = self_update::version::bump_is_compatible(current_str, &target_ver.to_string())
        .unwrap_or(false);
    if !compatible && !args.allow_breaking {
        bail!(
            "repo `--ref` {gref}: target semver appears incompatible with v{current_str} (use `--allow-breaking` to opt in)"
        );
    }
    Ok(())
}

fn ensure_git_available() -> Result<()> {
    let s = // vox-arch-check: allow git-exec
        Command::new("git")
        .arg("--version")
        .status()
        .context("spawn `git --version`")?;
    if !s.success() {
        bail!("`git` is not available on PATH");
    }
    Ok(())
}

fn worktree_clean(root: &Path) -> Result<bool> {
    let out = git_output(root, &["status", "--porcelain"])?;
    Ok(out.trim().is_empty())
}

fn resolve_fetch_target(root: &Path, args: &UpgradeToolchainArgs) -> Result<(String, String)> {
    if args.git_ref.is_some() {
        let remote = args.remote.clone().unwrap_or_else(|| "origin".to_string());
        let branch = args.branch.clone().unwrap_or_else(|| "HEAD".to_string());
        return Ok((remote, branch));
    }

    if let (Some(r), Some(b)) = (&args.remote, &args.branch) {
        return Ok((r.clone(), b.clone()));
    }
    if args.remote.is_some() ^ args.branch.is_some() {
        bail!(
            "`--remote` and `--branch` must be passed together when the current branch has no upstream"
        );
    }

    let upstream = git_upstream_symbolic_full(root).map_err(|_| {
        anyhow!(
            "current branch has no upstream configured; set tracking or pass `--remote` and `--branch`"
        )
    })?;
    let s = upstream.strip_prefix("refs/remotes/").unwrap_or(&upstream);
    let mut parts = s.splitn(2, '/');
    let remote = parts.next().unwrap_or("origin").to_string();
    let branch = parts
        .next()
        .ok_or_else(|| anyhow!("invalid upstream ref `{upstream}`"))?
        .to_string();
    Ok((remote, branch))
}

fn git_upstream_symbolic_full(root: &Path) -> Result<String> {
    git_trimmed(
        root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
}

fn git_symbolic_ref_head(root: &Path) -> Result<String> {
    git_trimmed(root, &["symbolic-ref", "-q", "HEAD"])
}

fn run_git_fetch(
    root: &Path,
    args: &UpgradeToolchainArgs,
    remote: &str,
    branch: &str,
) -> Result<()> {
    if args.git_ref.is_some() {
        let s = // vox-arch-check: allow git-exec
        Command::new("git")
            .current_dir(root)
            .args(["fetch", remote, "--tags"])
            .status()
            .context("git fetch --tags")?;
        if !s.success() {
            bail!("git fetch {remote} --tags failed");
        }
        let s = // vox-arch-check: allow git-exec
        Command::new("git")
            .current_dir(root)
            .args(["fetch", remote])
            .status()
            .context("git fetch")?;
        if !s.success() {
            bail!("git fetch {remote} failed");
        }
        if branch != "HEAD" {
            let s = // vox-arch-check: allow git-exec
        Command::new("git")
                .current_dir(root)
                .args(["fetch", remote, branch])
                .status()
                .context("git fetch branch")?;
            if !s.success() {
                bail!("git fetch {remote} {branch} failed");
            }
        }
        return Ok(());
    }

    let s = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(root)
        .args(["fetch", remote, branch])
        .status()
        .context("git fetch")?;
    if !s.success() {
        bail!("git fetch {remote} {branch} failed");
    }
    Ok(())
}

fn fast_forward_current_branch(root: &Path, remote: &str, branch: &str) -> Result<()> {
    let merge_ref = format!("{remote}/{branch}");
    let base = git_trimmed(root, &["merge-base", "HEAD", &merge_ref])
        .map_err(|_| anyhow!("fast-forward: could not compare HEAD to {}", merge_ref))?;
    let head = git_trimmed(root, &["rev-parse", "HEAD"])?;
    let remote_head = git_trimmed(root, &["rev-parse", &merge_ref])
        .map_err(|_| anyhow!("missing {merge_ref} after fetch (wrong remote/branch?)"))?;
    if base != head {
        bail!(
            "refused non-fast-forward: local HEAD is not an ancestor of {merge_ref} (merge/rebase locally or pass `--ref`)"
        );
    }
    if remote_head == head {
        return Ok(());
    }
    let s = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(root)
        .args(["merge", "--ff-only", &merge_ref])
        .status()
        .context("git merge --ff-only")?;
    if !s.success() {
        bail!("git merge --ff-only {merge_ref} failed");
    }
    Ok(())
}

fn checkout_ref(root: &Path, gref: &str) -> Result<()> {
    let s = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(root)
        .args(["checkout", gref])
        .status()
        .context("git checkout")?;
    if !s.success() {
        bail!("git checkout {gref} failed (did you fetch tags?)");
    }
    Ok(())
}

fn cargo_install_locked_cli(root: &Path) -> Result<()> {
    let cargo_bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let s = Command::new(&cargo_bin)
        .current_dir(root)
        .args(CARGO_INSTALL_CLI_FROM_SOURCE)
        .status()
        .with_context(|| {
            format!(
                "run {} {}",
                cargo_bin,
                CARGO_INSTALL_CLI_FROM_SOURCE.join(" ")
            )
        })?;
    if !s.success() {
        bail!("`cargo {}` failed", CARGO_INSTALL_CLI_FROM_SOURCE.join(" "));
    }
    Ok(())
}

fn rollback_marker_path(root: &Path) -> PathBuf {
    root.join(ROLLBACK_REL)
}

fn write_rollback_marker(
    root: &Path,
    head_sha: &str,
    head_symbolic_ref: Option<&str>,
) -> Result<()> {
    let dir = root.join(".vox");
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let st = RollbackState {
        schema: ROLLBACK_SCHEMA,
        head_sha: head_sha.to_string(),
        head_symbolic_ref: head_symbolic_ref.map(String::from),
    };
    let json = serde_json::to_string_pretty(&st)?;
    std::fs::write(rollback_marker_path(root), json).with_context(|| "write rollback marker")?;
    Ok(())
}

fn remove_rollback_marker(root: &Path) -> Result<()> {
    let p = rollback_marker_path(root);
    if p.is_file() {
        std::fs::remove_file(&p).with_context(|| format!("remove {}", p.display()))?;
    }
    Ok(())
}

fn read_rollback_marker(root: &Path) -> Result<RollbackState> {
    let p = rollback_marker_path(root);
    let raw = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
    let st: RollbackState = serde_json::from_str(&raw).context("parse rollback marker")?;
    if st.schema != ROLLBACK_SCHEMA {
        bail!("rollback marker schema mismatch");
    }
    Ok(st)
}

fn rollback_git(root: &Path) -> Result<()> {
    let st = read_rollback_marker(root)?;
    if let Some(sym) = &st.head_symbolic_ref {
        let branch = sym.strip_prefix("refs/heads/").unwrap_or(sym.as_str());
        let s = // vox-arch-check: allow git-exec
        Command::new("git")
            .current_dir(root)
            .args(["checkout", branch])
            .status()
            .context("rollback: git checkout branch")?;
        if !s.success() {
            bail!("rollback: checkout {branch} failed");
        }
    }
    let s = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(root)
        .args(["reset", "--hard", &st.head_sha])
        .status()
        .context("rollback: git reset --hard")?;
    if !s.success() {
        bail!("rollback: git reset --hard failed");
    }
    Ok(())
}

fn emit_repo_check_only(root: &Path, args: &UpgradeToolchainArgs, json_output: bool) -> Result<()> {
    let head = git_trimmed(root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let full_head = git_trimmed(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let branch = git_trimmed(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let upstream = git_upstream_symbolic_full(root).ok();
    let dirty = !worktree_clean(root).unwrap_or(true);
    let (remote, br) = resolve_fetch_target(root, args)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "toolchain_upgrade": {
                    "lane": "repo",
                    "status": "check_only",
                    "repo_root": root.to_string_lossy(),
                    "head": full_head,
                    "head_short": head,
                    "branch": branch,
                    "upstream": upstream,
                    "worktree_dirty": dirty,
                    "fetch_remote": remote,
                    "fetch_branch": br,
                    "git_ref": args.git_ref,
                    "manifest_graph_touched": false,
                    "hint": format!(
                        "Re-run with `--apply` to fetch/update and `cargo install --locked --path {}` (rolls back on install failure).",
                        SOURCE_INSTALL_CLI_REL_PATH
                    ),
                }
            })
        );
    } else {
        println!("Repo upgrade (check-only): {}", root.display());
        println!("  HEAD: {head} ({full_head})");
        println!("  branch: {branch}");
        if let Some(u) = &upstream {
            println!("  upstream: {u}");
        }
        println!("  worktree: {}", if dirty { "dirty" } else { "clean" });
        if let Some(r) = &args.git_ref {
            println!("  would checkout: {r}");
        } else {
            println!("  would fast-forward: {remote}/{br}");
        }
        println!("Run: vox upgrade --apply --source repo [--repo-root …] [--ref …]");
    }
    Ok(())
}

fn emit_repo_apply_success(root: &Path, json_output: bool, prev_head: &str) -> Result<()> {
    let new_head = git_trimmed(root, &["rev-parse", "HEAD"])?;
    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "toolchain_upgrade": {
                    "lane": "repo",
                    "status": "installed_from_source",
                    "repo_root": root.to_string_lossy(),
                    "previous_head": prev_head,
                    "head": new_head,
                    "manifest_graph_touched": false,
                }
            })
        );
    } else {
        println!("Repo updated and `cargo install --locked` completed.");
        println!("  root: {}", root.display());
        println!("  HEAD was {prev_head}, now {new_head}");
    }
    Ok(())
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    let out = // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("git {} failed: {err}", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn git_trimmed(root: &Path, args: &[&str]) -> Result<String> {
    Ok(git_output(root, args)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_args::{UpgradeLane, UpgradeToolchainArgs};

    fn args_repo_ref(semver_ref: &str, allow_breaking: bool) -> UpgradeToolchainArgs {
        UpgradeToolchainArgs {
            lane: UpgradeLane::Repo,
            repo_root: None,
            git_ref: Some(semver_ref.to_string()),
            remote: None,
            branch: None,
            allow_dirty: false,
            apply: false,
            channel: "stable".into(),
            version: None,
            provider: None,
            repo: None,
            base_url: None,
            gitlab_host: None,
            github_api_url: None,
            allow_breaking,
            allow_prerelease: false,
        }
    }

    #[test]
    fn semver_gate_blocks_huge_major_without_allow_breaking() {
        let a = args_repo_ref("v999.0.0", false);
        let err = repo_semver_gate(&a).unwrap_err().to_string();
        assert!(err.contains("allow-breaking"), "unexpected message: {err}");
    }

    #[test]
    fn semver_gate_allows_with_allow_breaking() {
        let a = args_repo_ref("v999.0.0", true);
        repo_semver_gate(&a).expect("allow-breaking");
    }

    #[test]
    fn semver_gate_ignores_non_version_refs() {
        let a = args_repo_ref("main", false);
        repo_semver_gate(&a).expect("branch name");
    }
}
