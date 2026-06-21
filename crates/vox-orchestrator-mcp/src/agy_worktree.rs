//! Isolated git worktree that jails an `agy` delegation (our real safety
//! boundary under --dangerously-skip-permissions), plus diff capture.

use crate::agy_exec::sanitize_slug;
use crate::git_exec::{GitExec, GitExecError};
use std::path::{Path, PathBuf};

pub fn delegation_worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".vox").join("agy-worktrees").join(slug)
}

pub fn count_changed(tracked_diff: &str, untracked_list: &str) -> usize {
    let tracked = tracked_diff.matches("diff --git").count();
    let untracked = untracked_list
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    tracked + untracked
}

pub struct DelegationWorktree {
    pub path: PathBuf,
    pub branch: String,
    git: GitExec,
}

impl DelegationWorktree {
    /// Create a fresh worktree+branch off HEAD. `slug` MUST be unique per call
    /// (callers derive it from a monotonic counter; see agy_tools).
    pub async fn create(repo_root: &Path, slug: &str) -> Result<Self, GitExecError> {
        let slug = sanitize_slug(slug);
        let path = delegation_worktree_path(repo_root, &slug);
        let branch = format!("agy/{slug}");
        let path_s = path.to_string_lossy().to_string();
        GitExec::new(repo_root)
            .run(&["worktree", "add", "-b", &branch, &path_s, "HEAD"])
            .await?;
        Ok(Self {
            path: path.clone(),
            branch,
            git: GitExec::new(path),
        })
    }

    /// (unified-diff text, changed-file count). Includes tracked + untracked.
    pub async fn capture(&self) -> Result<(String, usize), GitExecError> {
        let tracked = self.git.run(&["diff", "HEAD"]).await?;
        let untracked = self
            .git
            .run(&["ls-files", "--others", "--exclude-standard"])
            .await?;
        let n = count_changed(&tracked.stdout, &untracked.stdout);
        let text = format!(
            "# tracked\n{}\n# new files\n{}",
            tracked.stdout, untracked.stdout
        );
        Ok((text, n))
    }

    pub async fn cleanup(&self, repo_root: &Path) -> Result<(), GitExecError> {
        let path_s = self.path.to_string_lossy().to_string();
        GitExec::new(repo_root)
            .run(&["worktree", "remove", "--force", &path_s])
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_path_is_jailed_under_dot_vox() {
        let p = delegation_worktree_path(std::path::Path::new("/repo"), "d-123-foo");
        assert!(p.starts_with("/repo/.vox/agy-worktrees"));
        assert!(p.to_string_lossy().contains("d-123-foo"));
    }

    #[test]
    fn counts_changed_files_from_diff_parts() {
        let tracked = "diff --git a/x b/x\n...\ndiff --git a/y b/y\n...";
        let untracked = "newfile.txt\n";
        assert_eq!(count_changed(tracked, untracked), 3);
    }
}
