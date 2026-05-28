//! Conflict-funnel outcome returned from merge and file-affinity checks (P3-T5).
//!
//! Implements the three-tier conflict funnel from the multi-agent VCS replication spec:
//!   Tier 1 — `Merged`     : no conflict, content merged cleanly.
//!   Tier 2 — `LockWait`   : remote daemon holds a lease; caller should retry after `lease_ms`.
//!   Tier 3 — `Conflict`   : unresolvable; needs human or AI review.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Opaque 16-byte daemon node identity (UUIDv4 bytes in network order).
///
/// Defined here so `MergeOutcome` can reference it without pulling in the
/// full orchestrator-queue crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DaemonId(pub [u8; 16]);

/// Three-tier conflict-funnel outcome for branch-merge and lock operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MergeOutcome {
    /// Content merged cleanly; no conflict.
    Merged,

    /// Tier-2 of the conflict funnel (per multi-agent-vcs-replication-spec §Wire-protocol).
    ///
    /// A remote daemon holds an unexpired lease on `path`. Caller should retry
    /// after `lease_ms` milliseconds or request a hand-off from `leader`.
    LockWait {
        path: PathBuf,
        /// Daemon node that currently holds the write lease.
        leader: DaemonId,
        /// Suggested retry delay in milliseconds (typically half the remaining lease).
        lease_ms: u64,
        /// Lamport clock observed at the leader at the time of the wait response.
        leader_lamport: u64,
    },

    /// Tier-3: unresolvable conflict requiring human or AI review.
    Conflict { path: PathBuf, reason: String },
}

impl MergeOutcome {
    /// Returns `true` if the merge completed without any conflict.
    pub fn is_merged(&self) -> bool {
        matches!(self, Self::Merged)
    }

    /// Returns `true` if the caller should retry after a lease expires.
    pub fn is_lock_wait(&self) -> bool {
        matches!(self, Self::LockWait { .. })
    }

    /// Returns the suggested retry delay when `LockWait`, otherwise `None`.
    pub fn lock_wait_lease_ms(&self) -> Option<u64> {
        if let Self::LockWait { lease_ms, .. } = self {
            Some(*lease_ms)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_serializes_round_trips() {
        let outcome = MergeOutcome::Merged;
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"outcome\":\"merged\""));
        let back: MergeOutcome = serde_json::from_str(&json).unwrap();
        assert!(back.is_merged());
    }

    #[test]
    fn lock_wait_round_trips() {
        let outcome = MergeOutcome::LockWait {
            path: PathBuf::from("src/main.rs"),
            leader: DaemonId([7u8; 16]),
            lease_ms: 15_000,
            leader_lamport: 42,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: MergeOutcome = serde_json::from_str(&json).unwrap();
        assert!(back.is_lock_wait());
        assert_eq!(back.lock_wait_lease_ms(), Some(15_000));
    }

    #[test]
    fn conflict_round_trips() {
        let outcome = MergeOutcome::Conflict {
            path: PathBuf::from("Cargo.toml"),
            reason: "concurrent writes".into(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: MergeOutcome = serde_json::from_str(&json).unwrap();
        assert!(!back.is_merged());
        assert!(!back.is_lock_wait());
    }

    #[test]
    fn is_merged_false_for_lock_wait() {
        let o = MergeOutcome::LockWait {
            path: PathBuf::from("x.rs"),
            leader: DaemonId([0u8; 16]),
            lease_ms: 5_000,
            leader_lamport: 1,
        };
        assert!(!o.is_merged());
    }

    #[test]
    fn lock_wait_lease_ms_none_for_conflict() {
        let o = MergeOutcome::Conflict {
            path: PathBuf::from("y.rs"),
            reason: "x".into(),
        };
        assert_eq!(o.lock_wait_lease_ms(), None);
    }
}
