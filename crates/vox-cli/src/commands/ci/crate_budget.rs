//! `vox ci crate-budget` — gate on keystone blast-radius-seconds.
//!
//! Reads `.vox/cache/graphify/crate-map/graph.json` (produced by `vox graphify crate-map`)
//! and fails when any keystone crate in `contracts/ci/crate-budget.v1.json` exceeds its
//! `blast_s_ceiling`. Advisory flag `--exit-zero` prevents CI breakage before the baseline
//! is populated with real measurements.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct BudgetFile {
    #[allow(dead_code)]
    pub schema_version: u32,
    pub keystones: Vec<KeystoneBudget>,
}

#[derive(Debug, Deserialize)]
pub struct KeystoneBudget {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub blast_s_ceiling: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CrateMapNode {
    pub(crate) id: String,
    pub(crate) blast_s: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CrateMap {
    pub(crate) nodes: Vec<CrateMapNode>,
}

/// Load `blast_s` per crate from a crate-map JSON value (Vec of node objects).
pub fn blast_s_from_nodes(nodes: &[CrateMapNode]) -> HashMap<String, f64> {
    nodes
        .iter()
        .filter_map(|n| n.blast_s.map(|s| (n.id.clone(), s)))
        .collect()
}

/// Check keystones against their ceilings. Returns violation messages (empty = pass).
pub fn check_keystones(budget: &BudgetFile, blast_map: &HashMap<String, f64>) -> Vec<String> {
    let mut violations = Vec::new();
    for k in &budget.keystones {
        if let Some(&actual) = blast_map.get(&k.crate_name) {
            if actual > k.blast_s_ceiling {
                violations.push(format!(
                    "  {} blast_s={:.0}s > ceiling {:.0}s (overage: {:.0}s)",
                    k.crate_name,
                    actual,
                    k.blast_s_ceiling,
                    actual - k.blast_s_ceiling
                ));
            }
        }
        // Missing crate → warn but don't fail (may be renamed or not yet in map)
    }
    violations
}

pub fn run_crate_budget(root: &Path, exit_zero: bool) -> Result<()> {
    let map_path = root.join(".vox/cache/graphify/crate-map/graph.json");
    if !map_path.exists() {
        eprintln!(
            "ADVISORY: crate-map not found at {}. Run `vox graphify crate-map` to generate.",
            map_path.display()
        );
        eprintln!("Skipping crate-budget gate (no data).");
        return Ok(());
    }

    let budget_path = root.join("contracts/ci/crate-budget.v1.json");
    let budget_raw = std::fs::read_to_string(&budget_path)
        .with_context(|| format!("read {}", budget_path.display()))?;
    let budget: BudgetFile = serde_json::from_str(&budget_raw)
        .with_context(|| format!("parse {}", budget_path.display()))?;

    let map_raw = std::fs::read_to_string(&map_path)
        .with_context(|| format!("read {}", map_path.display()))?;
    let map: CrateMap =
        serde_json::from_str(&map_raw).with_context(|| format!("parse {}", map_path.display()))?;

    let blast_map = blast_s_from_nodes(&map.nodes);

    for k in &budget.keystones {
        match blast_map.get(&k.crate_name) {
            Some(&actual) => {
                println!(
                    "{}  {} blast_s={:.0}s (ceiling {:.0}s)",
                    if actual > k.blast_s_ceiling {
                        "OVER"
                    } else {
                        "OK  "
                    },
                    k.crate_name,
                    actual,
                    k.blast_s_ceiling
                );
            }
            None => {
                eprintln!(
                    "WARN: keystone '{}' not in crate-map (renamed or not yet generated)",
                    k.crate_name
                );
            }
        }
    }

    let violations = check_keystones(&budget, &blast_map);

    if violations.is_empty() {
        println!("crate-budget: all keystones within budget.");
        return Ok(());
    }

    eprintln!("crate-budget VIOLATIONS ({}):", violations.len());
    for v in &violations {
        eprintln!("{v}");
    }

    if exit_zero {
        eprintln!("(advisory — exiting 0 due to --exit-zero)");
        return Ok(());
    }

    anyhow::bail!(
        "{} keystone crate(s) exceed blast-radius budget",
        violations.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_budget(keystones: Vec<(&str, f64)>) -> BudgetFile {
        BudgetFile {
            schema_version: 1,
            keystones: keystones
                .into_iter()
                .map(|(name, ceiling)| KeystoneBudget {
                    crate_name: name.to_string(),
                    blast_s_ceiling: ceiling,
                })
                .collect(),
        }
    }

    fn make_map(entries: Vec<(&str, f64)>) -> HashMap<String, f64> {
        entries
            .into_iter()
            .map(|(name, s)| (name.to_string(), s))
            .collect()
    }

    #[test]
    fn no_violations_when_all_within_budget() {
        let budget = make_budget(vec![("vox-db", 400.0), ("vox-compiler", 400.0)]);
        let map = make_map(vec![("vox-db", 370.0), ("vox-compiler", 364.0)]);
        assert!(check_keystones(&budget, &map).is_empty());
    }

    #[test]
    fn violation_when_over_ceiling() {
        let budget = make_budget(vec![("vox-db", 300.0)]);
        let map = make_map(vec![("vox-db", 370.0)]);
        let v = check_keystones(&budget, &map);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("vox-db"));
        assert!(v[0].contains("370"));
    }

    #[test]
    fn missing_crate_does_not_fail() {
        // Crate not in map → no violation (WARN only, handled in run_crate_budget)
        let budget = make_budget(vec![("vox-missing", 100.0)]);
        let map = make_map(vec![]);
        assert!(check_keystones(&budget, &map).is_empty());
    }

    #[test]
    fn exactly_at_ceiling_is_ok() {
        let budget = make_budget(vec![("vox-db", 370.0)]);
        let map = make_map(vec![("vox-db", 370.0)]);
        assert!(check_keystones(&budget, &map).is_empty());
    }

    #[test]
    fn zero_blast_s_never_violates() {
        // When crate-map has no audit data, blast_s=0; must not trigger false violation
        let budget = make_budget(vec![("vox-db", 445.0)]);
        let map = make_map(vec![("vox-db", 0.0)]);
        assert!(check_keystones(&budget, &map).is_empty());
    }
}
