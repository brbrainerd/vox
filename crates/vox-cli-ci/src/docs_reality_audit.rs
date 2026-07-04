//! `vox ci docs-reality-audit` — validate Documentation Reality Audit artifacts (inventory, findings, metrics).
//!
//! SSOT: `contracts/documentation/docs-reality-audit.program.v1.yaml`

use anyhow::{Context, Result, anyhow};
use glob::glob;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;
use vox_jsonschema_util::{compile_validator, validate};

const PROGRAM_REL: &str = "contracts/documentation/docs-reality-audit.program.v1.yaml";
const INVENTORY_REL: &str = "contracts/reports/docs-reality-audit/inventory.v1.json";
const FINDINGS_REL: &str = "contracts/reports/docs-reality-audit/findings.v1.json";
const METRICS_REL: &str = "contracts/reports/docs-reality-audit/metrics.v1.json";
const INVENTORY_SCHEMA_REL: &str = "contracts/reports/docs-reality-audit/inventory.v1.schema.json";
const FINDINGS_SCHEMA_REL: &str = "contracts/reports/docs-reality-audit/findings.v1.schema.json";
const METRICS_SCHEMA_REL: &str = "contracts/reports/docs-reality-audit/metrics.v1.schema.json";

#[derive(Debug, Deserialize)]
struct InventoryFile {
    #[allow(dead_code)]
    schema_version: u32,
    claims: Vec<ClaimRow>,
}

#[derive(Debug, Deserialize)]
struct ClaimRow {
    id: String,
    doc_path: String,
    evidence_hints: Option<EvidenceHints>,
}

#[derive(Debug, Deserialize)]
struct EvidenceHints {
    code_globs: Option<Vec<String>>,
    contracts: Option<Vec<String>>,
    tests_globs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct FindingsFile {
    #[allow(dead_code)]
    schema_version: u32,
    findings: Vec<FindingRow>,
}

#[derive(Debug, Deserialize)]
struct FindingRow {
    id: String,
    claim_ids: Vec<String>,
    classification: String,
    scores: FindingScores,
    priority_score: i32,
    priority_band: String,
    status: String,
    #[allow(dead_code)]
    recommended_action: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FindingScores {
    impact: i32,
    blast_radius: i32,
    staleness: i32,
    enforcement_gap: i32,
    tractability: i32,
}

/// PriorityScore = Impact*2 + BlastRadius*2 + Staleness + EnforcementGap + Tractability (program SSOT).
pub(crate) fn compute_priority_score(s: &FindingScores) -> i32 {
    s.impact * 2 + s.blast_radius * 2 + s.staleness + s.enforcement_gap + s.tractability
}

pub(crate) fn priority_band_from_score(score: i32) -> &'static str {
    if score >= 22 {
        "P0"
    } else if score >= 14 {
        "P1"
    } else {
        "P2"
    }
}

fn validate_json_against_schema(
    root: &Path,
    schema_rel: &str,
    label: &str,
    instance: &Value,
) -> Result<()> {
    let schema_path = root.join(schema_rel);
    let schema_raw = read_utf8_path_capped(&schema_path)
        .with_context(|| format!("read {}", schema_path.display()))?;
    let schema_val: Value = serde_json::from_str(&schema_raw)
        .with_context(|| format!("parse JSON schema {}", schema_path.display()))?;
    let validator = compile_validator(&schema_val, schema_path.display())
        .with_context(|| format!("compile schema {}", schema_path.display()))?;
    validate(instance, &validator, label).map_err(|e| anyhow!("{e}"))
}

fn glob_match_count(root: &Path, pattern: &str) -> Result<usize> {
    let full = root.join(pattern);
    let pat = full.to_string_lossy().to_string();
    let entries: Vec<_> = glob(&pat)
        .with_context(|| format!("invalid glob pattern {pat:?}"))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("glob iteration failed for {pat:?}"))?;
    Ok(entries.len())
}

fn verify_paths_for_claim(root: &Path, claim: &ClaimRow) -> Result<()> {
    let dp = root.join(&claim.doc_path);
    if !dp.is_file() {
        anyhow::bail!(
            "inventory claim {}: doc_path missing or not a file: {}",
            claim.id,
            claim.doc_path
        );
    }
    let Some(h) = &claim.evidence_hints else {
        return Ok(());
    };
    if let Some(contracts) = &h.contracts {
        for c in contracts {
            let p = root.join(c);
            if !p.is_file() {
                anyhow::bail!(
                    "inventory claim {}: evidence_hints.contracts entry missing: {c}",
                    claim.id
                );
            }
        }
    }
    for globs in [&h.code_globs, &h.tests_globs].into_iter().flatten() {
        for g in globs {
            let n = glob_match_count(root, g).with_context(|| {
                format!(
                    "inventory claim {}: glob expansion failed for {g:?}",
                    claim.id
                )
            })?;
            if n == 0 {
                anyhow::bail!(
                    "inventory claim {}: glob matched 0 paths (expected ≥1): {g}",
                    claim.id
                );
            }
        }
    }
    Ok(())
}

fn verify_inventory_paths(root: &Path, inv: &InventoryFile) -> Result<()> {
    let mut ids = HashSet::new();
    for c in &inv.claims {
        if !ids.insert(c.id.clone()) {
            anyhow::bail!("duplicate inventory claim id: {}", c.id);
        }
        verify_paths_for_claim(root, c)?;
    }
    Ok(())
}

fn verify_findings_consistency(
    _root: &Path,
    inv: &InventoryFile,
    findings: &FindingsFile,
) -> Result<()> {
    let claim_ids: HashSet<_> = inv.claims.iter().map(|c| c.id.as_str()).collect();
    for f in &findings.findings {
        let expected = compute_priority_score(&f.scores);
        if expected != f.priority_score {
            anyhow::bail!(
                "finding {}: priority_score {} does not match formula (expected {})",
                f.id,
                f.priority_score,
                expected
            );
        }
        let band = priority_band_from_score(f.priority_score);
        if band != f.priority_band {
            anyhow::bail!(
                "finding {}: priority_band {:?} does not match score {} (expected {:?})",
                f.id,
                f.priority_band,
                f.priority_score,
                band
            );
        }
        for cid in &f.claim_ids {
            if !claim_ids.contains(cid.as_str()) {
                anyhow::bail!(
                    "finding {}: unknown claim_id {:?} (not in inventory)",
                    f.id,
                    cid
                );
            }
        }
    }

    // Program file exists (human SSOT; YAML parse smoke).
    let prog = _root.join(PROGRAM_REL);
    if !prog.is_file() {
        anyhow::bail!("missing program SSOT: {}", PROGRAM_REL);
    }
    let _prog_txt =
        read_utf8_path_capped(&prog).with_context(|| format!("read {}", prog.display()))?;

    Ok(())
}

/// Validate schemas, path hints, and finding score invariants.
pub fn run_verify(root: &Path) -> Result<()> {
    let inv_path = root.join(INVENTORY_REL);
    let inv_raw =
        read_utf8_path_capped(&inv_path).with_context(|| format!("read {}", inv_path.display()))?;
    let inv_val: Value = serde_json::from_str(&inv_raw).context("parse inventory JSON")?;
    validate_json_against_schema(
        root,
        INVENTORY_SCHEMA_REL,
        "docs-reality-audit inventory",
        &inv_val,
    )?;
    let inv: InventoryFile = serde_json::from_value(inv_val).context("deserialize inventory")?;

    let findings_path = root.join(FINDINGS_REL);
    let findings_raw = read_utf8_path_capped(&findings_path)
        .with_context(|| format!("read {}", findings_path.display()))?;
    let findings_val: Value = serde_json::from_str(&findings_raw).context("parse findings JSON")?;
    validate_json_against_schema(
        root,
        FINDINGS_SCHEMA_REL,
        "docs-reality-audit findings",
        &findings_val,
    )?;
    let findings: FindingsFile =
        serde_json::from_value(findings_val).context("deserialize findings")?;

    let metrics_path = root.join(METRICS_REL);
    let metrics_raw = read_utf8_path_capped(&metrics_path)
        .with_context(|| format!("read {}", metrics_path.display()))?;
    let metrics_val: Value = serde_json::from_str(&metrics_raw).context("parse metrics JSON")?;
    validate_json_against_schema(
        root,
        METRICS_SCHEMA_REL,
        "docs-reality-audit metrics",
        &metrics_val,
    )?;

    verify_inventory_paths(root, &inv)?;
    verify_findings_consistency(root, &inv, &findings)?;

    println!(
        "docs-reality-audit verify OK ({} claims, {} findings)",
        inv.claims.len(),
        findings.findings.len()
    );
    Ok(())
}

fn rollout_milestone_pct(inv_claims: usize, findings: &[FindingRow]) -> u8 {
    if inv_claims == 0 {
        return 0;
    }
    if findings.is_empty() {
        return 25;
    }
    let total = findings.len() as f64;
    let closed = findings
        .iter()
        .filter(|f| f.status == "closed" || f.status == "verified")
        .count() as f64;
    let extra = (closed / total) * 75.0;
    let pct = 25.0 + extra;
    pct.round().clamp(0.0, 100.0) as u8
}

/// Recompute `metrics.v1.json` from findings + inventory (optional `--write`).
pub fn run_metrics(root: &Path, write: bool) -> Result<()> {
    let inv_path = root.join(INVENTORY_REL);
    let inv_raw =
        read_utf8_path_capped(&inv_path).with_context(|| format!("read {}", inv_path.display()))?;
    let inv_val: Value = serde_json::from_str(&inv_raw).context("parse inventory JSON")?;
    validate_json_against_schema(
        root,
        INVENTORY_SCHEMA_REL,
        "docs-reality-audit inventory",
        &inv_val,
    )?;
    let inv: InventoryFile = serde_json::from_value(inv_val).context("deserialize inventory")?;

    let findings_path = root.join(FINDINGS_REL);
    let findings_raw = read_utf8_path_capped(&findings_path)
        .with_context(|| format!("read {}", findings_path.display()))?;
    let findings_val: Value = serde_json::from_str(&findings_raw).context("parse findings JSON")?;
    validate_json_against_schema(
        root,
        FINDINGS_SCHEMA_REL,
        "docs-reality-audit findings",
        &findings_val,
    )?;
    let findings: FindingsFile =
        serde_json::from_value(findings_val).context("deserialize findings")?;

    let mut counts_class: HashMap<String, i32> = HashMap::new();
    let mut counts_status: HashMap<String, i32> = HashMap::new();
    let mut counts_band: HashMap<String, i32> = HashMap::new();
    let mut open_p0 = 0i32;
    let mut open_p1 = 0i32;
    let terminal = HashSet::from(["closed", "verified"]);

    for f in &findings.findings {
        *counts_class.entry(f.classification.clone()).or_insert(0) += 1;
        *counts_status.entry(f.status.clone()).or_insert(0) += 1;
        *counts_band.entry(f.priority_band.clone()).or_insert(0) += 1;
        if !terminal.contains(f.status.as_str()) {
            if f.priority_band == "P0" {
                open_p0 += 1;
            }
            if f.priority_band == "P1" {
                open_p1 += 1;
            }
        }
    }

    let closed = findings
        .findings
        .iter()
        .filter(|f| terminal.contains(f.status.as_str()))
        .count();
    let open = findings.findings.len().saturating_sub(closed);
    let milestone = rollout_milestone_pct(inv.claims.len(), &findings.findings);

    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let metrics = serde_json::json!({
        "schema_version": 1,
        "generated_at": generated_at,
        "inventory_claim_count": inv.claims.len(),
        "findings_total": findings.findings.len(),
        "findings_open": open,
        "findings_closed": closed,
        "counts_by_classification": counts_class,
        "counts_by_status": counts_status,
        "counts_by_priority_band": counts_band,
        "open_p0": open_p0,
        "open_p1": open_p1,
        "rollout_milestone_pct": milestone,
        "rollout_notes": "Computed by `vox ci docs-reality-audit metrics`; see contracts/documentation/docs-reality-audit.program.v1.yaml."
    });

    validate_json_against_schema(
        root,
        METRICS_SCHEMA_REL,
        "docs-reality-audit metrics",
        &metrics,
    )?;

    if write {
        let metrics_path = root.join(METRICS_REL);
        std::fs::write(
            &metrics_path,
            serde_json::to_string_pretty(&metrics)? + "\n",
        )
        .with_context(|| format!("write {}", metrics_path.display()))?;
        println!("wrote {}", metrics_path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&metrics)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_score_matches_plan_formula() {
        let s = FindingScores {
            impact: 5,
            blast_radius: 5,
            staleness: 3,
            enforcement_gap: 3,
            tractability: 3,
        };
        assert_eq!(compute_priority_score(&s), 29);
        assert_eq!(priority_band_from_score(29), "P0");
        assert_eq!(priority_band_from_score(21), "P1");
        assert_eq!(priority_band_from_score(13), "P2");
    }

    #[test]
    fn rollout_milestone_empty_findings_is_25_when_inventory_nonempty() {
        assert_eq!(rollout_milestone_pct(10, &[]), 25);
    }
}
