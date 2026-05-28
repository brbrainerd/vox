use std::path::Path;
use std::time::Instant;

use crate::sync_lock;
use vox_orchestrator_types::AgentId;

use super::{FileLock, FileLockManager, LockConflict, LockEntry, LockKind};

/// Snapshot of a distributed lock from the DB.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbLock {
    pub lock_key: String,
    pub holder_node: String,
    pub holder_agent: AgentId,
    pub fence_token: i64,
    pub acquired_at: String,
    pub expires_at: String,
    pub repository_id: String,
}

/// Attempt to acquire a distributed lock in Turso with circuit breaker protection.
pub async fn acquire_distributed_lock_with_breaker(
    db: &vox_db::VoxDb,
    lock_key: &str,
    node_id: &str,
    agent_id: AgentId,
    ttl_secs: u64,
    repository_id: &str,
) -> Result<Result<i64, String>, String> {
    db.breaker()
        .call(|| async {
            acquire_distributed_lock(db, lock_key, node_id, agent_id, ttl_secs, repository_id).await
        })
        .await
}

/// Release a distributed lock in the database with circuit breaker protection.
pub async fn release_distributed_lock_with_breaker(
    db: &vox_db::VoxDb,
    lock_key: &str,
    node_id: &str,
    repository_id: &str,
) -> Result<(), String> {
    db.breaker()
        .call(|| async { release_distributed_lock(db, lock_key, node_id, repository_id).await })
        .await
}

/// Remove all locks that have passed their `expires_at` with circuit breaker protection.
pub async fn prune_stale_distributed_locks_with_breaker(db: &vox_db::VoxDb) -> Result<u64, String> {
    db.breaker()
        .call(|| async { prune_stale_distributed_locks(db).await })
        .await
}

/// Attempt to acquire a distributed lock in Turso.
///
/// Returns `Ok(fence_token)` if successful, or `Err(holder_node_id)` if already held.
pub async fn acquire_distributed_lock(
    store: &vox_db::VoxDb,
    lock_key: &str,
    node_id: &str,
    agent_id: AgentId,
    ttl_secs: u64,
    repository_id: &str,
) -> Result<Result<i64, String>, String> {
    store
        .acquire_distributed_lock(
            lock_key,
            node_id,
            &agent_id.0.to_string(),
            ttl_secs as i64,
            repository_id,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Release a distributed lock in the database.
pub async fn release_distributed_lock(
    store: &vox_db::VoxDb,
    lock_key: &str,
    node_id: &str,
    repository_id: &str,
) -> Result<(), String> {
    store
        .release_distributed_lock(lock_key, node_id, repository_id)
        .await
        .map_err(|e| e.to_string())
}

/// Remove all locks that have passed their `expires_at`.
pub async fn prune_stale_distributed_locks(store: &vox_db::VoxDb) -> Result<u64, String> {
    store
        .prune_stale_distributed_locks()
        .await
        .map_err(|e| e.to_string())
}

impl FileLockManager {
    /// Create a new, empty in-memory-only lock manager (no DB persistence).
    pub fn new() -> Self {
        Self {
            locks: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            queue: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            db: None,
            node_id: String::new(),
            repository_id: String::new(),
        }
    }

    /// Try to acquire a lock on a file.
    pub fn try_acquire(
        &self,
        path: &Path,
        agent: AgentId,
        kind: LockKind,
    ) -> Result<(), LockConflict> {
        let mut locks = sync_lock::rw_write(&*self.locks);

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
                    let mut locks = sync_lock::rw_write(&*self.locks);
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
        let mut locks = sync_lock::rw_write(&*self.locks);
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
        let mut locks = sync_lock::rw_write(&*self.locks);
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
        let locks = sync_lock::rw_read(&*self.locks);
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
        sync_lock::rw_read(&*self.locks).contains_key(path)
    }

    /// List all current locks. Returns (path, holder_agent_id, exclusive).
    /// For shared read locks, one entry per holder with exclusive = false.
    pub fn list_locks(&self) -> Vec<(std::path::PathBuf, AgentId, bool)> {
        let locks = sync_lock::rw_read(&*self.locks);
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
    pub fn deadlock_check(&self) -> Vec<(AgentId, AgentId, std::path::PathBuf)> {
        // Simple pairwise check: if agent A holds file X and wants file Y,
        // and agent B holds file Y and wants file X, that's a deadlock.
        // For now we just report all exclusive lock pairs that could conflict.
        let locks = sync_lock::rw_read(&*self.locks);
        let mut pairs = Vec::new();
        let holders: Vec<(std::path::PathBuf, AgentId)> = locks
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
        sync_lock::rw_read(&*self.locks).len()
    }

    /// Calculate how long a lock has been held (in milliseconds).
    pub fn lock_age(&self, path: &Path) -> Option<u128> {
        let locks = sync_lock::rw_read(&*self.locks);
        match locks.get(path) {
            Some(LockEntry::Exclusive(lock)) => Some(lock.acquired_at.elapsed().as_millis()),
            Some(LockEntry::SharedRead(readers)) => {
                readers.first().map(|r| r.acquired_at.elapsed().as_millis())
            }
            None => None,
        }
    }
}
