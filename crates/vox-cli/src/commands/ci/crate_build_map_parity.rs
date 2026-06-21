//! `vox ci crate-build-map-parity` — drift gate for the committed crate-build SSOT.
//!
//! Recomputes derived fields (dependents/blast_s/fan_in) from committed
//! `crate-graph.v1.json` + the `compile_s` embedded in `crate-build-map.v1.json`, then
//! compares to the committed derived values. Fails on drift (e.g. a Cargo.toml dep changed
//! but the summary wasn't regenerated). Mirrors crate-graph / config-registry parity gates.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Compare committed vs recomputed summary. Returns drift messages (empty = in sync).
/// Only DERIVED fields are compared; `compile_s` is the input, taken from committed as-is.
pub fn diff_summaries(
    committed: &serde_json::Value,
    recomputed: &serde_json::Value,
) -> Vec<String> {
    let idx = |v: &serde_json::Value| -> HashMap<String, (i64, i64, i64)> {
        let mut m = HashMap::new();
        if let Some(arr) = v.get("crates").and_then(|x| x.as_array()) {
            for c in arr {
                let name = c
                    .get("crate")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let dep = c.get("dependents").and_then(|x| x.as_i64()).unwrap_or(-1);
                let blast = c
                    .get("blast_s")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(-1.0)
                    .round() as i64;
                let fan = c.get("fan_in").and_then(|x| x.as_i64()).unwrap_or(-1);
                m.insert(name, (dep, blast, fan));
            }
        }
        m
    };
    let a = idx(committed);
    let b = idx(recomputed);
    let mut drift = Vec::new();
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) if x != y => drift.push(format!(
                "  {k}: committed (dep={},blast={},fan={}) != recomputed (dep={},blast={},fan={})",
                x.0, x.1, x.2, y.0, y.1, y.2
            )),
            (Some(_), None) => {
                drift.push(format!("  {k}: in committed summary but not recomputed"))
            }
            (None, Some(_)) => drift.push(format!(
                "  {k}: recomputed but missing from committed (regen needed)"
            )),
            _ => {}
        }
    }
    drift
}

pub fn run_crate_build_map_parity(root: &Path) -> Result<()> {
    let graph_path = root.join("contracts/ci/crate-graph.v1.json");
    let summary_path = root.join("contracts/ci/crate-build-map.v1.json");

    let crate_graph: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&graph_path)
            .with_context(|| format!("read {}", graph_path.display()))?,
    )?;
    let committed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&summary_path).with_context(|| {
            format!(
                "read {} — regenerate with `vox graphify crate-map --write-summary`",
                summary_path.display()
            )
        })?)?;

    // Pull compile_s (the input) back out so recomputation is deterministic.
    let mut compile_times: HashMap<String, f64> = HashMap::new();
    if let Some(arr) = committed.get("crates").and_then(|v| v.as_array()) {
        for c in arr {
            if let (Some(name), Some(cs)) = (
                c.get("crate").and_then(|v| v.as_str()),
                c.get("compile_s").and_then(|v| v.as_f64()),
            ) {
                compile_times.insert(name.to_string(), cs);
            }
        }
    }

    let recomputed =
        vox_graphify_reader::crate_model::build_crate_summary(&crate_graph, &compile_times);
    let drift = diff_summaries(&committed, &recomputed);

    if drift.is_empty() {
        println!("crate-build-map-parity: committed summary matches crate-graph.v1.json.");
        return Ok(());
    }
    eprintln!("crate-build-map-parity DRIFT ({}):", drift.len());
    for d in &drift {
        eprintln!("{d}");
    }
    anyhow::bail!(
        "crate-build-map.v1.json is stale vs crate-graph.v1.json — \
         run `vox graphify crate-map --write-summary` and commit the result"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identical_summaries_have_no_drift() {
        let s = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 0, "blast_s": 1.0, "fan_in": 0 }
        ]});
        assert!(diff_summaries(&s, &s).is_empty());
    }

    #[test]
    fn dependent_count_drift_detected() {
        let committed = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 2, "blast_s": 6.0, "fan_in": 1 }
        ]});
        let recomputed = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 3, "blast_s": 9.0, "fan_in": 1 }
        ]});
        let d = diff_summaries(&committed, &recomputed);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("a"));
    }

    #[test]
    fn new_crate_in_recompute_flagged() {
        let committed = json!({ "crates": [] });
        let recomputed = json!({ "crates": [
            { "crate": "newbie", "compile_s": 1.0, "dependents": 0, "blast_s": 1.0, "fan_in": 0 }
        ]});
        let d = diff_summaries(&committed, &recomputed);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("newbie"));
    }
}
