//! Git ref management for vox-git.
//!
//! Provides Vox-native types for branches, tags, and remote tracking refs.

use crate::object::ObjectId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A fully-qualified Git ref name (e.g., `refs/heads/main`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RefName(pub String);

impl RefName {
    /// Create from a string.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// `refs/heads/<branch>` shortcut.
    pub fn branch(name: &str) -> Self {
        Self(format!("refs/heads/{name}"))
    }

    /// `refs/tags/<tag>` shortcut.
    pub fn tag(name: &str) -> Self {
        Self(format!("refs/tags/{name}"))
    }

    /// `refs/remotes/<remote>/<branch>` shortcut.
    pub fn remote_tracking(remote: &str, branch: &str) -> Self {
        Self(format!("refs/remotes/{remote}/{branch}"))
    }

    /// Extract branch name from `refs/heads/…`.
    pub fn as_branch_name(&self) -> Option<&str> {
        self.0.strip_prefix("refs/heads/")
    }

    /// Extract tag name from `refs/tags/…`.
    pub fn as_tag_name(&self) -> Option<&str> {
        self.0.strip_prefix("refs/tags/")
    }

    /// Raw ref string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A resolved Git ref — a ref name bound to an object ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRef {
    pub name: RefName,
    pub target: ObjectId,
    /// Whether the ref is symbolic (e.g., `HEAD -> refs/heads/main`).
    pub is_symbolic: bool,
}

/// A remote bookmark — mirrors a remote's current ref state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    /// Remote name (e.g., "origin").
    pub remote: String,
    /// Branch/tag name on the remote.
    pub ref_name: RefName,
    /// Object ID the remote reports.
    pub target: ObjectId,
}

/// Diff between local and remote refs — used by sync to determine what to push/fetch.
#[derive(Debug, Clone)]
pub struct RefDiff {
    pub ref_name: RefName,
    pub local: Option<ObjectId>,
    pub remote: Option<ObjectId>,
    pub status: RefStatus,
}

/// Status of a ref relative to its remote counterpart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefStatus {
    /// Local is ahead of remote (can push).
    Ahead,
    /// Remote is ahead of local (should fetch).
    Behind,
    /// Both sides modified independently (diverged).
    Diverged,
    /// In sync.
    UpToDate,
    /// Ref only on local side.
    LocalOnly,
    /// Ref only on remote side.
    RemoteOnly,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_name_shortcuts() {
        assert_eq!(RefName::branch("main").as_str(), "refs/heads/main");
        assert_eq!(RefName::tag("v1.0").as_str(), "refs/tags/v1.0");
        assert_eq!(
            RefName::remote_tracking("origin", "main").as_str(),
            "refs/remotes/origin/main"
        );
    }

    #[test]
    fn branch_name_extraction() {
        let r = RefName::branch("feature/my-branch");
        assert_eq!(r.as_branch_name(), Some("feature/my-branch"));
        assert!(r.as_tag_name().is_none());
    }
}
