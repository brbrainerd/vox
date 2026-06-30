//! Read-only graph reader for Graphify `graph.json` (NetworkX JSON export format).
//!
//! # Graph format
//! ```json
//! { "nodes": [{"id": "x", "label": "...", "community": "c1"}],
//!   "links": [{"source": "x", "target": "y"}] }
//! ```
//! Edges may appear under `"links"` or `"edges"` — both are supported.
//! The graph is treated as **undirected**: edges are indexed in both directions.

#![allow(clippy::collapsible_if, clippy::unnecessary_map_or)]

pub mod ast;
pub mod bfs;
pub mod cache;
pub mod cluster;
pub mod compare;
pub mod coverage;
pub mod crate_model;
pub mod gc;
pub mod lens;
pub mod manifest;
pub mod overlay;
pub mod reachability;
pub mod rebuild;
pub mod registry;
pub mod snapshot;

pub use bfs::Direction;

use std::collections::HashMap;

/// Error type for [`GraphifyReader`] construction.
#[derive(Debug)]
pub enum GraphifyReaderError {
    /// The JSON value had no `"nodes"` array.
    MissingNodes,
}

impl std::fmt::Display for GraphifyReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphifyReaderError::MissingNodes => {
                write!(f, "graph JSON missing 'nodes' array")
            }
        }
    }
}

impl std::error::Error for GraphifyReaderError {}

/// A single result from BFS traversal.
#[derive(Debug, Clone)]
pub struct TraversalHit {
    /// Node ID as it appears in the graph JSON.
    pub node_id: String,
    /// Human-readable label for this node.
    pub label: String,
    /// Number of hops from the nearest seed node.
    pub depth: u8,
    /// Ordered list of node IDs from seed to this node (inclusive of both).
    pub path: Vec<String>,
}

/// Read-only graph reader. Builds an in-memory adjacency index from a Graphify JSON value.
///
/// Construction is O(N + E). Queries are O(N + E) worst case for full BFS.
#[derive(Debug)]
pub struct GraphifyReader {
    // node_id → (label, community_id)
    nodes: HashMap<String, (String, Option<String>)>,
    // Undirected adjacency: node_id → Vec<neighbor_ids>
    adjacency: HashMap<String, Vec<String>>,
    // Directed: caller → callees (forward = on-disk source→target order).
    forward: HashMap<String, Vec<String>>,
    // Directed: callee → callers (reverse).
    reverse: HashMap<String, Vec<String>>,
}

impl GraphifyReader {
    /// Construct from a parsed `serde_json::Value`.
    ///
    /// Returns [`GraphifyReaderError::MissingNodes`] if the `"nodes"` key is absent or not an array.
    pub fn from_value(value: serde_json::Value) -> Result<Self, GraphifyReaderError> {
        let nodes_arr = value
            .get("nodes")
            .and_then(|n| n.as_array())
            .ok_or(GraphifyReaderError::MissingNodes)?;

        let mut nodes: HashMap<String, (String, Option<String>)> =
            HashMap::with_capacity(nodes_arr.len());

        for node in nodes_arr {
            // Prefer "id", fall back to "label" for the node key.
            let id = node
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| node.get("label").and_then(|v| v.as_str()).unwrap_or(""))
                .to_string();
            if id.is_empty() {
                continue;
            }
            let label = node
                .get("label")
                .or_else(|| node.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&id)
                .to_string();
            let community = node
                .get("community")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            nodes.insert(id, (label, community));
        }

        // Build undirected adjacency from "links" or "edges" (both supported).
        let edges_arr = value
            .get("links")
            .or_else(|| value.get("edges"))
            .and_then(|e| e.as_array());

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());
        let mut forward: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());
        let mut reverse: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());

        if let Some(edges) = edges_arr {
            for edge in edges {
                let src = edge
                    .get("source")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let dst = edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                if let (Some(s), Some(d)) = (src, dst) {
                    // Symmetric (legacy).
                    adjacency.entry(s.clone()).or_default().push(d.clone());
                    adjacency.entry(d.clone()).or_default().push(s.clone());
                    // Directed: on-disk order is caller→callee.
                    forward.entry(s.clone()).or_default().push(d.clone());
                    reverse.entry(d).or_default().push(s);
                }
            }
        }

        for neighbors in adjacency.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }
        for neighbors in forward.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }
        for neighbors in reverse.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }

        Ok(GraphifyReader {
            nodes,
            adjacency,
            forward,
            reverse,
        })
    }

    /// Total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Total number of undirected edges (each edge counted once).
    ///
    /// **Assumption:** The source `graph.json` contains no duplicate directed edges.
    /// If the same `{source, target}` pair appears more than once in the `links` array,
    /// this count will be inflated (each duplicate pair adds 2 to the adjacency sum).
    /// Graphify's own export format does not produce duplicates, so this is safe in practice.
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum::<usize>() / 2
    }

    /// BFS from one or more seed node IDs up to `max_depth` hops.
    ///
    /// `direction` selects callees ([`Direction::Out`]), callers ([`Direction::In`]),
    /// or the legacy undirected neighborhood ([`Direction::Both`]).
    pub fn bfs_from_seeds(
        &self,
        seeds: &[&str],
        max_depth: u8,
        limit: usize,
        direction: Direction,
    ) -> Vec<TraversalHit> {
        let adj = match direction {
            Direction::Out => &self.forward,
            Direction::In => &self.reverse,
            Direction::Both => &self.adjacency,
        };
        bfs::bfs_from_seeds(&self.nodes, adj, seeds, max_depth, limit)
    }

    /// Shortest path between two node IDs (BFS). Returns `None` if unreachable.
    pub fn shortest_path(&self, from: &str, to: &str, direction: Direction) -> Option<Vec<String>> {
        let adj = match direction {
            Direction::Out => &self.forward,
            Direction::In => &self.reverse,
            Direction::Both => &self.adjacency,
        };
        bfs::shortest_path(adj, from, to)
    }

    /// Nodes sorted by degree (highest first), capped at `top_n`.
    ///
    /// Returns `(node_id, degree)` pairs.
    pub fn god_nodes(&self, top_n: usize) -> Vec<(String, usize)> {
        let mut degrees: Vec<(String, usize)> = self
            .adjacency
            .iter()
            .map(|(id, neighbors)| (id.clone(), neighbors.len()))
            .collect();
        degrees.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        degrees.truncate(top_n);
        degrees
    }

    /// All node IDs belonging to `community_id` (matched on the `"community"` node field).
    pub fn community_members(&self, community_id: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter_map(|(id, (_, comm))| {
                comm.as_deref()
                    .filter(|c| *c == community_id)
                    .map(|_| id.clone())
            })
            .collect()
    }
}

/// BLAKE3 hex digest of graph bytes. The single source of truth for `graph_json_sha256`
/// (rebuild) and `lexical_ingest_sha256` (ingest) so `lexical_lag` comparisons are valid.
pub fn graph_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod directed_tests {
    use crate::{Direction, GraphifyReader};

    fn fixture() -> GraphifyReader {
        // A→B→C, plus a stray D→B. Storage order is caller→callee.
        let value = serde_json::json!({
            "nodes": [
                {"id": "A", "label": "A"}, {"id": "B", "label": "B"},
                {"id": "C", "label": "C"}, {"id": "D", "label": "D"}
            ],
            "links": [
                {"source": "A", "target": "B"},
                {"source": "B", "target": "C"},
                {"source": "D", "target": "B"}
            ]
        });
        GraphifyReader::from_value(value).expect("reader builds")
    }

    #[test]
    fn callees_of_b_is_c_only() {
        let ids: Vec<_> = fixture()
            .bfs_from_seeds(&["B"], 1, 100, Direction::Out)
            .iter()
            .map(|h| h.node_id.clone())
            .collect();
        assert_eq!(ids, vec!["C".to_string()]);
    }

    #[test]
    fn callers_of_b_are_a_and_d() {
        let mut ids: Vec<_> = fixture()
            .bfs_from_seeds(&["B"], 1, 100, Direction::In)
            .iter()
            .map(|h| h.node_id.clone())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["A".to_string(), "D".to_string()]);
    }

    #[test]
    fn directed_path_respects_direction() {
        let r = fixture();
        assert!(r.shortest_path("A", "C", Direction::Out).is_some());
        assert!(r.shortest_path("C", "A", Direction::In).is_some());
        assert!(
            r.shortest_path("A", "C", Direction::In).is_none(),
            "regression guard: callers-direction must not reach forward"
        );
    }

    #[test]
    fn both_preserves_legacy_undirected_neighborhood() {
        let mut ids: Vec<_> = fixture()
            .bfs_from_seeds(&["B"], 1, 100, Direction::Both)
            .iter()
            .map(|h| h.node_id.clone())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["A".to_string(), "C".to_string(), "D".to_string()]);
    }
}
