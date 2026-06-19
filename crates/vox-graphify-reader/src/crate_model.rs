use std::collections::{HashMap, HashSet};

/// Per-crate model metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct CrateMetrics {
    /// Count of crates that transitively depend on this one.
    pub dependents: usize,
    /// self_s + sum of self_s over transitive dependents (0.0 when times are unavailable).
    pub blast_s: f64,
}

/// Compute per-crate metrics from `adj` (crate -> its deps) and `self_s` (crate -> compile seconds).
/// `self_s` may be empty or partial; `blast_s` degrades to 0 while `dependents` stays meaningful.
/// Cycle-safe: uses a visited set in the reverse-BFS.
pub fn crate_metrics(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
) -> HashMap<String, CrateMetrics> {
    // Build reverse adjacency (dep -> [crates that depend on dep]) and collect all nodes.
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    for (c, deps) in adj {
        nodes.insert(c.clone());
        for d in deps {
            nodes.insert(d.clone());
            rev.entry(d.clone()).or_default().push(c.clone());
        }
    }
    // For each node, BFS upward through rev to collect all transitive dependents.
    let mut out = HashMap::new();
    for n in &nodes {
        // Seed seen with n so the node never counts itself as its own dependent.
        let mut seen: HashSet<String> = HashSet::from([n.clone()]);
        let mut stack = vec![n.clone()];
        while let Some(x) = stack.pop() {
            if let Some(parents) = rev.get(&x) {
                for p in parents {
                    if seen.insert(p.clone()) {
                        stack.push(p.clone());
                    }
                }
            }
        }
        // Remove n itself (was seeded to prevent self-loops from inflating the count).
        seen.remove(n);
        let base = self_s.get(n).copied().unwrap_or(0.0);
        let dep_sum: f64 = seen
            .iter()
            .map(|x| self_s.get(x).copied().unwrap_or(0.0))
            .sum();
        out.insert(
            n.clone(),
            CrateMetrics {
                dependents: seen.len(),
                blast_s: base + dep_sum,
            },
        );
    }
    out
}
