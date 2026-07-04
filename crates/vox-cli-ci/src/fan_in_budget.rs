//! `vox ci fan-in-budget` — deny-new-growth gate for workspace fan-in.
//!
//! Reads `contracts/ci/crate-graph.v1.json` and `contracts/ci/fan-in-snapshot.v1.json`.
//! Fails when any crate's in-tree dependent COUNT grows beyond its committed snapshot value.
//! Crates that shrink their fan-in are not flagged (ratchet, not two-way).

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct FanInSnapshot {
    #[allow(dead_code)]
    pub schema_version: u32,
    pub snapshot: HashMap<String, usize>,
}

/// Compute fan-in from `crate-graph.v1.json` `{"crates": {name: [dep, ...]}}`.
/// Returns `(crate_name, dependents_count)` for all crates listed in `snapshot`.
pub fn compute_fan_in(graph: &Value, snapshot: &FanInSnapshot) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    // Init to 0 for all snapshot keys so missing crates show up as 0 (not missing)
    for k in snapshot.snapshot.keys() {
        counts.insert(k.clone(), 0);
    }
    if let Some(m) = graph.get("crates").and_then(|v| v.as_object()) {
        for (_crate_name, deps) in m {
            if let Some(arr) = deps.as_array() {
                for dep in arr {
                    if let Some(dep_str) = dep.as_str()
                        && let Some(c) = counts.get_mut(dep_str)
                    {
                        *c += 1;
                    }
                }
            }
        }
    }
    counts
}

/// Check for fan-in regressions (crate gained new dependents beyond snapshot).
/// Returns violation messages for crates whose count > snapshot.
pub fn check_regressions(actual: &HashMap<String, usize>, snapshot: &FanInSnapshot) -> Vec<String> {
    let mut violations = Vec::new();
    let mut keys: Vec<&String> = snapshot.snapshot.keys().collect();
    keys.sort();
    for k in keys {
        let snap = *snapshot.snapshot.get(k).unwrap_or(&0);
        let act = *actual.get(k).unwrap_or(&0);
        if act > snap {
            violations.push(format!(
                "  {} fan-in grew: {} → {} ({}  new dependent(s))",
                k,
                snap,
                act,
                act - snap
            ));
        }
    }
    violations
}

pub fn run_fan_in_budget(root: &Path, exit_zero: bool) -> Result<()> {
    let graph_path = root.join("contracts/ci/crate-graph.v1.json");
    let snapshot_path = root.join("contracts/ci/fan-in-snapshot.v1.json");

    let graph_raw = std::fs::read_to_string(&graph_path)
        .with_context(|| format!("read {}", graph_path.display()))?;
    let graph: Value = serde_json::from_str(&graph_raw)
        .with_context(|| format!("parse {}", graph_path.display()))?;

    let snapshot_raw = std::fs::read_to_string(&snapshot_path)
        .with_context(|| format!("read {}", snapshot_path.display()))?;
    let snapshot: FanInSnapshot = serde_json::from_str(&snapshot_raw)
        .with_context(|| format!("parse {}", snapshot_path.display()))?;

    let actual = compute_fan_in(&graph, &snapshot);

    // Report all
    let mut keys: Vec<&String> = snapshot.snapshot.keys().collect();
    keys.sort();
    for k in &keys {
        let snap = *snapshot.snapshot.get(*k).unwrap_or(&0);
        let act = *actual.get(*k).unwrap_or(&0);
        let status = if act > snap {
            "GREW"
        } else if act < snap {
            "shrank"
        } else {
            "ok  "
        };
        println!("{status}  {k}: {act} (snapshot {snap})");
    }

    let violations = check_regressions(&actual, &snapshot);
    if violations.is_empty() {
        println!("fan-in-budget: no regressions.");
        return Ok(());
    }

    eprintln!("fan-in-budget REGRESSIONS ({}):", violations.len());
    for v in &violations {
        eprintln!("{v}");
    }
    eprintln!(
        "To allow: bump the count in contracts/ci/fan-in-snapshot.v1.json and update crate-graph.v1.json."
    );

    if exit_zero {
        eprintln!("(advisory — exiting 0 due to --exit-zero)");
        return Ok(());
    }

    anyhow::bail!("{} fan-in regression(s) detected", violations.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_snapshot(entries: Vec<(&str, usize)>) -> FanInSnapshot {
        FanInSnapshot {
            schema_version: 1,
            snapshot: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    fn make_actual(entries: Vec<(&str, usize)>) -> HashMap<String, usize> {
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn no_regressions_when_all_at_snapshot() {
        let snap = make_snapshot(vec![("vox-config", 40), ("vox-db", 25)]);
        let actual = make_actual(vec![("vox-config", 40), ("vox-db", 25)]);
        assert!(check_regressions(&actual, &snap).is_empty());
    }

    #[test]
    fn growth_detected() {
        let snap = make_snapshot(vec![("vox-config", 40)]);
        let actual = make_actual(vec![("vox-config", 42)]);
        let v = check_regressions(&actual, &snap);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("vox-config"));
        assert!(v[0].contains("40"));
        assert!(v[0].contains("42"));
    }

    #[test]
    fn shrinkage_not_flagged() {
        let snap = make_snapshot(vec![("vox-config", 40)]);
        let actual = make_actual(vec![("vox-config", 38)]);
        assert!(check_regressions(&actual, &snap).is_empty());
    }

    #[test]
    fn compute_fan_in_counts_correctly() {
        let snap = make_snapshot(vec![("vox-db", 0)]);
        let graph = json!({
            "crates": {
                "vox-cli": ["vox-db", "vox-compiler"],
                "vox-scientia": ["vox-db"],
                "vox-search": ["vox-compiler"]
            }
        });
        let result = compute_fan_in(&graph, &snap);
        assert_eq!(result["vox-db"], 2);
    }

    #[test]
    fn missing_crate_in_graph_counts_as_zero() {
        let snap = make_snapshot(vec![("vox-nobody", 5)]);
        let graph = json!({ "crates": {} });
        let result = compute_fan_in(&graph, &snap);
        assert_eq!(result.get("vox-nobody").copied().unwrap_or(0), 0);
        // Shrinkage (0 < 5) — not a regression
        let violations = check_regressions(&result, &snap);
        assert!(violations.is_empty());
    }
}
