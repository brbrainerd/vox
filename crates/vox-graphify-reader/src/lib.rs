//! Read-only graph reader for Graphify `graph.json` (NetworkX JSON export format).
//!
//! # Graph format
//! ```json
//! { "nodes": [{"id": "x", "label": "...", "community": "c1"}],
//!   "links": [{"source": "x", "target": "y"}] }
//! ```
//! Edges may appear under `"links"` or `"edges"` — both are supported.
//! The graph is treated as **undirected**: edges are indexed in both directions.

pub mod bfs;
pub mod compare;

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
                    adjacency.entry(s.clone()).or_default().push(d.clone());
                    adjacency.entry(d).or_default().push(s);
                }
            }
        }

        Ok(GraphifyReader { nodes, adjacency })
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
    /// Seeds themselves are excluded from the output — only their reachable neighbors are
    /// returned. Results are capped at `limit`. If the `VOX_GRAPHIFY_VIZ_NODE_LIMIT` env var
    /// is set and lower than `limit`, that cap applies instead.
    pub fn bfs_from_seeds(&self, seeds: &[&str], max_depth: u8, limit: usize) -> Vec<TraversalHit> {
        bfs::bfs_from_seeds(&self.nodes, &self.adjacency, seeds, max_depth, limit)
    }

    /// Shortest path between two node IDs (BFS). Returns `None` if unreachable.
    ///
    /// Returns `Some(vec![node_id])` (single element) when `from == to`.
    pub fn shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        bfs::shortest_path(&self.adjacency, from, to)
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
