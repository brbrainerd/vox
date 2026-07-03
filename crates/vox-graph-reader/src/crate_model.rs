use std::collections::{HashMap, HashSet};

use crate::cluster::{ClusterEdge, ClusterNode, cluster_nodes};
use serde_json::{Value, json};

/// Crates that are deliberate, workspace-wide coupling (feature unification,
/// build-script guards, etc.) and are never a valid cut/removal candidate —
/// the single source of truth shared by `what_if::top_cuts` and
/// `edge_weights::weigh_edges` so the two analyses can't drift apart.
pub const NEVER_REMOVAL_CANDIDATES: &[&str] = &["workspace-hack"];

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
        let mut dep_vals: Vec<f64> = seen
            .iter()
            .map(|x| self_s.get(x).copied().unwrap_or(0.0))
            .collect();
        dep_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let dep_sum: f64 = dep_vals.iter().sum();
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

/// Build a deterministic graphify-shaped crate map from `crate-graph.v1.json`
/// (`{crates:{name:[deps]}}`) and the `crate_audit.json` array
/// (`crate`, `compile_s` [string or number], `loc`, `layer`).
///
/// Node attrs: `compile_s`, `loc`, `layer`, `fan_in`, `dependents`, `blast_s`, `community`.
/// When `audit` is empty (`[]`), times degrade to 0 and `dependents` still ranks crates by impact.
pub fn build_crate_map(crate_graph: &Value, audit: &Value) -> Value {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            let deps = ds
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            adj.insert(c.clone(), deps);
        }
    }

    let (mut self_s, mut loc_map, mut layer_map): (
        HashMap<String, f64>,
        HashMap<String, i64>,
        HashMap<String, i64>,
    ) = (HashMap::new(), HashMap::new(), HashMap::new());
    if let Some(arr) = audit.as_array() {
        for r in arr {
            let Some(name) = r.get("crate").and_then(|v| v.as_str()) else {
                continue;
            };
            let cs = r
                .get("compile_s")
                .and_then(|v| {
                    v.as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .or_else(|| v.as_f64())
                })
                .unwrap_or(0.0);
            self_s.insert(name.to_string(), cs);
            loc_map.insert(
                name.to_string(),
                r.get("loc").and_then(|v| v.as_i64()).unwrap_or(0),
            );
            layer_map.insert(
                name.to_string(),
                r.get("layer").and_then(|v| v.as_i64()).unwrap_or(-1),
            );
        }
    }

    let metrics = crate_metrics(&adj, &self_s);

    let mut nodes_set: HashSet<String> = HashSet::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    for (c, deps) in &adj {
        nodes_set.insert(c.clone());
        for d in deps {
            nodes_set.insert(d.clone());
            *fan_in.entry(d.clone()).or_insert(0) += 1;
        }
    }

    let mut sorted_nodes: Vec<String> = nodes_set.iter().cloned().collect();
    sorted_nodes.sort();
    let cnodes: Vec<ClusterNode> = sorted_nodes
        .iter()
        .map(|n| ClusterNode {
            id: n.clone(),
            label: n.clone(),
        })
        .collect();
    let mut cedges: Vec<ClusterEdge> = Vec::new();
    for (c, deps) in &adj {
        for d in deps {
            cedges.push(ClusterEdge {
                source: c.clone(),
                target: d.clone(),
            });
        }
    }
    cedges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));
    let comm = cluster_nodes(&cnodes, &cedges);

    let mut names: Vec<String> = nodes_set.into_iter().collect();
    names.sort();
    let nodes_val: Vec<Value> = names
        .iter()
        .map(|n| {
            let cs = (self_s.get(n).copied().unwrap_or(0.0) * 10.0).round() / 10.0;
            let mm = metrics.get(n);
            json!({
                "id": n, "label": n,
                "community": comm.get(n).cloned().unwrap_or_else(|| "c_0".to_string()),
                "compile_s": cs,
                "loc": loc_map.get(n).copied().unwrap_or(0),
                "layer": layer_map.get(n).copied().unwrap_or(-1),
                "fan_in": fan_in.get(n).copied().unwrap_or(0),
                "dependents": mm.map(|m| m.dependents).unwrap_or(0),
                "blast_s": mm.map(|m| m.blast_s).unwrap_or(0.0).round(),
            })
        })
        .collect();

    let mut links_val: Vec<Value> = Vec::new();
    for (c, deps) in &adj {
        for d in deps {
            links_val.push(json!({"source": c, "target": d}));
        }
    }
    links_val.sort_by_key(|a| a.to_string());

    json!({ "nodes": nodes_val, "links": links_val })
}

/// Build the small, committed crate-build SSOT (`contracts/ci/crate-build-map.v1.json`).
///
/// `crate_graph` is the `{crates:{name:[deps]}}` shape from `crate-graph.v1.json`.
/// `compile_times` maps crate name -> self compile seconds (audit native precision; may be partial/empty).
///
/// Output embeds `compile_s` (the periodically-refreshed INPUT) plus the DERIVED
/// `dependents`/`blast_s`/`fan_in`, so the parity gate can recompute the derived fields from
/// `crate_graph` + the embedded `compile_s` and detect drift. Deterministic: crates sorted
/// alphabetically; `blast_s` rounded to whole seconds; `compile_s` kept at input precision.
pub fn build_crate_summary(crate_graph: &Value, compile_times: &HashMap<String, f64>) -> Value {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            nodes.insert(c.clone());
            let deps: Vec<String> = ds
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            for d in &deps {
                nodes.insert(d.clone());
                *fan_in.entry(d.clone()).or_insert(0) += 1;
            }
            adj.insert(c.clone(), deps);
        }
    }

    let metrics = crate_metrics(&adj, compile_times);

    let mut names: Vec<String> = nodes.into_iter().collect();
    names.sort();

    let mut without = 0usize;
    let crates_val: Vec<Value> = names
        .iter()
        .map(|n| {
            let cs = compile_times.get(n).copied();
            if cs.is_none() {
                without += 1;
            }
            let m = metrics.get(n);
            json!({
                "crate": n,
                "compile_s": cs.unwrap_or(0.0),
                "dependents": m.map(|x| x.dependents).unwrap_or(0),
                "blast_s": m.map(|x| x.blast_s).unwrap_or(0.0).round(),
                "fan_in": fan_in.get(n).copied().unwrap_or(0),
            })
        })
        .collect();

    json!({
        "schema_version": 1,
        "has_compile_times": !compile_times.is_empty(),
        "crates_without_compile_times": without,
        "crates": crates_val,
    })
}
