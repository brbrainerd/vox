//! Semantic lenses: post-process a structural graph into a different "semantic line".
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn module_of(id: &str) -> &str {
    id.rsplit_once("::").map(|(m, _)| m).unwrap_or(id)
}

/// Collapse a `module::symbol` graph into a module-level graph: one node per module, one
/// weighted edge per inter-module call relationship. Intra-module edges drop. Both a
/// distinct semantic line and the coarse view that keeps very large repos navigable.
pub fn collapse_to_modules(graph: &Value) -> Value {
    let empty = vec![];
    let nodes = graph
        .get("nodes")
        .and_then(|n| n.as_array())
        .unwrap_or(&empty);
    let links = graph
        .get("links")
        .or_else(|| graph.get("edges"))
        .and_then(|n| n.as_array())
        .unwrap_or(&empty);

    let mut modules: HashSet<String> = HashSet::new();
    for n in nodes {
        if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
            modules.insert(module_of(id).to_string());
        }
    }
    let mut weights: HashMap<(String, String), u64> = HashMap::new();
    for l in links {
        if let (Some(s), Some(t)) = (
            l.get("source").and_then(|v| v.as_str()),
            l.get("target").and_then(|v| v.as_str()),
        ) {
            let (sm, tm) = (module_of(s).to_string(), module_of(t).to_string());
            if sm != tm {
                *weights.entry((sm, tm)).or_insert(0) += 1;
            }
        }
    }
    let mut module_list: Vec<String> = modules.into_iter().collect();
    module_list.sort();
    let nodes_val: Vec<Value> = module_list
        .into_iter()
        .map(|id| json!({"id": id, "label": id, "kind": "module", "community": "c_0"}))
        .collect();
    let mut edge_list: Vec<((String, String), u64)> = weights.into_iter().collect();
    edge_list.sort();
    let links_val: Vec<Value> = edge_list
        .into_iter()
        .map(|((s, t), w)| json!({"source": s, "target": t, "weight": w}))
        .collect();
    json!({"nodes": nodes_val, "links": links_val})
}
