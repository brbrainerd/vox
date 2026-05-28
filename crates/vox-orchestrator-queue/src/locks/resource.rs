use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use vox_orchestrator_types::AgentId;

/// Kind of resource lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceLockKind {
    /// Exclusive access — only one agent at a time.
    Exclusive,
    /// Shared access — multiple agents can hold simultaneously.
    Shared,
}

/// A generic resource lock (e.g. database row, URI, logical semaphore).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLock {
    /// Unique resource identifier (e.g. `"db://users/1"`, `"http://api.com/resource"`).
    pub resource_id: String,
    pub kind: ResourceLockKind,
    pub holder: AgentId,
    /// Unix timestamp when the lock expires.
    pub expires_ms: u64,
}

/// Thread-safe resource lock manager with lease propagation support.
#[derive(Clone, Default)]
pub struct ResourceLockManager {
    locks: Arc<std::sync::RwLock<HashMap<String, ResourceLock>>>,
}

impl ResourceLockManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to acquire a resource lock.
    pub fn try_acquire(
        &self,
        resource_id: &str,
        agent_id: AgentId,
        kind: ResourceLockKind,
        ttl_ms: u64,
    ) -> Result<ResourceLock, String> {
        let mut locks = self.locks.write().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if let Some(existing) = locks.get(resource_id)
            && existing.expires_ms > now
            && existing.holder != agent_id
        {
            return Err(format!(
                "Resource '{}' is already locked by agent {}",
                resource_id, existing.holder
            ));
        }
        // Re-entrant: extend TTL

        let lock = ResourceLock {
            resource_id: resource_id.to_string(),
            kind,
            holder: agent_id,
            expires_ms: now + ttl_ms,
        };
        locks.insert(resource_id.to_string(), lock.clone());
        Ok(lock)
    }

    /// Release a resource lock.
    pub fn release(&self, resource_id: &str, agent_id: AgentId) {
        let mut locks = self.locks.write().unwrap();
        if let Some(existing) = locks.get(resource_id)
            && existing.holder == agent_id
        {
            locks.remove(resource_id);
        }
    }

    /// Number of active locks.
    pub fn len(&self) -> usize {
        self.locks.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.locks.read().unwrap().is_empty()
    }

    /// Returns a snapshot of all active locks.
    pub fn snapshot(&self) -> Vec<ResourceLock> {
        self.locks.read().unwrap().values().cloned().collect()
    }

    /// Check if a resource is currently locked.
    pub fn is_locked(&self, resource_id: &str) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let locks = self.locks.read().unwrap();
        if let Some(lock) = locks.get(resource_id) {
            lock.expires_ms > now
        } else {
            false
        }
    }
}
