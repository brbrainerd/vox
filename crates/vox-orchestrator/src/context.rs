use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::types::{AgentId, VcsContext};

/// An entry in the shared context store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextEntry {
    /// The agent that set this context value.
    pub agent_id: AgentId,
    /// The actual JSON/text value being stored.
    pub value: String,
    /// Absolute expiration timestamp (Unix timestamp in seconds).
    /// If 0, the context never expires.
    pub expires_at: u64,
    /// Optional VCS state this context refers to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_context: Option<VcsContext>,
}

/// In-memory store for sharing context between agents.
/// State is designed to be serialized alongside the Orchestrator via state.rs.
#[derive(Debug, Clone, Default)]
pub struct ContextStore {
    inner: Arc<RwLock<HashMap<String, ContextEntry>>>,
}

impl ContextStore {
    /// Create a new, empty context store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load from an existing map of entries (e.g. from state loader).
    pub fn from_entries(entries: HashMap<String, ContextEntry>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(entries)),
        }
    }

    /// Dump all current entries (e.g. for state saving).
    pub fn entries(&self) -> HashMap<String, ContextEntry> {
        self.inner.read().expect("context lock poisoned").clone()
    }

    /// Set a context value.
    /// `ttl_seconds` is the time-to-live. Use 0 for no expiration.
    pub fn set(
        &self,
        agent_id: AgentId,
        key: impl Into<String>,
        value: impl Into<String>,
        ttl_seconds: u64,
    ) {
        self.set_with_vcs(agent_id, key, value, ttl_seconds, None)
    }

    /// Set a context value with attached VCS context.
    pub fn set_with_vcs(
        &self,
        agent_id: AgentId,
        key: impl Into<String>,
        value: impl Into<String>,
        ttl_seconds: u64,
        vcs_context: Option<VcsContext>,
    ) {
        let expires_at = if ttl_seconds > 0 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                + ttl_seconds
        } else {
            0
        };

        let entry = ContextEntry {
            agent_id,
            value: value.into(),
            expires_at,
            vcs_context,
        };

        self.inner
            .write()
            .expect("context lock poisoned")
            .insert(key.into(), entry);
    }

    /// Get a context value by key.
    /// Returns None if the key does not exist or has expired.
    pub fn get(&self, key: &str) -> Option<String> {
        self.get_entry(key).map(|e| e.value)
    }

    /// Get the full ContextEntry by key.
    pub fn get_entry(&self, key: &str) -> Option<ContextEntry> {
        let map = self.inner.read().expect("context lock poisoned");
        if let Some(entry) = map.get(key) {
            if entry.expires_at == 0 || entry.expires_at > current_timestamp() {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Return all keys starting with a specific prefix.
    pub fn list_keys(&self, prefix: &str) -> Vec<String> {
        let map = self.inner.read().expect("context lock poisoned");
        let now = current_timestamp();
        map.iter()
            .filter(|(k, v)| k.starts_with(prefix) && (v.expires_at == 0 || v.expires_at > now))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Clear out items that have expired their TTL.
    /// Intended to be run periodically by the orchestrator.
    pub fn expire_stale(&self) -> usize {
        let mut map = self.inner.write().expect("context lock poisoned");
        let now = current_timestamp();
        let initial_len = map.len();
        map.retain(|_, v| v.expires_at == 0 || v.expires_at > now);
        initial_len - map.len()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let store = ContextStore::new();
        store.set(AgentId(1), "test_key", "test_value", 0);
        assert_eq!(store.get("test_key").unwrap(), "test_value");
    }

    #[test]
    fn ttl_expiration() {
        let store = ContextStore::new();
        // Set with 2 seconds TTL to avoid immediate expiry due to rapid execution,
        // but simulate expiration via modifying the entry.
        store.set(AgentId(1), "exp", "val", 1);

        {
            let mut map = store.inner.write().unwrap();
            map.get_mut("exp").unwrap().expires_at = current_timestamp() - 10;
        }

        assert_eq!(store.get("exp"), None);
        assert_eq!(store.expire_stale(), 1);
        assert_eq!(store.get("exp"), None);
    }

    #[test]
    fn vcs_context_retrieval() {
        let store = ContextStore::new();
        let ctx = VcsContext {
            snapshot_before: Some(10),
            snapshot_after: None,
            touched_paths: vec!["src/main.rs".into()],
            change_id: None,
            op_id: None,
            content_hash: None,
        };
        store.set_with_vcs(AgentId(1), "vcs_key", "data", 0, Some(ctx));

        let entry = store.get_entry("vcs_key").unwrap();
        assert_eq!(entry.value, "data");
        let vcs = entry.vcs_context.unwrap();
        assert_eq!(vcs.snapshot_before, Some(10));
        assert_eq!(vcs.touched_paths.len(), 1);
    }

    #[test]
    fn prefix_matching() {
        let store = ContextStore::new();
        store.set(AgentId(1), "app.component.a", "val", 0);
        store.set(AgentId(1), "app.component.b", "val", 0);
        store.set(AgentId(1), "other.key", "val", 0);

        let mut keys = store.list_keys("app.");
        keys.sort();
        assert_eq!(keys, vec!["app.component.a", "app.component.b"]);
    }
}
