//! Per-file lock manager for exclusive writer access.

pub mod leader;
mod lease;
mod persisted;
mod refresh;
mod resource;

pub use lease::*;
pub use resource::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use vox_orchestrator_types::AgentId;

/// Kind of lock an agent holds on a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockKind {
    /// Exclusive write access — only one agent at a time.
    Exclusive,
    /// Shared read access — multiple agents can hold simultaneously.
    SharedRead,
}

/// A file lock held by an agent.
#[derive(Debug, Clone)]
pub struct FileLock {
    /// Locked file path (normalized canonical form where possible).
    pub path: PathBuf,
    /// Whether this is exclusive write or shared read.
    pub kind: LockKind,
    /// Agent that owns the lock.
    pub holder: AgentId,
    /// When the lock was granted (for debugging and TTL).
    pub acquired_at: std::time::Instant,
}

/// Error returned when a lock cannot be acquired.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LockConflict {
    /// Another agent already holds an exclusive lock on this path.
    #[error("File '{path}' is exclusively locked by agent {holder}")]
    ExclusivelyHeld {
        /// Path requested for locking.
        path: PathBuf,
        /// Current exclusive holder.
        holder: AgentId,
    },
    /// Exclusive lock requested while readers are still active.
    #[error("File '{path}' has shared readers; cannot acquire exclusive lock")]
    SharedReadersExist {
        /// Path requested for exclusive access.
        path: PathBuf,
        /// Agents currently holding shared read locks.
        readers: Vec<AgentId>,
    },
}

/// Entry tracking all current locks on a single file.
#[derive(Debug, Clone)]
pub(crate) enum LockEntry {
    /// A single agent holds exclusive access.
    Exclusive(
        /// Active exclusive lock record.
        FileLock,
    ),
    /// One or more agents hold shared read access.
    SharedRead(
        /// All concurrent read locks on this file.
        Vec<FileLock>,
    ),
}

/// Thread-safe file-level lock manager.
///
/// Enforces the single-writer principle: at most one agent can hold an
/// exclusive lock on any file, while multiple agents can hold shared read locks.
///
/// When constructed with [`FileLockManager::with_db`] the manager writes
/// through to the `vcs_lock` table on every acquire/release so that a fresh
/// daemon instance can replay the lock map on startup via
/// [`FileLockManager::hydrate_from_db`].
#[derive(Clone)]
pub struct FileLockManager {
    pub(crate) locks: Arc<std::sync::RwLock<HashMap<PathBuf, LockEntry>>>,
    pub(crate) queue: Arc<std::sync::RwLock<HashMap<PathBuf, std::collections::VecDeque<AgentId>>>>,
    /// Optional DB handle for persistent write-through (P0-T1).
    pub(crate) db: Option<vox_db::VoxDb>,
    /// Node-id string used as `holder_node_id` in the `vcs_lock` table.
    pub(crate) node_id: String,
    /// Repository-id scoping this manager's lock namespace.
    pub(crate) repository_id: String,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn acquire_and_release_exclusive() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .expect("should acquire");
        assert!(mgr.is_locked(Path::new("a.rs")));
        mgr.release(Path::new("a.rs"), AgentId(1));
        assert!(!mgr.is_locked(Path::new("a.rs")));
    }

    #[test]
    fn exclusive_conflict() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .expect("should acquire");
        let err = mgr
            .try_acquire(Path::new("a.rs"), AgentId(2), LockKind::Exclusive)
            .unwrap_err();
        assert!(matches!(err, LockConflict::ExclusivelyHeld { .. }));
    }

    #[test]
    fn reentrant_exclusive() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .expect("first");
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .expect("reentrant should succeed");
    }

    #[test]
    fn shared_read_coexistence() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::SharedRead)
            .expect("reader 1");
        mgr.try_acquire(Path::new("a.rs"), AgentId(2), LockKind::SharedRead)
            .expect("reader 2");
        assert!(mgr.is_locked(Path::new("a.rs")));
    }

    #[test]
    fn exclusive_blocked_by_shared_readers() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::SharedRead)
            .expect("reader");
        mgr.try_acquire(Path::new("a.rs"), AgentId(2), LockKind::SharedRead)
            .expect("reader 2");
        let err = mgr
            .try_acquire(Path::new("a.rs"), AgentId(3), LockKind::Exclusive)
            .unwrap_err();
        assert!(matches!(err, LockConflict::SharedReadersExist { .. }));
    }

    #[test]
    fn release_all_for_agent() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .unwrap();
        mgr.try_acquire(Path::new("b.rs"), AgentId(1), LockKind::Exclusive)
            .unwrap();
        mgr.try_acquire(Path::new("c.rs"), AgentId(2), LockKind::Exclusive)
            .unwrap();
        mgr.release_all(AgentId(1));
        assert!(!mgr.is_locked(Path::new("a.rs")));
        assert!(!mgr.is_locked(Path::new("b.rs")));
        assert!(mgr.is_locked(Path::new("c.rs")));
    }

    #[test]
    fn holder_returns_correct_info() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("x.rs"), AgentId(5), LockKind::Exclusive)
            .unwrap();
        let (agent, kind) = mgr.holder(Path::new("x.rs")).unwrap();
        assert_eq!(agent, AgentId(5));
        assert_eq!(kind, LockKind::Exclusive);
    }

    #[test]
    fn list_locks_returns_all_entries() {
        let mgr = FileLockManager::new();
        mgr.try_acquire(Path::new("a.rs"), AgentId(1), LockKind::Exclusive)
            .unwrap();
        mgr.try_acquire(Path::new("b.rs"), AgentId(2), LockKind::Exclusive)
            .unwrap();
        mgr.try_acquire(Path::new("c.rs"), AgentId(1), LockKind::SharedRead)
            .unwrap();
        mgr.try_acquire(Path::new("c.rs"), AgentId(3), LockKind::SharedRead)
            .unwrap();

        let list = mgr.list_locks();
        assert_eq!(list.len(), 4);
        let paths: std::collections::HashSet<_> = list.iter().map(|(p, _, _)| p.clone()).collect();
        assert!(paths.contains(&PathBuf::from("a.rs")));
        assert!(paths.contains(&PathBuf::from("b.rs")));
        assert!(paths.contains(&PathBuf::from("c.rs")));
        let exclusive: Vec<_> = list.iter().filter(|(_, _, ex)| *ex).collect();
        assert_eq!(exclusive.len(), 2);
        let shared: Vec<_> = list.iter().filter(|(_, _, ex)| !*ex).collect();
        assert_eq!(shared.len(), 2);
    }
}
