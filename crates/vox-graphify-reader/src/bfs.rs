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

    let seed_set: HashSet<String> = seeds.iter().map(|s| s.to_string()).collect();
    let mut visited: HashSet<String> = seed_set.clone();

    // Queue entry: (node_id, depth, path_from_seed_to_this_node)
    let mut queue: VecDeque<(String, u8, Vec<String>)> = VecDeque::new();

    for &seed in seeds {
        if let Some(neighbors) = adjacency.get(seed) {
            for neighbor in neighbors {
                if visited.insert(neighbor.clone()) {
                    queue.push_back((
                        neighbor.clone(),
                        1,
                        vec![seed.to_string(), neighbor.clone()],
                    ));
                }
            }
        }
    }

    let mut results = Vec::new();

    while let Some((node_id, depth, path)) = queue.pop_front() {
        if results.len() >= effective_limit {
            break;
        }

        if let Some((label, _)) = nodes.get(&node_id) {
            results.push(TraversalHit {
                node_id: node_id.clone(),
                label: label.clone(),
                depth,
                path: path.clone(),
            });
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
