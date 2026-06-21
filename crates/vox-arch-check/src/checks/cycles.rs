//! Rule 17 — Iterative Tarjan SCC cycle detector for the workspace dep graph.
//!
//! Using an iterative (stack-based) implementation to avoid stack overflow on
//! large graphs; a 109-crate workspace fits comfortably but recursive DFS with
//! heavy debug binaries can hit Windows default-stack limits.

use std::collections::HashMap;

/// Find all strongly-connected components with ≥2 nodes (i.e., real cycles).
/// `edges` is a list of `(from, to)` pairs over workspace-member crate names.
/// Returns one `Vec<String>` per cycle, nodes in the order first discovered.
pub fn find_cycles(edges: &[(String, String)]) -> Vec<Vec<String>> {
    // Build adjacency list.
    let mut node_index: HashMap<&str, usize> = HashMap::new();
    let mut nodes: Vec<&str> = Vec::new();
    for (from, to) in edges {
        for name in [from.as_str(), to.as_str()] {
            if !node_index.contains_key(name) {
                node_index.insert(name, nodes.len());
                nodes.push(name);
            }
        }
    }
    let n = nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (from, to) in edges {
        let fi = node_index[from.as_str()];
        let ti = node_index[to.as_str()];
        adj[fi].push(ti);
    }

    // Iterative Tarjan.
    let mut index_counter = 0usize;
    let mut index: Vec<Option<usize>> = vec![None; n];
    let mut lowlink: Vec<usize> = vec![0; n];
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();

    // Per-node DFS state: (node, iterator-over-neighbours position).
    let mut call_stack: Vec<(usize, usize)> = Vec::new();
    let mut result: Vec<Vec<String>> = Vec::new();

    for start in 0..n {
        if index[start].is_some() {
            continue;
        }
        call_stack.push((start, 0));
        index[start] = Some(index_counter);
        lowlink[start] = index_counter;
        index_counter += 1;
        on_stack[start] = true;
        stack.push(start);

        while let Some((v, ni)) = call_stack.last_mut() {
            let v = *v;
            if *ni < adj[v].len() {
                let w = adj[v][*ni];
                *ni += 1;
                if index[w].is_none() {
                    index[w] = Some(index_counter);
                    lowlink[w] = index_counter;
                    index_counter += 1;
                    on_stack[w] = true;
                    stack.push(w);
                    call_stack.push((w, 0));
                } else if on_stack[w] {
                    let w_idx = index[w].unwrap();
                    if w_idx < lowlink[v] {
                        lowlink[v] = w_idx;
                    }
                }
            } else {
                call_stack.pop();
                if let Some(&(parent, _)) = call_stack.last() {
                    if lowlink[v] < lowlink[parent] {
                        lowlink[parent] = lowlink[v];
                    }
                }
                // SCC root?
                if lowlink[v] == index[v].unwrap() {
                    let mut component = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        component.push(nodes[w].to_string());
                        if w == v {
                            break;
                        }
                    }
                    if component.len() >= 2 {
                        component.sort();
                        result.push(component);
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(a: &str, b: &str) -> (String, String) {
        (a.to_string(), b.to_string())
    }

    #[test]
    fn acyclic_graph_has_no_cycles() {
        let edges = vec![e("a", "b"), e("b", "c"), e("a", "c")];
        assert!(find_cycles(&edges).is_empty());
    }

    #[test]
    fn two_node_cycle_is_detected() {
        let edges = vec![e("a", "b"), e("b", "a")];
        let cycles = find_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        let cycle = &cycles[0];
        assert!(cycle.contains(&"a".to_string()));
        assert!(cycle.contains(&"b".to_string()));
    }

    #[test]
    fn three_node_cycle_is_detected() {
        let edges = vec![e("a", "b"), e("b", "c"), e("c", "a")];
        let cycles = find_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
    }

    #[test]
    fn self_loop_is_not_reported_as_cycle() {
        // Cargo doesn't produce self-edges; make sure we don't crash on one.
        let edges = vec![e("a", "a")];
        // A self-loop is technically a 1-node SCC, but we filter for len >= 2.
        let cycles = find_cycles(&edges);
        assert!(cycles.is_empty());
    }

    #[test]
    fn two_independent_cycles_both_detected() {
        let edges = vec![e("a", "b"), e("b", "a"), e("c", "d"), e("d", "c")];
        let cycles = find_cycles(&edges);
        assert_eq!(cycles.len(), 2);
    }
}
