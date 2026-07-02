//! What-if simulation over the crate dependency graph: cut one edge or split a
//! crate, recompute blast_s/dependents via `crate_model::crate_metrics`, and
//! report deltas. Pure functions; no I/O.

use std::collections::HashMap;

use crate::crate_model::{CrateMetrics, crate_metrics};
use serde::Serialize;

/// Cut targets excluded from `top_cuts` recommendations by default:
/// deliberate-coupling crates where "cut this edge" is an anti-goal.
pub const DEFAULT_EXCLUDED_CUT_TARGETS: &[&str] = &["workspace-hack"];

/// One crate whose metrics change under a hypothetical edit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrateDelta {
    pub krate: String,
    pub blast_s_before: f64,
    pub blast_s_after: f64,
    pub dependents_before: usize,
    pub dependents_after: usize,
}

/// Result of one hypothetical edit. `total_blast_s_*` sums blast_s over every
/// crate — a comparative index, not wall-clock. Self-time attribution for
/// splits stays with the original crate (the synthetic `__split` node has 0
/// self time), so split savings are an UPPER BOUND on the dependency-shape win.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WhatIfDelta {
    pub description: String,
    pub total_blast_s_before: f64,
    pub total_blast_s_after: f64,
    /// Crates whose blast_s or dependents changed, sorted by blast saving desc.
    pub changed: Vec<CrateDelta>,
}

fn total(m: &HashMap<String, CrateMetrics>) -> f64 {
    m.values().map(|x| x.blast_s).sum()
}

fn diff(
    description: String,
    before: &HashMap<String, CrateMetrics>,
    after: &HashMap<String, CrateMetrics>,
) -> WhatIfDelta {
    let zero = CrateMetrics {
        dependents: 0,
        blast_s: 0.0,
    };
    let mut changed: Vec<CrateDelta> = Vec::new();
    let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
    names.sort();
    names.dedup();
    for n in names {
        let b = before.get(n).unwrap_or(&zero);
        let a = after.get(n).unwrap_or(&zero);
        if b.blast_s != a.blast_s || b.dependents != a.dependents {
            changed.push(CrateDelta {
                krate: n.clone(),
                blast_s_before: b.blast_s,
                blast_s_after: a.blast_s,
                dependents_before: b.dependents,
                dependents_after: a.dependents,
            });
        }
    }
    changed.sort_by(|x, y| {
        let sx = x.blast_s_before - x.blast_s_after;
        let sy = y.blast_s_before - y.blast_s_after;
        sy.partial_cmp(&sx)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(x.krate.cmp(&y.krate))
    });
    WhatIfDelta {
        description,
        total_blast_s_before: total(before),
        total_blast_s_after: total(after),
        changed,
    }
}

/// Remove the dependency edge `from -> to` and report metric deltas.
pub fn what_if_cut(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    from: &str,
    to: &str,
) -> Result<WhatIfDelta, String> {
    let deps = adj
        .get(from)
        .ok_or_else(|| format!("unknown crate '{from}'"))?;
    if !deps.iter().any(|d| d == to) {
        return Err(format!("no dependency edge {from} -> {to}"));
    }
    let before = crate_metrics(adj, self_s);
    let mut cut = adj.clone();
    if let Some(v) = cut.get_mut(from) {
        v.retain(|d| d != to);
    }
    let after = crate_metrics(&cut, self_s);
    Ok(diff(format!("cut {from} -> {to}"), &before, &after))
}

/// Model extracting the part of `krate` that uses `moved` deps into a new leaf
/// crate `<krate>__split` (which nobody depends on). `krate` keeps its
/// dependents and self time; it just stops depending on `moved`.
pub fn what_if_split(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    krate: &str,
    moved: &[String],
) -> Result<WhatIfDelta, String> {
    let deps = adj
        .get(krate)
        .ok_or_else(|| format!("unknown crate '{krate}'"))?;
    for m in moved {
        if !deps.iter().any(|d| d == m) {
            return Err(format!("'{krate}' does not depend on '{m}'"));
        }
    }
    let before = crate_metrics(adj, self_s);
    let mut split = adj.clone();
    if let Some(v) = split.get_mut(krate) {
        v.retain(|d| !moved.iter().any(|m| m == d));
    }
    split.insert(format!("{krate}__split"), moved.to_vec());
    let after = crate_metrics(&split, self_s);
    Ok(diff(
        format!(
            "split {krate}: move deps [{}] to {krate}__split",
            moved.join(", ")
        ),
        &before,
        &after,
    ))
}

/// Evaluate cutting every existing edge (skipping edges whose TARGET is in
/// `exclude_targets`), return the `n` best by total blast saved.
// ponytail: brute force — one crate_metrics per edge. 121 crates / 593 edges is
// instant; revisit with incremental recompute only if the workspace 10×es.
pub fn top_cuts(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    n: usize,
    exclude_targets: &[String],
) -> Vec<WhatIfDelta> {
    let mut edges: Vec<(String, String)> = Vec::new();
    for (c, deps) in adj {
        for d in deps {
            if exclude_targets.iter().any(|x| x == d) {
                continue;
            }
            edges.push((c.clone(), d.clone()));
        }
    }
    edges.sort();
    let mut out: Vec<WhatIfDelta> = edges
        .iter()
        .filter_map(|(a, b)| what_if_cut(adj, self_s, a, b).ok())
        .collect();
    out.sort_by(|x, y| {
        let sx = x.total_blast_s_before - x.total_blast_s_after;
        let sy = y.total_blast_s_before - y.total_blast_s_after;
        sy.partial_cmp(&sx)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(x.description.cmp(&y.description))
    });
    out.truncate(n);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// b and c each take 10s; a takes 1s. a -> b -> c.
    /// blast(c) = 10 + 10 + 1 = 21 (self + b + a); blast(b) = 10 + 1 = 11; blast(a) = 1.
    fn toy() -> (HashMap<String, Vec<String>>, HashMap<String, f64>) {
        let adj = HashMap::from([
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["c".to_string()]),
        ]);
        let self_s = HashMap::from([
            ("a".to_string(), 1.0),
            ("b".to_string(), 10.0),
            ("c".to_string(), 10.0),
        ]);
        (adj, self_s)
    }

    #[test]
    fn cut_edge_recomputes_blast() {
        let (adj, self_s) = toy();
        let d = what_if_cut(&adj, &self_s, "a", "b").unwrap();
        // Cutting a->b: blast(b) = 10 (loses a), blast(c) = 20 (loses a).
        assert_eq!(d.total_blast_s_before, 21.0 + 11.0 + 1.0);
        assert_eq!(d.total_blast_s_after, 20.0 + 10.0 + 1.0);
        let b = d.changed.iter().find(|c| c.krate == "b").unwrap();
        assert_eq!(b.blast_s_before, 11.0);
        assert_eq!(b.blast_s_after, 10.0);
        assert_eq!(b.dependents_before, 1);
        assert_eq!(b.dependents_after, 0);
        // 'a' unchanged -> not listed.
        assert!(d.changed.iter().all(|c| c.krate != "a"));
    }

    #[test]
    fn cut_missing_edge_errors() {
        let (adj, self_s) = toy();
        assert!(what_if_cut(&adj, &self_s, "a", "c").is_err());
        assert!(what_if_cut(&adj, &self_s, "nope", "b").is_err());
    }

    #[test]
    fn split_moves_dep_edges_to_new_leaf_node() {
        let (adj, self_s) = toy();
        // Split b: the part of b that uses c moves out. b no longer depends on c;
        // b__split (nobody depends on it) takes the c edge.
        let d = what_if_split(&adj, &self_s, "b", &["c".to_string()]).unwrap();
        // After: blast(c) = 10 (only itself; b__split has 0 self time).
        let c = d.changed.iter().find(|x| x.krate == "c").unwrap();
        assert_eq!(c.blast_s_before, 21.0);
        assert_eq!(c.blast_s_after, 10.0);
        assert_eq!(c.dependents_after, 1); // b__split
    }

    #[test]
    fn split_validates_moved_deps() {
        let (adj, self_s) = toy();
        assert!(what_if_split(&adj, &self_s, "b", &["zzz".to_string()]).is_err());
        assert!(what_if_split(&adj, &self_s, "zzz", &["c".to_string()]).is_err());
    }

    #[test]
    fn top_cuts_ranks_by_total_saving() {
        let (adj, self_s) = toy();
        let cuts = top_cuts(&adj, &self_s, 10, &[]);
        assert_eq!(cuts.len(), 2); // two edges exist
        // Cutting b->c saves blast(c): 21 -> 10 = 11s. Cutting a->b saves 2s total.
        assert_eq!(cuts[0].description, "cut b -> c");
        let s0 = cuts[0].total_blast_s_before - cuts[0].total_blast_s_after;
        let s1 = cuts[1].total_blast_s_before - cuts[1].total_blast_s_after;
        assert!(s0 >= s1);
    }

    #[test]
    fn top_cuts_excludes_listed_targets() {
        // workspace-hack-shaped case: high fan-in target that must never be a
        // recommendation. Exclusion is by TARGET name.
        let adj = HashMap::from([
            (
                "a".to_string(),
                vec!["workspace-hack".to_string(), "b".to_string()],
            ),
            ("b".to_string(), vec!["workspace-hack".to_string()]),
        ]);
        let self_s = HashMap::from([
            ("workspace-hack".to_string(), 100.0),
            ("a".to_string(), 1.0),
            ("b".to_string(), 1.0),
        ]);
        let cuts = top_cuts(&adj, &self_s, 10, &["workspace-hack".to_string()]);
        assert!(
            cuts.iter()
                .all(|c| !c.description.contains("workspace-hack"))
        );
        assert_eq!(cuts.len(), 1); // only a->b survives
    }

    #[test]
    fn cycle_safe() {
        // a <-> b cycle plus times; must terminate and produce numbers.
        let adj = HashMap::from([
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["a".to_string()]),
        ]);
        let self_s = HashMap::from([("a".to_string(), 1.0), ("b".to_string(), 2.0)]);
        let d = what_if_cut(&adj, &self_s, "a", "b").unwrap();
        assert!(d.total_blast_s_after <= d.total_blast_s_before);
    }
}
