use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedNode {
    pub node_id: String,
    pub pubkey_hex: String,
    pub label: Option<String>,
    pub added_at: String,
}

impl TrustedNode {
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

/// Registry of Ed25519-pubkey-to-node-id trust bindings.
///
/// Two modes:
/// - **File-backed** (`new()`): persists to `~/.vox/trusted_nodes.json`.
/// - **In-memory** (`default()` / `new_in_memory()`): no disk I/O; suitable for tests
///   and for the P5-T1b verifier path that receives nodes via A2A handshake.
pub struct TrustedNodeRegistry {
    path: Option<PathBuf>,
    cache: HashMap<String, TrustedNode>,
}

impl TrustedNodeRegistry {
    /// File-backed registry rooted at `~/.vox/trusted_nodes.json`.
    pub fn new() -> Self {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".vox");
        path.push("trusted_nodes.json");
        Self {
            path: Some(path),
            cache: HashMap::new(),
        }
    }

    /// Pure in-memory registry — no disk I/O. Used by tests and the P5 auth path.
    pub fn new_in_memory() -> Self {
        Self {
            path: None,
            cache: HashMap::new(),
        }
    }

    fn load(&self) -> Result<HashMap<String, TrustedNode>> {
        match &self.path {
            None => Ok(self.cache.clone()),
            Some(path) => {
                if !path.exists() {
                    return Ok(HashMap::new());
                }
                let data = fs::read_to_string(path)?;
                let nodes: Vec<TrustedNode> = serde_json::from_str(&data)?;
                Ok(nodes.into_iter().map(|n| (n.node_id.clone(), n)).collect())
            }
        }
    }

    fn save(&self, nodes: &HashMap<String, TrustedNode>) -> Result<()> {
        let path = match &self.path {
            None => return Ok(()), // in-memory: nothing to persist
            Some(p) => p,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let vec: Vec<&TrustedNode> = nodes.values().collect();
        let data = serde_json::to_string_pretty(&vec)?;
        fs::write(path, data)?;
        Ok(())
    }

    /// Add or update a node. For file-backed registries this persists to disk.
    pub fn add(&self, node_id: String, pubkey_hex: String, label: Option<String>) -> Result<()> {
        let mut nodes = self.load()?;
        let now = chrono::Utc::now().to_rfc3339();
        nodes.insert(
            node_id.clone(),
            TrustedNode {
                node_id,
                pubkey_hex,
                label,
                added_at: now,
            },
        );
        self.save(&nodes)
    }

    /// In-place upsert for in-memory registries (and tests). Does not persist.
    pub fn upsert(&mut self, node_id: &str, pubkey_hex: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        self.cache.insert(
            node_id.to_string(),
            TrustedNode {
                node_id: node_id.to_string(),
                pubkey_hex: pubkey_hex.to_string(),
                label: None,
                added_at: now,
            },
        );
    }

    /// Look up a trusted node by its Ed25519 pubkey hex. Searches in-memory
    /// cache first, then the file-backed store (if any).
    pub fn lookup_by_pubkey_hex(&self, pubkey_hex: &str) -> Option<&TrustedNode> {
        if let Some(node) = self.cache.values().find(|n| n.pubkey_hex == pubkey_hex) {
            return Some(node);
        }
        None
    }

    pub fn remove(&self, node_id: &str) -> Result<bool> {
        let mut nodes = self.load()?;
        let removed = nodes.remove(node_id).is_some();
        if removed {
            self.save(&nodes)?;
        }
        Ok(removed)
    }

    pub fn is_trusted(&self, node_id: &str) -> Result<bool> {
        if self.cache.contains_key(node_id) {
            return Ok(true);
        }
        let nodes = self.load()?;
        Ok(nodes.contains_key(node_id))
    }

    pub fn list(&self) -> Result<Vec<TrustedNode>> {
        let nodes = self.load()?;
        let mut vec: Vec<TrustedNode> = nodes.into_values().collect();
        vec.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        Ok(vec)
    }
}

impl Default for TrustedNodeRegistry {
    /// Creates an in-memory registry (no disk I/O). Use `new()` for the
    /// file-backed production registry.
    fn default() -> Self {
        Self::new_in_memory()
    }
}
