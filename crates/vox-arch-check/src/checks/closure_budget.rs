//! Rule 19 — Dependency-closure-size budget.
//!
//! Tracks the number of reachable workspace crates in the transitive
//! normal-dep closure of budgeted crates. This is a deterministic
//! build-speed proxy (more workspace deps = more crates to compile in
//! a cold build), unlike wall-clock time which varies by machine.
//!
//! Budgets are committed in `contracts/reports/closure-budgets.v1.json`
//! and loaded at check time.

use std::collections::{HashMap, HashSet, VecDeque};

pub struct Budget {
    pub crate_name: String,
    pub max_closure: usize,
}

/// Compute the workspace-dep closure size for each crate in `budgets`.
/// `edges` is the list of `(from, to)` normal workspace-dep pairs.
/// Returns error messages for crates that exceed their budget.
pub fn check(edges: &[(String, String)], budgets: &[Budget]) -> Vec<String> {
    // Build adjacency list.
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for (from, to) in edges {
        adj.entry(from.as_str()).or_default().push(to.as_str());
    }

    let mut violations: Vec<String> = Vec::new();
    for budget in budgets {
        let actual = closure_size(budget.crate_name.as_str(), &adj);
        if actual > budget.max_closure {
            violations.push(format!(
                "Rule 19: `{}` closure size {} exceeds budget {} (add {} deps)",
                budget.crate_name,
                actual,
                budget.max_closure,
                actual - budget.max_closure,
            ));
        }
    }
    violations
}

fn closure_size(start: &str, adj: &HashMap<&str, Vec<&str>>) -> usize {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    queue.push_back(start);
    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) {
            continue;
        }
        if let Some(deps) = adj.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    queue.push_back(dep);
                }
            }
        }
    }
    // Exclude the root crate itself from the count.
    visited.len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(a: &str, b: &str) -> (String, String) {
        (a.to_string(), b.to_string())
    }

    fn b(name: &str, max: usize) -> Budget {
        Budget {
            crate_name: name.to_string(),
            max_closure: max,
        }
    }

    #[test]
    fn crate_within_budget_passes() {
        let edges = vec![e("vox-cli", "vox-db"), e("vox-db", "vox-primitives")];
        let budgets = vec![b("vox-cli", 10)];
        assert!(check(&edges, &budgets).is_empty());
    }

    #[test]
    fn crate_over_budget_is_reported() {
        let edges = vec![
            e("vox-cli", "dep-a"),
            e("vox-cli", "dep-b"),
            e("vox-cli", "dep-c"),
        ];
        let budgets = vec![b("vox-cli", 2)];
        let violations = check(&edges, &budgets);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("vox-cli"), "{}", violations[0]);
        assert!(violations[0].contains("3"), "{}", violations[0]);
    }

    #[test]
    fn transitive_deps_are_counted() {
        let edges = vec![e("root", "mid"), e("mid", "leaf")];
        let budgets = vec![b("root", 1)];
        let violations = check(&edges, &budgets);
        assert_eq!(violations.len(), 1, "transitive dep should be counted");
    }

    #[test]
    fn exactly_at_budget_passes() {
        let edges = vec![e("root", "mid"), e("mid", "leaf")];
        let budgets = vec![b("root", 2)];
        assert!(
            check(&edges, &budgets).is_empty(),
            "exactly at budget should pass"
        );
    }

    #[test]
    fn unknown_crate_has_zero_closure() {
        let edges: Vec<(String, String)> = vec![];
        let budgets = vec![b("nonexistent", 0)];
        assert!(
            check(&edges, &budgets).is_empty(),
            "unknown crate = 0 closure = passes budget 0"
        );
    }
}
