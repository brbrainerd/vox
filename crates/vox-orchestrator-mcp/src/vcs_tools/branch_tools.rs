//! Branch creation tool for the Vox MCP server.
//!
//! Provides [`branch_create`] which validates the branch name, checks for
//! conflicts, runs `git branch`, and returns a [`WorkingTreeWrite`] capability.

use std::path::Path;

use vox_orchestrator_types::vcs_capability::mint_working_tree_write;
use vox_orchestrator_types::{BranchName, BranchNameError, WorkingTreeWrite, WorkspaceId};

use crate::git_exec::{GitExec, GitExecError};

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BranchError {
    #[error("invalid branch name: {0}")]
    InvalidName(#[from] BranchNameError),
    #[error("branch already exists: {0}")]
    AlreadyExists(String),
    #[error("git failed: {0}")]
    GitFailed(#[from] GitExecError),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new git branch and return a [`WorkingTreeWrite`] capability for it.
///
/// - `workspace_id`: opaque workspace token forwarded into the capability.
/// - `branch_name`: will be validated via [`BranchName::parse`].
/// - `start_point`: if `Some`, passed as the starting commit/branch to
///   `git branch`; if `None`, the current `HEAD` is used.
///
/// Returns `Err(BranchError::AlreadyExists)` when the branch is already
/// present in the local repository.
pub async fn branch_create(
    cwd: &Path,
    workspace_id: u64,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<WorkingTreeWrite, BranchError> {
    // 1. Validate name first — cheap, no I/O.
    let parsed_branch = BranchName::parse(branch_name)?;

    let git = GitExec::new(cwd);

    // 2. Check existence.
    let list_out = git.run(&["branch", "--list", branch_name]).await?;
    if parse_branch_exists(&list_out.stdout) {
        return Err(BranchError::AlreadyExists(branch_name.to_string()));
    }

    // 3. Create the branch.
    match start_point {
        Some(sp) => git.run(&["branch", branch_name, sp]).await?,
        None => git.run(&["branch", branch_name]).await?,
    };

    // 4. Return capability.
    tracing::info!(
        target: "vox.vcs.branch",
        branch = branch_name,
        workspace_id = workspace_id,
        "branch created"
    );

    Ok(mint_working_tree_write(
        WorkspaceId(workspace_id),
        parsed_branch,
    ))
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn parse_branch_exists(git_output: &str) -> bool {
    !git_output.trim().is_empty()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_exists_when_output_nonempty() {
        assert!(parse_branch_exists("  main\n"));
    }

    #[test]
    fn branch_exists_false_for_empty() {
        assert!(!parse_branch_exists(""));
    }

    #[test]
    fn branch_exists_false_for_whitespace_only() {
        assert!(!parse_branch_exists("   \n"));
    }

    #[test]
    fn branch_name_invalid_propagates() {
        // Space is not allowed in branch names.
        assert!(matches!(
            BranchName::parse("bad name with space"),
            Err(BranchNameError::IllegalChar(' '))
        ));
    }
}
