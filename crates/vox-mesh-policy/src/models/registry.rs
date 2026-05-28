//! In-memory model registry aggregated from node heartbeats.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::types::{ModelEntry, NodeModelMap};

/// Aggregated view of models across all mesh nodes.
///
/// Updated by the orchestrator as nodes report their model inventories
/// via heartbeat. Thread-safe; readers take a read lock.
#[derive(Clone, Default)]
pub struct ModelRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

#[derive(Default)]
struct RegistryInner {
    /// node_id → list of (tag, kind, size_bytes, digest)
    by_node: HashMap<String, Vec<ModelEntry>>,
}

impl ModelRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Report or update the model inventory for a node.
    pub fn upsert_node(&self, node_id: &str, models: Vec<ModelEntry>) {
        let mut g = self.inner.write().expect("model registry lock poisoned");
        g.by_node.insert(node_id.to_string(), models);
    }

    /// Remove a node's inventory (called when the node leaves the mesh).
    pub fn remove_node(&self, node_id: &str) {
        let mut g = self.inner.write().expect("model registry lock poisoned");
        g.by_node.remove(node_id);
    }

    /// Return a de-duplicated list of all models across all nodes.
    ///
    /// Models with the same `tag` and `kind` are merged; their `nodes` lists
    /// are unioned. The result is sorted by tag for stable output.
    pub fn all_models(&self) -> Vec<ModelEntry> {
        let g = self.inner.read().expect("model registry lock poisoned");
        let mut merged: HashMap<String, ModelEntry> = HashMap::new();
        for (node_id, models) in &g.by_node {
            for m in models {
                let key = format!("{}:{}", m.kind.as_str(), m.tag);
                merged
                    .entry(key)
                    .and_modify(|e| {
                        if !e.nodes.contains(node_id) {
                            e.nodes.push(node_id.clone());
                        }
                    })
                    .or_insert_with(|| ModelEntry {
                        tag: m.tag.clone(),
                        kind: m.kind.clone(),
                        nodes: vec![node_id.clone()],
                        size_bytes: m.size_bytes,
                        digest: m.digest.clone(),
                    });
            }
        }
        let mut result: Vec<ModelEntry> = merged.into_values().collect();
        result.sort_by(|a, b| a.tag.cmp(&b.tag));
        result
    }

    /// Return the model inventory for a specific node.
    pub fn models_for_node(&self, node_id: &str) -> Vec<ModelEntry> {
        let g = self.inner.read().expect("model registry lock poisoned");
        g.by_node.get(node_id).cloned().unwrap_or_default()
    }

    /// Return a per-node breakdown of all model inventories.
    pub fn node_model_maps(&self) -> Vec<NodeModelMap> {
        let g = self.inner.read().expect("model registry lock poisoned");
        let mut maps: Vec<NodeModelMap> = g
            .by_node
            .iter()
            .map(|(node_id, models): (&String, &Vec<ModelEntry>)| NodeModelMap {
                node_id: node_id.clone(),
                models: models.clone(),
            })
            .collect();
        maps.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        maps
    }

    /// Total number of nodes that have reported model inventories.
    pub fn node_count(&self) -> usize {
        let g = self.inner.read().expect("model registry lock poisoned");
        g.by_node.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::types::ModelKind;

    fn model(tag: &str, kind: ModelKind) -> ModelEntry {
        ModelEntry {
            tag: tag.into(),
            kind,
            nodes: vec![],
            size_bytes: None,
            digest: None,
        }
    }

    #[test]
    fn empty_registry_returns_no_models() {
        let r = ModelRegistry::empty();
        assert_eq!(r.all_models().len(), 0);
    }

    #[test]
    fn upsert_node_populates_inventory() {
        let r = ModelRegistry::empty();
        r.upsert_node("node-a", vec![model("llama3:8b", ModelKind::Ollama)]);
        assert_eq!(r.all_models().len(), 1);
        assert_eq!(r.all_models()[0].tag, "llama3:8b");
        assert_eq!(r.all_models()[0].nodes, vec!["node-a"]);
    }

    #[test]
    fn same_model_on_two_nodes_is_merged() {
        let r = ModelRegistry::empty();
        r.upsert_node("node-a", vec![model("mistral:latest", ModelKind::Ollama)]);
        r.upsert_node("node-b", vec![model("mistral:latest", ModelKind::Ollama)]);
        let models = r.all_models();
        assert_eq!(models.len(), 1);
        let mut nodes = models[0].nodes.clone();
        nodes.sort();
        assert_eq!(nodes, vec!["node-a", "node-b"]);
    }

    #[test]
    fn remove_node_removes_its_models() {
        let r = ModelRegistry::empty();
        r.upsert_node("node-a", vec![model("llama3:8b", ModelKind::Ollama)]);
        r.upsert_node("node-b", vec![model("mistral:latest", ModelKind::Ollama)]);
        r.remove_node("node-a");
        let models = r.all_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].tag, "mistral:latest");
    }

    #[test]
    fn node_model_maps_returns_per_node_view() {
        let r = ModelRegistry::empty();
        r.upsert_node("node-a", vec![model("llama3:8b", ModelKind::Ollama)]);
        r.upsert_node("node-b", vec![model("codellama:7b", ModelKind::Ollama)]);
        let maps = r.node_model_maps();
        assert_eq!(maps.len(), 2);
        // sorted by node_id
        assert_eq!(maps[0].node_id, "node-a");
        assert_eq!(maps[1].node_id, "node-b");
    }
}
