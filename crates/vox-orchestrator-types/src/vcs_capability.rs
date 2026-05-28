//! VCS capability tokens — `WorkingTreeWrite` and `BranchCreate`.
//!
//! Holding one of these structs is meant to be evidence that an authorized
//! orchestrator code path minted it. The fields are private, so the only
//! construction path is the `mint` associated function on each struct.
//!
//! In Phase 1 the `mint` methods are `pub` but `#[doc(hidden)]`. This is
//! "soft-private": the methods are reachable from any crate that knows the
//! path, but they are absent from rustdoc and are documented as for
//! orchestrator use only. The convention is that only
//! `vox_orchestrator::authorize_*` wrappers call them.
//!
//! Phase 4 of the agentic-VCS roadmap hardens this to genuine
//! unforgeability via Vox `@vcs.*` decorators and possibly stricter
//! Rust visibility (`pub(crate)` plus a sealed mint trait re-exported
//! through a thin wrapper crate). Do NOT preemptively change the
//! visibility in Phase 1.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceId(pub u64);

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "W-{:06}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    /// Reject empty, whitespace, and any name containing characters git refuses.
    /// Matches the subset of `git check-ref-format --branch` we care about for
    /// agent-generated names: ASCII, no spaces, no `..`, `:`, `?`, `*`, `[`, `\`,
    /// `^`, `~`, no leading `/` or `-`, length 1..=255.
    ///
    /// Known gaps (not currently rejected — caller may still see a `git`
    /// error downstream): leading `.`, trailing `.`, `.lock` suffix,
    /// trailing `/`, consecutive `//`. These are unlikely from agent-style
    /// names like `agent/<slug>` and are deliberately not enforced in
    /// Phase 1 to keep the validator small. Tighten if a real callsite
    /// trips on them.
    pub fn parse(s: &str) -> Result<Self, BranchNameError> {
        if s.is_empty() || s.len() > 255 {
            return Err(BranchNameError::InvalidLength);
        }
        if s.starts_with('/') || s.starts_with('-') {
            return Err(BranchNameError::IllegalPrefix);
        }
        if s.contains("..") {
            return Err(BranchNameError::IllegalSequence);
        }
        for ch in s.chars() {
            let ok = ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.');
            if !ok {
                return Err(BranchNameError::IllegalChar(ch));
            }
        }
        Ok(BranchName(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BranchNameError {
    #[error("branch name length must be 1..=255")]
    InvalidLength,
    #[error("branch name cannot start with '/' or '-'")]
    IllegalPrefix,
    #[error("branch name cannot contain '..'")]
    IllegalSequence,
    #[error("branch name contains illegal character {0:?}")]
    IllegalChar(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteId(pub u32);

/// Capability: holder may stage and commit hunks against `branch` of `workspace`.
/// Constructed only via `vox-orchestrator-cap-mint::mint_working_tree_write`.
#[derive(Debug, Clone)]
pub struct WorkingTreeWrite {
    workspace: WorkspaceId,
    branch: BranchName,
}

impl WorkingTreeWrite {
    /// Crate-internal constructor. External callers must use `vox-orchestrator-cap-mint`.
    /// Currently only exercised by in-crate tests; the runtime path goes through
    /// `sealed::__mint_working_tree_write`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn mint(workspace: WorkspaceId, branch: BranchName) -> Self {
        Self { workspace, branch }
    }

    pub fn workspace(&self) -> WorkspaceId {
        self.workspace
    }
    pub fn branch(&self) -> &BranchName {
        &self.branch
    }
}

/// Capability: holder may create a new branch in `workspace` rooted at `parent`.
#[derive(Debug, Clone)]
pub struct BranchCreate {
    workspace: WorkspaceId,
    parent: BranchName,
}

impl BranchCreate {
    /// Crate-internal constructor. External callers must use `vox-orchestrator-cap-mint`.
    /// Currently only exercised by in-crate tests; the runtime path goes through
    /// `sealed::__mint_branch_create`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn mint(workspace: WorkspaceId, parent: BranchName) -> Self {
        Self { workspace, parent }
    }

    pub fn workspace(&self) -> WorkspaceId {
        self.workspace
    }
    pub fn parent(&self) -> &BranchName {
        &self.parent
    }
}

/// Mint a [`WorkingTreeWrite`] capability for `workspace`/`branch`.
///
/// Authorization (lock-leader check, affinity, signature) is the caller's
/// responsibility — `vox_orchestrator::authorize_*` wrappers are the only
/// intended callers.
#[doc(hidden)]
pub fn mint_working_tree_write(workspace: WorkspaceId, branch: BranchName) -> WorkingTreeWrite {
    WorkingTreeWrite { workspace, branch }
}

/// Mint a [`BranchCreate`] capability for `workspace`/`parent`.
#[doc(hidden)]
pub fn mint_branch_create(workspace: WorkspaceId, parent: BranchName) -> BranchCreate {
    BranchCreate { workspace, parent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_id_displays_padded() {
        assert_eq!(WorkspaceId(7).to_string(), "W-000007");
    }

    #[test]
    fn branch_name_accepts_typical_agent_names() {
        BranchName::parse("agent/refactor-cache").unwrap();
        BranchName::parse("feature/cap-types").unwrap();
        BranchName::parse("wip.fix.42").unwrap();
    }

    #[test]
    fn branch_name_rejects_empty_or_too_long() {
        assert_eq!(
            BranchName::parse("").unwrap_err(),
            BranchNameError::InvalidLength
        );
        let too_long = "a".repeat(256);
        assert_eq!(
            BranchName::parse(&too_long).unwrap_err(),
            BranchNameError::InvalidLength
        );
    }

    #[test]
    fn branch_name_rejects_illegal_prefix_or_sequence() {
        assert_eq!(
            BranchName::parse("/foo").unwrap_err(),
            BranchNameError::IllegalPrefix
        );
        assert_eq!(
            BranchName::parse("-foo").unwrap_err(),
            BranchNameError::IllegalPrefix
        );
        assert_eq!(
            BranchName::parse("foo..bar").unwrap_err(),
            BranchNameError::IllegalSequence
        );
    }

    #[test]
    fn branch_name_rejects_illegal_chars() {
        assert!(matches!(
            BranchName::parse("foo bar"),
            Err(BranchNameError::IllegalChar(' '))
        ));
        assert!(matches!(
            BranchName::parse("foo:bar"),
            Err(BranchNameError::IllegalChar(':'))
        ));
        assert!(matches!(
            BranchName::parse("foo^bar"),
            Err(BranchNameError::IllegalChar('^'))
        ));
        assert!(matches!(
            BranchName::parse("foo~bar"),
            Err(BranchNameError::IllegalChar('~'))
        ));
        assert!(matches!(
            BranchName::parse("foo\\bar"),
            Err(BranchNameError::IllegalChar('\\'))
        ));
    }

    #[test]
    fn working_tree_write_round_trip() {
        // Uses the crate-internal `mint` (same crate as the test module).
        let cap = WorkingTreeWrite::mint(WorkspaceId(1), BranchName::parse("agent/x").unwrap());
        assert_eq!(cap.workspace(), WorkspaceId(1));
        assert_eq!(cap.branch().as_str(), "agent/x");
    }

    #[test]
    fn branch_create_round_trip() {
        let cap = BranchCreate::mint(WorkspaceId(2), BranchName::parse("main").unwrap());
        assert_eq!(cap.workspace(), WorkspaceId(2));
        assert_eq!(cap.parent().as_str(), "main");
    }

    #[test]
    fn mint_working_tree_write_round_trip_func() {
        let cap = mint_working_tree_write(WorkspaceId(5), BranchName::parse("test/branch").unwrap());
        assert_eq!(cap.workspace(), WorkspaceId(5));
    }
}
