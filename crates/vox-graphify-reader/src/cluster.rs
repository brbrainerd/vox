use leiden_rs::{GraphDataBuilder, Leiden, LeidenConfig};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ClusterNode {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct ClusterEdge {
    pub source: String,
    pub target: String,
}

pub fn cluster_nodes(
    nodes: &[ClusterNode],
    edges: &[ClusterEdge],
) -> HashMap<String, String> {
    if nodes.is_empty() {
        return HashMap::new();
    }

    let mut builder = GraphDataBuilder::new(nodes.len());
    let mut node_indices = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        node_indices.insert(node.id.clone(), i);
    }

    for edge in edges {
        if let (Some(&s), Some(&t)) = (node_indices.get(&edge.source), node_indices.get(&edge.target)) {
            let _ = builder.add_edge(s, t, 1.0);
        }
    }

    let graph = builder.build().unwrap_or_else(|_| GraphDataBuilder::new(nodes.len()).build().unwrap());
    let leiden = Leiden::new(LeidenConfig::default());
    
    let mut communities = HashMap::new();
    if let Ok(result) = leiden.run(&graph) {
        for (node_idx, community_id) in result.partition.iter() {
            if node_idx < nodes.len() {
                let node_id = &nodes[node_idx].id;
                communities.insert(node_id.clone(), format!("c_{}", community_id));
            }
        }
    }

    // Ensure all input nodes have a community ID assigned
    for node in nodes {
        communities.entry(node.id.clone()).or_insert_with(|| "c_0".to_string());
    }

    communities
}
