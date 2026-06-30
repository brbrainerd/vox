//! GitHub adapter: PR lifecycle via `vox-forge`, local git via CLI (worktrees).

mod api;
mod comments;
mod reviews;

pub(crate) use api::{forge_token, parse_github_owner_repo};
pub use comments::{trigger_coderabbit, wait_for_review};
pub use reviews::{
    create_chunk_pr_via_worktree, create_orphan_baseline, create_stack_chunk_pr,
    git_worktree_remove, push_baseline_from_origin, submit, worktree_dir,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::worktree_dir;

    #[test]
    fn worktree_dir_sanitizes_branch_slashes() {
        let repo = Path::new("/repo");
        let w = worktree_dir(repo, "cr/review-02_foo");
        let s = w.to_string_lossy();
        assert!(s.contains("cr__review-02_foo"), "got {s}");
    }
}
