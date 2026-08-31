//! `vox ci crate-budget` — gate on keystone blast-radius-seconds.
//!
//! Reads the committed SSOT `contracts/ci/crate-build-map.v1.json` (produced by
//! `vox graphify crate-map --write-summary`) and fails when any keystone in
//! `contracts/ci/crate-budget.v1.json` exceeds its `blast_s_ceiling`. Fails loud when the
//! SSOT is missing or count-only (`has_compile_times=false`). `--exit-zero` → advisory.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct BudgetFile {
    #[allow(dead_code)]
    pub schema_version: u32,
    /// Host triple + measurement mode the ceilings were derived on, e.g.
    /// `"x86_64-unknown-linux-gnu (cargo build)"`. `None` in files predating
    /// provenance tracking.
    #[serde(default)]
    pub measured_on: Option<String>,
    pub keystones: Vec<KeystoneBudget>,
}

/// Warn when the blast-radius map and the ceilings were measured under different
/// conditions.
///
/// `blast_s` scales with both the machine and the measurement mode: `cargo check`
/// skips codegen and linking, so its numbers are on a different scale from
/// `cargo build`, and a slower or busier host inflates everything uniformly.
/// Grading a map produced under one set of conditions against ceilings derived
/// under another reports that difference as regression.
///
/// Returns `None` when either side is unknown. That is deliberate: files
/// predating provenance tracking carry no `measured_on`, and a warning that
/// fires on every run is one people learn to ignore — the failure mode that let
/// the build-bench gate sit disabled behind `continue-on-error` for months.
fn host_mismatch_warning(map_host: Option<&str>, ceiling_host: Option<&str>) -> Option<String> {
    match (map_host, ceiling_host) {
        (Some(m), Some(c)) if m != c => Some(format!(
            "crate-build-map was measured on '{m}' but the ceilings in \
             contracts/ci/crate-budget.v1.json derive from '{c}'. blast_s scales \
             with host and measurement mode, so any OVER verdict below may be \
             that difference rather than a regression. Re-measure on '{c}', or \
             re-derive the ceilings on '{m}', before treating a failure as real \
             — do NOT simply raise the ceilings to match, which resets the \
             ratchet to whatever the current run produced."
        )),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
pub struct KeystoneBudget {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub blast_s_ceiling: f64,
}

/// Extract `crate -> blast_s` from a parsed crate-build-map summary value.
pub fn blast_map_from_summary(summary: &serde_json::Value) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    if let Some(arr) = summary.get("crates").and_then(|v| v.as_array()) {
        for c in arr {
            if let (Some(name), Some(b)) = (
                c.get("crate").and_then(|v| v.as_str()),
                c.get("blast_s").and_then(|v| v.as_f64()),
            ) {
                out.insert(name.to_string(), b);
            }
        }
    }
    out
}

/// Check keystones against their ceilings. Returns violation messages (empty = pass).
pub fn check_keystones(budget: &BudgetFile, blast_map: &HashMap<String, f64>) -> Vec<String> {
    let mut violations = Vec::new();
    for k in &budget.keystones {
        if let Some(&actual) = blast_map.get(&k.crate_name)
            && actual > k.blast_s_ceiling
        {
            violations.push(format!(
                "  {} blast_s={:.0}s > ceiling {:.0}s (overage: {:.0}s)",
                k.crate_name,
                actual,
                k.blast_s_ceiling,
                actual - k.blast_s_ceiling
            ));
        }
        // Missing crate → warn but don't fail (may be renamed or not yet in map)
    }
    violations
}

pub fn run_crate_budget(root: &Path, exit_zero: bool) -> Result<()> {
    let budget_path = root.join("contracts/ci/crate-budget.v1.json");
    let budget: BudgetFile = serde_json::from_str(
        &std::fs::read_to_string(&budget_path)
            .with_context(|| format!("read {}", budget_path.display()))?,
    )
    .with_context(|| format!("parse {}", budget_path.display()))?;

    // FAIL LOUD when the SSOT is missing — never silently skip the gate.
    let summary_path = root.join("contracts/ci/crate-build-map.v1.json");
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&summary_path).with_context(|| {
            format!(
                "read {} — regenerate with `vox graphify crate-map --write-summary`",
                summary_path.display()
            )
        })?)
        .with_context(|| format!("parse {}", summary_path.display()))?;

    // FAIL LOUD when blast_s is count-only — a green gate that can't fail is worse than none.
    let has_times = summary
        .get("has_compile_times")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !has_times && !exit_zero {
        anyhow::bail!(
            "{} has has_compile_times=false (count-only blast_s) — gate would be toothless. \
             Run scripts/crate-build-audit.vox then `vox graphify crate-map --write-summary`.",
            summary_path.display()
        );
    }

    // Surface a provenance mismatch BEFORE the verdicts, so an OVER line is
    // never read as a regression when it is really a host/mode difference.
    if let Some(w) = host_mismatch_warning(
        summary.get("measured_on").and_then(|v| v.as_str()),
        budget.measured_on.as_deref(),
    ) {
        eprintln!("WARN: {w}");
    }

    let blast_map = blast_map_from_summary(&summary);

    for k in &budget.keystones {
        match blast_map.get(&k.crate_name) {
            Some(&actual) => println!(
                "{}  {} blast_s={:.0}s (ceiling {:.0}s)",
                if actual > k.blast_s_ceiling {
                    "OVER"
                } else {
                    "OK  "
                },
                k.crate_name,
                actual,
                k.blast_s_ceiling
            ),
            None => eprintln!(
                "WARN: keystone '{}' not in crate-build-map (renamed?)",
                k.crate_name
            ),
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
            measured_on: None,
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
    fn warns_when_map_host_differs_from_ceiling_host() {
        let w = host_mismatch_warning(
            Some("x86_64-pc-windows-msvc (cargo check)"),
            Some("x86_64-unknown-linux-gnu (cargo build)"),
        )
        .expect("differing provenance must warn");
        assert!(
            w.contains("x86_64-pc-windows-msvc"),
            "names the map host: {w}"
        );
        assert!(
            w.contains("x86_64-unknown-linux-gnu"),
            "names the ceiling host: {w}"
        );
    }

    #[test]
    fn no_warning_when_provenance_matches_or_is_unknown() {
        let same = "x86_64-unknown-linux-gnu (cargo build)";
        assert!(host_mismatch_warning(Some(same), Some(same)).is_none());
        // Absent provenance must NOT warn: files predating provenance tracking
        // carry none, and a warning that fires on every run trains people to
        // ignore it — which is how the build-bench gate got disabled.
        assert!(host_mismatch_warning(None, Some(same)).is_none());
        assert!(host_mismatch_warning(Some(same), None).is_none());
        assert!(host_mismatch_warning(None, None).is_none());
    }

    #[test]
    fn blast_map_from_summary_parses_committed_shape() {
        let summary = serde_json::json!({
            "has_compile_times": true,
            "crates": [
                { "crate": "vox-db",  "compile_s": 34.0, "dependents": 25, "blast_s": 349.0, "fan_in": 5 },
                { "crate": "vox-cli", "compile_s": 10.0, "dependents": 0,  "blast_s": 10.0,  "fan_in": 0 }
            ]
        });
        let m = blast_map_from_summary(&summary);
        assert_eq!(m.get("vox-db").copied(), Some(349.0));
        assert_eq!(m.get("vox-cli").copied(), Some(10.0));
    }
}
