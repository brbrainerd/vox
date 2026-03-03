use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::types::AgentId;

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
    pub path: PathBuf,
    pub kind: LockKind,
    pub holder: AgentId,
    pub acquired_at: Instant,
}

/// Error returned when a lock cannot be acquired.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LockConflict {
    #[error("File '{path}' is exclusively locked by agent {holder}")]
    ExclusivelyHeld { path: PathBuf, holder: AgentId },
    #[error("File '{path}' has shared readers; cannot acquire exclusive lock")]
    SharedReadersExist {
        path: PathBuf,
        readers: Vec<AgentId>,
    },
}

/// Entry tracking all current locks on a single file.
#[derive(Debug, Clone)]
enum LockEntry {
    /// A single agent holds exclusive access.
    Exclusive(FileLock),
    /// One or more agents hold shared read access.
    SharedRead(Vec<FileLock>),
}

/// Thread-safe file-level lock manager.
///
/// Enforces the single-writer principle: at most one agent can hold an
/// exclusive lock on any file, while multiple agents can hold shared read locks.
#[derive(Clone)]
pub struct FileLockManager {
    locks: Arc<RwLock<HashMap<PathBuf, LockEntry>>>,
    queue: Arc<RwLock<HashMap<PathBuf, std::collections::VecDeque<AgentId>>>>,
}

impl FileLockManager {
    /// Create a new, empty lock manager.
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Try to acquire a lock on a file.
    pub fn try_acquire(
        &self,
        path: &Path,
        agent: AgentId,
        kind: LockKind,
    ) -> Result<(), LockConflict> {
        let mut locks = self.locks.write().expect("lock manager lock poisoned");

        match (kind, locks.get(path)) {
            // No existing lock — acquire freely
            (_, None) => {
                let lock = FileLock {
                    path: path.to_path_buf(),
                    kind,
                    holder: agent,
                    acquired_at: Instant::now(),
                };
                let entry = match kind {
                    LockKind::Exclusive => LockEntry::Exclusive(lock),
                    LockKind::SharedRead => LockEntry::SharedRead(vec![lock]),
                };
                locks.insert(path.to_path_buf(), entry);
                Ok(())
            }

            // Requesting exclusive, but file already has exclusive lock
            (LockKind::Exclusive, Some(LockEntry::Exclusive(existing))) => {
                if existing.holder == agent {
                    Ok(()) // Re-entrant: same agent already holds it
                } else {
                    Err(LockConflict::ExclusivelyHeld {
                        path: path.to_path_buf(),
                        holder: existing.holder,
                    })
                }
            }

            // Requesting exclusive, but file has shared readers
            (LockKind::Exclusive, Some(LockEntry::SharedRead(readers))) => {
                // Allow if the only reader is the same agent
                if readers.len() == 1 && readers[0].holder == agent {
                    let lock = FileLock {
                        path: path.to_path_buf(),
                        kind: LockKind::Exclusive,
                        holder: agent,
                        acquired_at: Instant::now(),
                    };
                    locks.insert(path.to_path_buf(), LockEntry::Exclusive(lock));
                    Ok(())
                } else {
                    Err(LockConflict::SharedReadersExist {
                        path: path.to_path_buf(),
                        readers: readers.iter().map(|r| r.holder).collect(),
                    })
                }
            }

            // Requesting shared read, file has exclusive lock
            (LockKind::SharedRead, Some(LockEntry::Exclusive(existing))) => {
                if existing.holder == agent {
                    Ok(()) // Same agent can read its own exclusively locked file
                } else {
                    Err(LockConflict::ExclusivelyHeld {
                        path: path.to_path_buf(),
                        holder: existing.holder,
                    })
                }
            }

            // Requesting shared read, file already has shared readers — append
            (LockKind::SharedRead, Some(LockEntry::SharedRead(readers))) => {
                if readers.iter().any(|r| r.holder == agent) {
                    Ok(()) // Already has a read lock
                } else {
                    // Must drop immutable ref before mutating
                    drop(locks);
                    let mut locks = self.locks.write().expect("lock manager lock poisoned");
                    if let Some(LockEntry::SharedRead(readers)) = locks.get_mut(path) {
                        readers.push(FileLock {
                            path: path.to_path_buf(),
                            kind: LockKind::SharedRead,
                            holder: agent,
                            acquired_at: Instant::now(),
                        });
                    }
                    Ok(())
                }
            }
        }
    }

    /// Release a lock held by the given agent on the given file.
    pub fn release(&self, path: &Path, agent: AgentId) {
        let mut locks = self.locks.write().expect("lock manager lock poisoned");
        match locks.get(path) {
            Some(LockEntry::Exclusive(lock)) if lock.holder == agent => {
                locks.remove(path);
            }
            Some(LockEntry::SharedRead(readers)) => {
                let remaining: Vec<FileLock> = readers
                    .iter()
                    .filter(|r| r.holder != agent)
                    .cloned()
                    .collect();
                if remaining.is_empty() {
                    locks.remove(path);
                } else {
                    locks.insert(path.to_path_buf(), LockEntry::SharedRead(remaining));
                }
            }
            _ => {} // No lock held by this agent
        }
    }

    /// Release all locks held by the given agent.
    pub fn release_all(&self, agent: AgentId) {
        let mut locks = self.locks.write().expect("lock manager lock poisoned");
        let mut to_remove = Vec::new();
        let mut to_update = Vec::new();

        for (path, entry) in locks.iter() {
            match entry {
                LockEntry::Exclusive(lock) if lock.holder == agent => {
                    to_remove.push(path.clone());
                }
                LockEntry::SharedRead(readers) => {
                    let remaining: Vec<FileLock> = readers
                        .iter()
                        .filter(|r| r.holder != agent)
                        .cloned()
                        .collect();
                    if remaining.is_empty() {
                        to_remove.push(path.clone());
                    } else if remaining.len() != readers.len() {
                        to_update.push((path.clone(), remaining));
                    }
                }
                _ => {}
            }
        }

        for path in to_remove {
            locks.remove(&path);
        }
        for (path, remaining) in to_update {
            locks.insert(path, LockEntry::SharedRead(remaining));
        }
    }

    /// Check who holds a lock on a file (if any).
    pub fn holder(&self, path: &Path) -> Option<(AgentId, LockKind)> {
        let locks = self.locks.read().expect("lock manager lock poisoned");
        match locks.get(path) {
            Some(LockEntry::Exclusive(lock)) => Some((lock.holder, LockKind::Exclusive)),
            Some(LockEntry::SharedRead(readers)) => {
                readers.first().map(|r| (r.holder, LockKind::SharedRead))
            }
            None => None,
        }
    }

    /// Check whether a file is locked.
    pub fn is_locked(&self, path: &Path) -> bool {
        self.locks
            .read()
            .expect("lock manager lock poisoned")
            .contains_key(path)
    }

    /// List all current locks. Returns (path, holder_agent_id, exclusive).
    /// For shared read locks, one entry per holder with exclusive = false.
    pub fn list_locks(&self) -> Vec<(PathBuf, AgentId, bool)> {
        let locks = self.locks.read().expect("lock manager lock poisoned");
        let mut out = Vec::with_capacity(locks.len());
        for (path, entry) in locks.iter() {
            match entry {
                LockEntry::Exclusive(lock) => {
                    out.push((path.clone(), lock.holder, true));
                }
                LockEntry::SharedRead(readers) => {
                    for r in readers {
                        out.push((path.clone(), r.holder, false));
                    }
                }
            }
        }
        out
    }

    /// Detect potential deadlocks: agents waiting for each other's files.
    /// Returns a list of `(agent_a, agent_b, contested_path)` triples.
    pub fn deadlock_check(&self) -> Vec<(AgentId, AgentId, PathBuf)> {
        // Simple pairwise check: if agent A holds file X and wants file Y,
        // and agent B holds file Y and wants file X, that's a deadlock.
        // For now we just report all exclusive lock pairs that could conflict.
        let locks = self.locks.read().expect("lock manager lock poisoned");
        let mut pairs = Vec::new();
        let holders: Vec<(PathBuf, AgentId)> = locks
            .iter()
            .filter_map(|(path, entry)| match entry {
                LockEntry::Exclusive(lock) => Some((path.clone(), lock.holder)),
                _ => None,
            })
            .collect();

        for i in 0..holders.len() {
            for j in (i + 1)..holders.len() {
                if holders[i].1 != holders[j].1 {
                    pairs.push((holders[i].1, holders[j].1, holders[i].0.clone()));
                }
            }
        }
        pairs
    }

    /// Count of actively locked files.
    pub fn active_lock_count(&self) -> usize {
        self.locks.read().expect("lock manager lock poisoned").len()
    }

    /// Calculate how long a lock has been held (in milliseconds).
    pub fn lock_age(&self, path: &Path) -> Option<u128> {
        let locks = self.locks.read().expect("lock manager lock poisoned");
        match locks.get(path) {
            Some(LockEntry::Exclusive(lock)) => Some(lock.acquired_at.elapsed().as_millis()),
            Some(LockEntry::SharedRead(readers)) => {
                readers.first().map(|r| r.acquired_at.elapsed().as_millis())
            }
            None => None,
        }
    }

    /// Forcefully release any locks held for longer than timeout_ms.
    /// Returns the number of disconnected stale locks.
    pub fn force_release_stale(&self, timeout_ms: u128) -> usize {
        let mut locks = self.locks.write().expect("lock manager lock poisoned");
        let mut stale_count = 0;
        let mut to_remove = Vec::new();
        let mut to_update = Vec::new();

        for (path, entry) in locks.iter() {
            match entry {
                LockEntry::Exclusive(lock)
                    if lock.acquired_at.elapsed().as_millis() > timeout_ms =>
                {
                    to_remove.push(path.clone());
                    stale_count += 1;
                }
                LockEntry::SharedRead(readers) => {
                    let original_len = readers.len();
                    let remaining: Vec<FileLock> = readers
                        .iter()
                        .filter(|r| r.acquired_at.elapsed().as_millis() <= timeout_ms)
                        .cloned()
                        .collect();

                    stale_count += original_len - remaining.len();

                    if remaining.is_empty() {
                        to_remove.push(path.clone());
                    } else if remaining.len() != original_len {
                        to_update.push((path.clone(), remaining));
                    }
                }
                _ => {}
            }
        }

        for p in to_remove {
            locks.remove(&p);
        }
        for (p, arr) in to_update {
            locks.insert(p, LockEntry::SharedRead(arr));
        }

        stale_count
    }

    /// Upgrade a shared read lock to an exclusive write lock directly.
    pub fn escalate_read_to_write(&self, agent: AgentId, path: &Path) -> Result<(), LockConflict> {
        let mut locks = self.locks.write().expect("lock manager lock poisoned");
        match locks.get(path) {
            Some(LockEntry::SharedRead(readers)) => {
                // If this agent is the ONLY reader, escalate safely.
                if readers.len() == 1 && readers[0].holder == agent {
                    locks.insert(
                        path.to_path_buf(),
                        LockEntry::Exclusive(FileLock {
                            path: path.to_path_buf(),
                            kind: LockKind::Exclusive,
                            holder: agent,
                            acquired_at: Instant::now(),
                        }),
                    );
                    Ok(())
                } else {
                    // Other readers exist, cannot elevate right now
                    Err(LockConflict::SharedReadersExist {
                        path: path.to_path_buf(),
                        readers: readers.iter().map(|r| r.holder).collect(),
                    })
                }
            }
            Some(LockEntry::Exclusive(lock)) if lock.holder == agent => {
                // Already have write access
                Ok(())
            }
            Some(LockEntry::Exclusive(lock)) => Err(LockConflict::ExclusivelyHeld {
                path: path.to_path_buf(),
                holder: lock.holder,
            }),
            None => {
                // Not locked at all... this shouldn't typically happen in elevate context,
                // but just acquire write broadly
                locks.insert(
                    path.to_path_buf(),
                    LockEntry::Exclusive(FileLock {
                        path: path.to_path_buf(),
                        kind: LockKind::Exclusive,
                        holder: agent,
                        acquired_at: Instant::now(),
                    }),
                );
                Ok(())
            }
        }
    }

    /// Add an agent to the wait queue for a file they cannot lock.
    pub fn queue_agent_for_lock(&self, agent: AgentId, path: &Path) {
        let mut q = self.queue.write().unwrap();
        let queue = q.entry(path.to_path_buf()).or_default();
        if !queue.contains(&agent) {
            queue.push_back(agent);
        }
    }

    /// Dequeue the next agent waiting for a lock, if any.
    pub fn dequeue_waiter(&self, path: &Path) -> Option<AgentId> {
        let mut q = self.queue.write().unwrap();
        if let Some(queue) = q.get_mut(path) {
            queue.pop_front()
        } else {
            None
        }
    }

    /// Total number of agents waiting for any lock.
    pub fn contention_count(&self) -> usize {
        let q = self.queue.read().unwrap();
        q.values().map(|v| v.len()).sum()
    }
}

impl Default for FileLockManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
