//! Git-based file collection for semantic planning.

use std::path::Path;

use anyhow::{Context, Result};

/// Gather all files that differ from HEAD *or* are untracked new files.
///
/// - Modified/deleted tracked files come from `git diff HEAD --name-only`
/// - New files (untracked) come from `git status --short` (`??` prefix)
///
/// Returns deduplicated, forward-slash paths relative to repo root.
pub async fn collect_changed_files(repo: &Path) -> Result<Vec<String>> {
    // Resolve relative paths (e.g. ".") to absolute without using canonicalize(),
    // which produces UNC-style paths on Windows that break some git invocations.
    let cwd = std::env::current_dir().context("get current directory")?;
    let normalized: std::path::PathBuf = if repo.is_absolute() {
        repo.components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect()
    } else {
        cwd.join(repo)
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect()
    };
    let repo = normalized.as_path();

    // 1. Tracked modifications (modified/deleted tracked files)
    // -c core.autocrlf=false suppresses CRLF warnings that fill the stderr pipe and deadlock .output()
    let diff_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "diff",
        "HEAD",
        "--name-only",
        "--diff-filter=ACDMRT",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null()) // ← prevent CRLF warning deadlock
    .output()
    .await
    .context("git diff HEAD --name-only")?;
    let diff_str = String::from_utf8_lossy(&diff_out.stdout);

    // 2. Staged (already added with git add)
    let staged_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "diff",
        "--cached",
        "--name-only",
        "--diff-filter=ACDMRT",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git diff --cached --name-only")?;
    let staged_str = String::from_utf8_lossy(&staged_out.stdout);

    // 3. Untracked new files/directories
    let status_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "status",
        "--short",
        "--porcelain",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git status --short")?;
    let status_str = String::from_utf8_lossy(&status_out.stdout);

    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Tracked changes
    for line in diff_str.lines().chain(staged_str.lines()) {
        let p = line.trim().replace('\\', "/");
        if !p.is_empty() {
            files.insert(p);
        }
    }

    // Untracked new files/dirs
    for line in status_str.lines() {
        if !line.starts_with("??") {
            continue;
        }
        let raw = line[3..].trim().replace('\\', "/");
        // Directories end with '/' — include the prefix, actual staging will be recursive
        let p = raw.trim_end_matches('/').to_string();
        if !p.is_empty() {
            files.insert(p);
        }
    }

    let mut sorted: Vec<String> = files.into_iter().collect();
    sorted.sort();
    Ok(sorted)
}

/// Gather **every tracked file** in the repository plus untracked new files.
///
/// Use this for a full-codebase review (`--full-repo`) regardless of commit history.
/// Unlike [`collect_changed_files`] this uses `git ls-files` so even files with no
/// working-tree modifications are included.
pub async fn collect_all_files(repo: &Path) -> Result<Vec<String>> {
    let cwd = std::env::current_dir().context("get current directory")?;
    let normalized: std::path::PathBuf = if repo.is_absolute() {
        repo.components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect()
    } else {
        cwd.join(repo)
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect()
    };
    let repo = normalized.as_path();

    // 1. All tracked files.
    let ls_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args(["-c", "core.autocrlf=false", "ls-files"])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git ls-files")?;
    let ls_str = String::from_utf8_lossy(&ls_out.stdout);

    // 2. Untracked new files (same as collect_changed_files).
    let status_out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "status",
        "--short",
        "--porcelain",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git status --short")?;
    let status_str = String::from_utf8_lossy(&status_out.stdout);

    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in ls_str.lines() {
        let p = line.trim().replace('\\', "/");
        if !p.is_empty() {
            files.insert(p);
        }
    }

    for line in status_str.lines() {
        if !line.starts_with("??") {
            continue;
        }
        let raw = line[3..].trim().replace('\\', "/");
        let p = raw.trim_end_matches('/').to_string();
        if !p.is_empty() {
            files.insert(p);
        }
    }

    let mut sorted: Vec<String> = files.into_iter().collect();
    sorted.sort();
    Ok(sorted)
}

/// Files added/copied/modified/renamed since `since` (any git date expr, e.g.
/// "2026-04-01" or "2 weeks ago"). Deletions excluded. Sorted, deduped.
pub async fn collect_files_modified_since(repo: &Path, since: &str) -> Result<Vec<String>> {
    let out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "log",
        &format!("--since={since}"),
        "--name-only",
        "--diff-filter=ACMR",
        "--pretty=format:",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git log --since --name-only")?;
    anyhow::ensure!(out.status.success(), "git log --since failed");
    let mut seen = std::collections::BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() {
            seen.insert(super::super::path_policy::normalize_repo_rel_path(p));
        }
    }
    Ok(seen.into_iter().collect())
}

/// Sum of (insertions + deletions) per file since `since` (churn signal).
pub async fn churn_since(
    repo: &Path,
    since: &str,
) -> Result<std::collections::HashMap<String, u64>> {
    let out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "log",
        &format!("--since={since}"),
        "--numstat",
        "--pretty=format:",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git log --since --numstat")?;
    let mut m = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(3, '\t');
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or("");
        let p = parts.next().unwrap_or("");
        if p.is_empty() {
            continue;
        }
        let w = a.parse::<u64>().unwrap_or(0) + b.parse::<u64>().unwrap_or(0);
        *m.entry(super::super::path_policy::normalize_repo_rel_path(p))
            .or_insert(0) += w;
    }
    Ok(m)
}

/// Count of commits touching each file since `since` (recency proxy).
pub async fn recency_since(
    repo: &Path,
    since: &str,
) -> Result<std::collections::HashMap<String, f64>> {
    let out = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
    .args([
        "-c",
        "core.autocrlf=false",
        "log",
        &format!("--since={since}"),
        "--name-only",
        "--pretty=format:",
    ])
    .current_dir(repo)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .context("git log --since --name-only")?;
    let mut m = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() {
            *m.entry(super::super::path_policy::normalize_repo_rel_path(p))
                .or_insert(0.0) += 1.0;
        }
    }
    Ok(m)
}

#[cfg(test)]
mod since_tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
    }

    /// `--since` filters by COMMITTER date, so the old commit must back-date both.
    fn git_old_commit(dir: &Path, msg: &str) {
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["commit", "-qm", msg])
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00")
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn modified_since_lists_recent_not_old() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["config", "user.email", "t@t"]);
        git(p, &["config", "user.name", "t"]);
        std::fs::write(p.join("old.rs"), "x").unwrap();
        git(p, &["add", "-A"]);
        git_old_commit(p, "old");
        std::fs::write(p.join("new.rs"), "y\nz").unwrap();
        git(p, &["add", "-A"]);
        git(p, &["commit", "-qm", "new"]);

        let files = collect_files_modified_since(p, "1 day ago").await.unwrap();
        assert!(files.iter().any(|f| f.ends_with("new.rs")));
        assert!(!files.iter().any(|f| f.ends_with("old.rs")));

        let churn = churn_since(p, "1 day ago").await.unwrap();
        assert_eq!(churn.get("new.rs").copied().unwrap_or(0), 2);
        let rec = recency_since(p, "1 day ago").await.unwrap();
        assert!(rec.get("new.rs").copied().unwrap_or(0.0) >= 1.0);
    }
}
