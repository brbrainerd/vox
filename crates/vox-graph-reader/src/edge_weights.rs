//! Symbol-weighted crate dependency edges: join the repo-code-graph corpus
//! (native schema: node ids are `<path>::<symbol>`, edges carry `confidence`)
//! against the crate dependency adjacency to count how many distinct
//! target-crate symbols each declared dep edge actually uses.
//!
//! Honesty contract: the corpus is PARTIAL and its resolver drops ambiguous
//! names (undercount) while a globally-unique name can be misattributed
//! cross-crate (rare inflation). Therefore:
//! - `symbols_used == 0` is only ever "candidate — verify by removal";
//! - rows where either endpoint crate has < LOW_VISIBILITY_MIN extracted
//!   symbols are flagged `low_visibility` and never candidates;
//! - `workspace-hack` (deliberate feature-unification) is never a candidate.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Value, json};

/// A crate with fewer extracted symbols than this is invisible to the corpus:
/// its zero-weight edges mean "not extracted", not "not used".
pub const LOW_VISIBILITY_MIN: usize = 10;
const SAMPLE_CAP: usize = 20;
/// Deliberate-coupling dep targets that are never removal candidates.
const NEVER_CANDIDATES: &[&str] = &["workspace-hack"];

/// `"crates/<name>/src/lib.rs::sym"` -> `Some("<name>")`.
fn crate_of(node_id: &str) -> Option<&str> {
    let path = node_id.split("::").next().unwrap_or("");
    path.strip_prefix("crates/")?.split('/').next()
}

fn symbol_of(node_id: &str) -> &str {
    node_id.rsplit("::").next().unwrap_or(node_id)
}

/// For every declared dep edge in `adj`, count distinct target-crate symbols
/// referenced from the source crate via resolved corpus edges. Rows are
/// emitted for ALL adjacency edges (including zero-weight), sorted by
/// (`target_blast_s` desc, `symbols_used` asc) — best cut candidates first.
pub fn weigh_edges(
    corpus: &Value,
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
) -> Value {
    // Per-crate extracted-symbol counts (visibility) from node ids.
    let mut node_count: HashMap<String, usize> = HashMap::new();
    if let Some(nodes) = corpus.get("nodes").and_then(|v| v.as_array()) {
        for n in nodes {
            if let Some(c) = n.get("id").and_then(|v| v.as_str()).and_then(crate_of) {
                *node_count.entry(c.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Distinct target symbol IDs per (from_crate, to_crate), resolved edges only.
    let mut used: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    let mut refs_in_dep_graph = 0u64;
    let mut refs_not_in_dep_graph = 0u64;
    let links = corpus
        .get("links")
        .or_else(|| corpus.get("edges"))
        .and_then(|v| v.as_array());
    if let Some(links) = links {
        for e in links {
            if e.get("confidence").and_then(|v| v.as_str()) != Some("resolved") {
                continue;
            }
            let (Some(s), Some(t)) = (
                e.get("source").and_then(|v| v.as_str()),
                e.get("target").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let (Some(cs), Some(ct)) = (crate_of(s), crate_of(t)) else {
                continue;
            };
            if cs == ct {
                continue;
            }
            let declared = adj
                .get(cs)
                .map(|deps| deps.iter().any(|d| d == ct))
                .unwrap_or(false);
            if declared {
                refs_in_dep_graph += 1;
                used.entry((cs.to_string(), ct.to_string()))
                    .or_default()
                    .insert(t.to_string());
            } else {
                refs_not_in_dep_graph += 1;
            }
        }
    }

    let metrics = crate::crate_model::crate_metrics(adj, self_s);
    let mut sorted_edges: Vec<(&String, &String)> = adj
        .iter()
        .flat_map(|(c, deps)| deps.iter().map(move |d| (c, d)))
        .collect();
    sorted_edges.sort();

    let mut rows: Vec<Value> = Vec::new();
    for (from, to) in sorted_edges {
        let syms = used.get(&(from.clone(), to.clone()));
        let count = syms.map(|s| s.len()).unwrap_or(0);
        let sample: Vec<&str> = syms
            .map(|s| s.iter().take(SAMPLE_CAP).map(|id| symbol_of(id)).collect())
            .unwrap_or_default();
        let low_visibility = node_count.get(from).copied().unwrap_or(0) < LOW_VISIBILITY_MIN
            || node_count.get(to).copied().unwrap_or(0) < LOW_VISIBILITY_MIN;
        let blast = metrics.get(to).map(|m| m.blast_s).unwrap_or(0.0);
        let mut row = json!({
            "from": from,
            "to": to,
            "symbols_used": count,
            "symbols_sample": sample,
            "target_blast_s": blast,
            "low_visibility": low_visibility,
        });
        if count == 0 && !low_visibility && !NEVER_CANDIDATES.contains(&to.as_str()) {
            row["status"] = json!("candidate-unused — verify by removal");
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| {
        let ba = a["target_blast_s"].as_f64().unwrap_or(0.0);
        let bb = b["target_blast_s"].as_f64().unwrap_or(0.0);
        bb.partial_cmp(&ba)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a["symbols_used"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&b["symbols_used"].as_u64().unwrap_or(0))
            })
            .then_with(|| a["from"].as_str().cmp(&b["from"].as_str()))
            .then_with(|| a["to"].as_str().cmp(&b["to"].as_str()))
    });

    let candidates = rows.iter().filter(|r| r.get("status").is_some()).count();
    json!({
        "schema_version": 1,
        "meta": {
            "crates_with_symbols": node_count.len(),
            "low_visibility_min": LOW_VISIBILITY_MIN,
            "refs_in_dep_graph": refs_in_dep_graph,
            "refs_not_in_dep_graph": refs_not_in_dep_graph,
            "candidate_count": candidates,
            "note": "corpus is partial (resolver drops ambiguous names); zero-weight = candidate only",
        },
        "edges": rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn corpus() -> serde_json::Value {
        // aaa has 12 nodes (well-covered), bbb has 11, ddd has 1 (low-visibility).
        let mut nodes = vec![
            json!({"id": "crates/bbb/src/lib.rs::callee", "label": "callee", "kind": "fn"}),
            json!({"id": "crates/bbb/src/other.rs::Other", "label": "Other", "kind": "struct"}),
            json!({"id": "crates/ddd/src/lib.rs::lonely", "label": "lonely", "kind": "fn"}),
            json!({"id": "apps/x/src/main.ts::appfn", "label": "appfn", "kind": "fn"}),
        ];
        for i in 0..12 {
            nodes.push(json!({"id": format!("crates/aaa/src/lib.rs::f{i}"), "label": format!("f{i}"), "kind": "fn"}));
        }
        for i in 0..9 {
            nodes.push(json!({"id": format!("crates/bbb/src/lib.rs::g{i}"), "label": format!("g{i}"), "kind": "fn"}));
        }
        json!({
            "nodes": nodes,
            "links": [
                // aaa uses two distinct bbb symbols (one duplicated -> distinct count 2).
                {"source": "crates/aaa/src/lib.rs::f0", "target": "crates/bbb/src/lib.rs::callee", "confidence": "resolved"},
                {"source": "crates/aaa/src/lib.rs::f1", "target": "crates/bbb/src/lib.rs::callee", "confidence": "resolved"},
                {"source": "crates/aaa/src/lib.rs::f2", "target": "crates/bbb/src/other.rs::Other", "confidence": "resolved"},
                // dangling edges never count.
                {"source": "crates/aaa/src/lib.rs::f3", "target": "crates/bbb/src/lib.rs::g0", "confidence": "dangling"},
                // same-crate edge never counts.
                {"source": "crates/aaa/src/lib.rs::f4", "target": "crates/aaa/src/lib.rs::f5", "confidence": "resolved"},
                // cross-crate ref with NO declared dep edge (ccc not in adj): meta counter.
                {"source": "crates/ccc/src/lib.rs::h", "target": "crates/bbb/src/lib.rs::callee", "confidence": "resolved"}
            ]
        })
    }

    /// aaa -> {bbb, ddd, workspace-hack}; all leaves.
    fn adj() -> HashMap<String, Vec<String>> {
        HashMap::from([
            (
                "aaa".to_string(),
                vec![
                    "bbb".to_string(),
                    "ddd".to_string(),
                    "workspace-hack".to_string(),
                ],
            ),
            ("bbb".to_string(), vec![]),
            ("ddd".to_string(), vec![]),
            ("workspace-hack".to_string(), vec![]),
        ])
    }

    fn rows(v: &serde_json::Value) -> Vec<serde_json::Value> {
        v["edges"].as_array().unwrap().clone()
    }

    fn row<'a>(rs: &'a [serde_json::Value], from: &str, to: &str) -> &'a serde_json::Value {
        rs.iter()
            .find(|r| r["from"] == from && r["to"] == to)
            .unwrap()
    }

    #[test]
    fn counts_distinct_resolved_cross_crate_symbols() {
        let out = weigh_edges(&corpus(), &adj(), &HashMap::new());
        let rs = rows(&out);
        assert_eq!(rs.len(), 3); // one row per declared aaa edge
        let ab = row(&rs, "aaa", "bbb");
        assert_eq!(ab["symbols_used"], 2); // callee (deduped) + Other; dangling g0 excluded
        let sample: Vec<&str> = ab["symbols_sample"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(sample.contains(&"callee") && sample.contains(&"Other"));
        assert_eq!(ab["low_visibility"], false);
    }

    #[test]
    fn zero_weight_well_covered_edge_is_candidate() {
        // Give ddd enough nodes to be well-covered but unreferenced.
        let mut c = corpus();
        let nodes = c["nodes"].as_array_mut().unwrap();
        for i in 0..12 {
            nodes.push(serde_json::json!({"id": format!("crates/ddd/src/lib.rs::d{i}"), "label": format!("d{i}"), "kind": "fn"}));
        }
        let out = weigh_edges(&c, &adj(), &HashMap::new());
        let rs = rows(&out);
        let ad = row(&rs, "aaa", "ddd");
        assert_eq!(ad["symbols_used"], 0);
        assert_eq!(ad["status"], "candidate-unused — verify by removal");
    }

    #[test]
    fn low_visibility_row_is_flagged_and_never_candidate() {
        let out = weigh_edges(&corpus(), &adj(), &HashMap::new());
        let rs = rows(&out);
        let ad = row(&rs, "aaa", "ddd"); // ddd has 1 node < LOW_VISIBILITY_MIN
        assert_eq!(ad["low_visibility"], true);
        assert!(ad.get("status").is_none());
    }

    #[test]
    fn workspace_hack_is_never_a_candidate() {
        let out = weigh_edges(&corpus(), &adj(), &HashMap::new());
        let rs = rows(&out);
        let awh = row(&rs, "aaa", "workspace-hack");
        assert_eq!(awh["symbols_used"], 0);
        assert!(awh.get("status").is_none());
    }

    #[test]
    fn declared_and_undeclared_ref_counters_in_meta() {
        let out = weigh_edges(&corpus(), &adj(), &HashMap::new());
        // f0->callee, f1->callee, f2->Other are declared cross-crate refs;
        // ccc->bbb is cross-crate but undeclared. Both counters are needed so
        // the Phase 2 low-confidence gate can compute the ratio.
        assert_eq!(out["meta"]["refs_in_dep_graph"], 3);
        assert_eq!(out["meta"]["refs_not_in_dep_graph"], 1);
    }

    #[test]
    fn target_blast_included_when_times_present() {
        let self_s = HashMap::from([("bbb".to_string(), 30.0), ("aaa".to_string(), 5.0)]);
        let out = weigh_edges(&corpus(), &adj(), &self_s);
        let rs = rows(&out);
        // blast(bbb) = 30 + 5 (aaa depends on it) = 35.
        assert_eq!(row(&rs, "aaa", "bbb")["target_blast_s"], 35.0);
    }
}
