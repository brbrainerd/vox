//! Git-backed PR flows: submit, baselines, worktrees, stacked chunks.

mod baseline;
mod stack;
mod submit_branch;
mod worktree;

pub use baseline::push_baseline_from_origin;
pub use stack::{create_orphan_baseline, create_stack_chunk_pr};
pub use submit_branch::submit;
pub use worktree::{create_chunk_pr_via_worktree, git_worktree_remove, worktree_dir};
