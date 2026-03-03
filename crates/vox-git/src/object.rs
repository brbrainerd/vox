//! Low-level Git object representation for vox-git.
//!
//! Provides Vox-native wrappers over Git object IDs, trees, and blobs.
//! No gix types escape this module — callers work with Vox types only.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A Git object ID (SHA-1 or SHA-256 hash, hex-encoded).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    /// Parse a hex string into an `ObjectId`. Returns `None` if invalid.
    pub fn parse(hex: impl Into<String>) -> Option<Self> {
        let s = hex.into();
        if s.len() >= 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(Self(s))
        } else {
            None
        }
    }

    /// Short (7-char) display for human output.
    pub fn short(&self) -> &str {
        &self.0[..self.0.len().min(7)]
    }

    /// Full hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short())
    }
}

/// The kind of Git object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectKind {
    Blob,
    Tree,
    Commit,
    Tag,
}

/// A Git commit (Vox-native representation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    /// Full object ID.
    pub id: ObjectId,
    /// Parent commit IDs (empty for root commit).
    pub parents: Vec<ObjectId>,
    /// Tree object ID.
    pub tree_id: ObjectId,
    /// Commit message (decoded as UTF-8, lossy).
    pub message: String,
    /// Author name.
    pub author_name: String,
    /// Author email.
    pub author_email: String,
    /// Committer name.
    pub committer_name: String,
    /// Committer email.
    pub committer_email: String,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: i64,
}

impl GitCommit {
    /// One-line summary (first line of message).
    pub fn summary(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }

    /// True if this is a merge commit (>1 parent).
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// A Git tree entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    /// File mode (e.g., "100644", "040000").
    pub mode: String,
    /// Entry name (filename or directory name).
    pub name: String,
    /// Object ID of the blob or subtree.
    pub id: ObjectId,
    /// Whether this entry is a directory (tree).
    pub is_tree: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_parse_valid() {
        let id = ObjectId::parse("a94a8fe5ccb19ba61c4c0873d391e987982fbbd3").unwrap();
        assert_eq!(id.short(), "a94a8fe");
    }

    #[test]
    fn object_id_parse_too_short() {
        assert!(ObjectId::parse("a94a8fe").is_none());
    }

    #[test]
    fn commit_summary() {
        let c = GitCommit {
            id: ObjectId("aaaa".repeat(10)),
            parents: vec![],
            tree_id: ObjectId("bbbb".repeat(10)),
            message: "Fix bug\n\nDetailed explanation\n".into(),
            author_name: "Alice".into(),
            author_email: "alice@example.com".into(),
            committer_name: "Alice".into(),
            committer_email: "alice@example.com".into(),
            timestamp: 0,
        };
        assert_eq!(c.summary(), "Fix bug");
        assert!(!c.is_merge());
    }
}
