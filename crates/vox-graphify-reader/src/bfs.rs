//! BFS traversal and shortest-path search over a HashMap adjacency index.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::TraversalHit;

/// BFS expansion from seed nodes. Seeds are excluded from results.
pub(crate) fn bfs_from_seeds(
    nodes: &HashMap<String, (String, Option<String>)>,
    adjacency: &HashMap<String, Vec<String>>,
    seeds: &[&str],
    max_depth: u8,
    limit: usize,
) -> Vec<TraversalHit> {
    let env_cap = std::env::var("VOX_GRAPHIFY_VIZ_NODE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    let effective_limit = limit.min(env_cap);

    if effective_limit == 0 || max_depth == 0 {
        return vec![];
    }

    let mut visited: HashSet<String> = HashSet::new();

    // Queue entry: (node_id, depth, path_from_seed_to_this_node)
    let mut queue: VecDeque<(String, u8, Vec<String>)> = VecDeque::new();

    // Multi-source BFS: enqueue EVERY seed at depth 0 and mark visited before
    // any expansion runs. This guarantees each node's reported depth/path is the
    // minimum hops from the NEAREST seed, not merely the first seed to reach it.
    // A node 1 hop from seed B is discovered at depth 1 even if seed A would
    // otherwise reach it at depth 2.
    for &seed in seeds {
        if visited.insert(seed.to_string()) {
            queue.push_back((seed.to_string(), 0, vec![seed.to_string()]));
        }
    }

    let mut results = Vec::new();

    while let Some((node_id, depth, path)) = queue.pop_front() {
        if results.len() >= effective_limit {
            break;
        }

        // Seeds are excluded from results, but still expanded as depth-0 frontier.
        if depth > 0 {
            if let Some((label, _)) = nodes.get(&node_id) {
                results.push(TraversalHit {
                    node_id: node_id.clone(),
                    label: label.clone(),
                    depth,
                    path: path.clone(),
                });
            }
        }

        if depth < max_depth {
            if let Some(neighbors) = adjacency.get(&node_id) {
                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        let mut next_path = path.clone();
                        next_path.push(neighbor.clone());
                        queue.push_back((neighbor.clone(), depth + 1, next_path));
                    }
                }
            }
        }
    }

    results
}

/// BFS shortest path from `from` to `to`. Returns `None` if unreachable.
pub(crate) fn shortest_path(
    adjacency: &HashMap<String, Vec<String>>,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![from.to_string()]);
    }

    let mut visited = HashSet::new();
    visited.insert(from.to_string());
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((from.to_string(), vec![from.to_string()]));

    while let Some((node, path)) = queue.pop_front() {
        if let Some(neighbors) = adjacency.get(&node) {
            for neighbor in neighbors {
                if neighbor == to {
                    let mut result = path.clone();
                    result.push(to.to_string());
                    return Some(result);
                }
                if visited.insert(neighbor.clone()) {
                    let mut next_path = path.clone();
                    next_path.push(neighbor.clone());
                    queue.push_back((neighbor.clone(), next_path));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::GraphifyReader;

    /// Finding I3: multi-source BFS must report depth from the NEAREST seed.
    /// X is 1 hop from seed B (B-X) but 2 hops from seed A (A-M-X). With seeds
    /// [A, B] enqueued at depth 0, X must be reported at depth 1, not 2.
    #[test]
    fn multi_seed_reports_nearest_seed_depth() {
        let value = serde_json::json!({
            "nodes": [
                {"id": "A", "label": "A"},
                {"id": "B", "label": "B"},
                {"id": "M", "label": "M"},
                {"id": "X", "label": "X"}
            ],
            "links": [
                {"source": "A", "target": "M"},
                {"source": "M", "target": "X"},
                {"source": "B", "target": "X"}
            ]
        });

        let reader = GraphifyReader::from_value(value).expect("reader builds");
        let hits = reader.bfs_from_seeds(&["A", "B"], 5, 100);

        let x = hits
            .iter()
            .find(|h| h.node_id == "X")
            .expect("X is reached");
        assert_eq!(
            x.depth, 1,
            "X must be reported at its nearest-seed depth (B-X)"
        );
        assert_eq!(
            x.path,
            vec!["B".to_string(), "X".to_string()],
            "path must originate from the nearest seed B"
        );

        // Seeds themselves are excluded from results.
        assert!(hits.iter().all(|h| h.node_id != "A" && h.node_id != "B"));
    }
}
