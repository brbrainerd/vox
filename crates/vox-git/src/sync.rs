//! Sync operations for vox-git — fetch, push, and status.
//!
//! All network I/O goes through `gix` (pure Rust, no C).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::object::ObjectId;
use crate::refs::RefName;

/// Direction of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    /// Pull remote changes into local.
    Fetch,
    /// Push local changes to remote.
    Push,
    /// Fetch then push (full sync).
    Both,
}

/// Result of a fetch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    /// Remote URL that was fetched.
    pub remote_url: String,
    /// Refs that were updated locally.
    pub updated_refs: Vec<RefName>,
    /// Number of objects received.
    pub objects_received: u64,
    /// Bytes transferred.
    pub bytes_transferred: u64,
    /// Whether the fetch was a no-op (nothing new).
    pub was_up_to_date: bool,
}

/// Result of a push operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    /// Remote URL pushed to.
    pub remote_url: String,
    /// Refs successfully updated on remote.
    pub updated_refs: Vec<RefName>,
    /// Refs rejected by remote (e.g. non-fast-forward).
    pub rejected_refs: Vec<(RefName, String)>,
}

impl PushResult {
    /// True if the push was fully successful (no rejections).
    pub fn is_success(&self) -> bool {
        self.rejected_refs.is_empty()
    }
}

/// Current sync status between local and remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Repository root path.
    pub repo_path: PathBuf,
    /// Remote name (e.g., "origin").
    pub remote: String,
    /// Remote URL.
    pub remote_url: String,
    /// HEAD commit ID (if any).
    pub head_commit: Option<ObjectId>,
    /// Per-ref diffs between local and remote.
    pub ref_diffs: Vec<SyncStatusRef>,
}

/// One ref's sync status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusRef {
    pub ref_name: String,
    pub local_id: Option<String>,
    pub remote_id: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_result_success() {
        let r = PushResult {
            remote_url: "https://github.com/org/repo.git".into(),
            updated_refs: vec![RefName::branch("main")],
            rejected_refs: vec![],
        };
        assert!(r.is_success());
    }

    #[test]
    fn push_result_rejected() {
        let r = PushResult {
            remote_url: "https://github.com/org/repo.git".into(),
            updated_refs: vec![],
            rejected_refs: vec![(RefName::branch("main"), "non-fast-forward".into())],
        };
        assert!(!r.is_success());
    }
}
