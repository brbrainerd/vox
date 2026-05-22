//! Rule 14: evidence-ledger integrity.
//!
//! Reads `contracts/reports/evidence-ledger.v1.json` and verifies that every
//! row under `claims` points at an `artifact_path` that exists on disk and is
//! fresher than its declared `max_age_days`. Catches the failure class where
//! the audit scorecard claims "CLOSED" but the supporting evidence is missing
//! or stale.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §1.2,
//! a CLOSED / PASSING word in the audit scorecard must resolve to a claim_id
//! here; that scorecard cross-check lives in a sibling pass (see
//! `scan_scorecard_for_unbacked_claims`).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One row under `claims` (the "this is actually closed and here's where to look" entries).
///
/// `criterion` and `notes` are carried for ledger introspection / future
/// tooling (e.g. `vox audit ledger inspect`); the lint itself only acts on
/// `artifact_path`, `artifact_kind`, and `max_age_days`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EvidenceClaim {
    pub criterion: String,
    pub artifact_path: String,
    pub artifact_kind: String,
    pub max_age_days: u32,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LedgerFile {
    schema_version: u32,
    #[serde(default)]
    claims: BTreeMap<String, EvidenceClaim>,
    #[serde(default, rename = "blocked_claims")]
    _blocked_claims: serde_json::Value,
}

/// One per-claim finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceFinding {
    pub claim_id: String,
    pub kind: FindingKind,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    /// `artifact_path` does not exist on disk relative to workspace root.
    MissingArtifact,
    /// `artifact_kind == "directory_with_dated_json"` but no `<UTC>.json` files inside.
    DirectoryHasNoDatedReports,
    /// Newest report is older than `max_age_days`.
    Stale { age_days: u32, max_age_days: u32 },
    /// `artifact_kind` is not one of the recognized values.
    UnknownArtifactKind(String),
}

impl FindingKind {
    pub fn severity(&self) -> &'static str {
        match self {
            FindingKind::MissingArtifact => "ERROR",
            FindingKind::DirectoryHasNoDatedReports => "ERROR",
            // Staleness + unknown-kind are advisory.
            FindingKind::Stale { .. } => "WARN",
            FindingKind::UnknownArtifactKind(_) => "WARN",
        }
    }
}

/// Run the evidence-ledger integrity check.
///
/// Returns a vector of findings; empty vector ⇒ ledger is internally consistent.
/// Errors (vs findings) indicate the ledger file itself is malformed.
pub fn check_evidence_ledger(workspace_root: &Path) -> Result<Vec<EvidenceFinding>> {
    let ledger_path = workspace_root
        .join("contracts")
        .join("reports")
        .join("evidence-ledger.v1.json");
    let body = std::fs::read_to_string(&ledger_path)
        .with_context(|| format!("read {}", ledger_path.display()))?;
    let ledger: LedgerFile = serde_json::from_str(&body)
        .with_context(|| format!("parse {}", ledger_path.display()))?;
    if ledger.schema_version != 1 {
        anyhow::bail!(
            "evidence-ledger.v1.json schema_version is {}, expected 1",
            ledger.schema_version
        );
    }

    let mut findings = Vec::new();
    for (claim_id, claim) in &ledger.claims {
        let abs = workspace_root.join(&claim.artifact_path);
        if !abs.exists() {
            findings.push(EvidenceFinding {
                claim_id: claim_id.clone(),
                kind: FindingKind::MissingArtifact,
                artifact_path: abs,
            });
            continue;
        }
        match claim.artifact_kind.as_str() {
            "directory_with_dated_json" => {
                let newest = newest_dated_json(&abs);
                match newest {
                    None => findings.push(EvidenceFinding {
                        claim_id: claim_id.clone(),
                        kind: FindingKind::DirectoryHasNoDatedReports,
                        artifact_path: abs.clone(),
                    }),
                    Some((path, age_days)) => {
                        if age_days > claim.max_age_days {
                            findings.push(EvidenceFinding {
                                claim_id: claim_id.clone(),
                                kind: FindingKind::Stale {
                                    age_days,
                                    max_age_days: claim.max_age_days,
                                },
                                artifact_path: path,
                            });
                        }
                    }
                }
            }
            // For binary/test_file/source_file/directory we only validate
            // existence — staleness on source isn't meaningful. The ledger
            // can be extended with stricter kinds (e.g. "git_history") later.
            "binary" | "test_file" | "source_file" | "directory" => {}
            other => findings.push(EvidenceFinding {
                claim_id: claim_id.clone(),
                kind: FindingKind::UnknownArtifactKind(other.to_string()),
                artifact_path: abs,
            }),
        }
    }
    Ok(findings)
}

/// Find the newest dated artifact in `dir`, accepting any of:
/// - `YYYY-MM-DD.json` (canonical, most gates)
/// - `YYYY-MM-DD-<suffix>.json` (e.g. CR-P2's `<UTC>-7day.json`)
/// - `YYYY-QN.json` (quarterly artifacts — CR-L8 corpus-feedback uses this)
///
/// Returns the absolute path and the artifact's age in calendar days (UTC).
/// Quarter-formatted files are dated to the first day of the quarter.
fn newest_dated_json(dir: &Path) -> Option<(PathBuf, u32)> {
    let mut best: Option<(PathBuf, chrono::NaiveDate)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext != "json" {
            continue;
        }
        let parsed_date = parse_artifact_date(stem);
        if let Some(date) = parsed_date {
            if best.as_ref().map(|(_, d)| date > *d).unwrap_or(true) {
                best = Some((path.clone(), date));
            }
        }
    }
    let (path, date) = best?;
    let today = chrono::Utc::now().date_naive();
    let age_days = (today - date).num_days().max(0) as u32;
    Some((path, age_days))
}

/// Parse an artifact stem into a NaiveDate. Accepts:
/// - `YYYY-MM-DD`
/// - `YYYY-MM-DD-<suffix>` (takes the first 10 chars)
/// - `YYYY-QN` where N ∈ {1,2,3,4} (dates to the first day of the quarter)
fn parse_artifact_date(stem: &str) -> Option<chrono::NaiveDate> {
    if let Ok(d) = chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
        return Some(d);
    }
    if stem.len() >= 10
        && let Some(prefix) = stem.get(..10)
        && let Ok(d) = chrono::NaiveDate::parse_from_str(prefix, "%Y-%m-%d")
    {
        return Some(d);
    }
    // Quarter form: YYYY-Q1..Q4 → first day of the quarter.
    if stem.len() == 7 && &stem[4..6] == "-Q" {
        let year: i32 = stem.get(..4)?.parse().ok()?;
        let quarter: u32 = stem.get(6..7)?.parse().ok()?;
        if !(1..=4).contains(&quarter) {
            return None;
        }
        let month = (quarter - 1) * 3 + 1;
        return chrono::NaiveDate::from_ymd_opt(year, month, 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_ledger(root: &Path, claims_json: &str) {
        let dir = root.join("contracts").join("reports");
        fs::create_dir_all(&dir).unwrap();
        let body = format!(
            r#"{{"schema_version":1,"claims":{claims_json},"blocked_claims":{{}}}}"#
        );
        fs::write(dir.join("evidence-ledger.v1.json"), body).unwrap();
    }

    #[test]
    fn missing_artifact_is_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        write_ledger(
            tmp.path(),
            r#"{"fake":{"criterion":"CR-X","artifact_path":"does/not/exist.json","artifact_kind":"binary","max_age_days":30}}"#,
        );
        let findings = check_evidence_ledger(tmp.path()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].claim_id, "fake");
        assert_eq!(findings[0].kind, FindingKind::MissingArtifact);
    }

    #[test]
    fn empty_dated_dir_is_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        let report_dir = tmp.path().join("contracts").join("reports").join("retirement");
        fs::create_dir_all(&report_dir).unwrap();
        write_ledger(
            tmp.path(),
            r#"{"r":{"criterion":"CR-L6","artifact_path":"contracts/reports/retirement/","artifact_kind":"directory_with_dated_json","max_age_days":30}}"#,
        );
        let findings = check_evidence_ledger(tmp.path()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::DirectoryHasNoDatedReports);
    }

    #[test]
    fn fresh_dated_dir_is_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let report_dir = tmp.path().join("contracts").join("reports").join("retirement");
        fs::create_dir_all(&report_dir).unwrap();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        fs::write(report_dir.join(format!("{today}.json")), "{}").unwrap();
        write_ledger(
            tmp.path(),
            r#"{"r":{"criterion":"CR-L6","artifact_path":"contracts/reports/retirement/","artifact_kind":"directory_with_dated_json","max_age_days":30}}"#,
        );
        let findings = check_evidence_ledger(tmp.path()).unwrap();
        assert!(findings.is_empty(), "expected clean, got: {findings:?}");
    }

    #[test]
    fn stale_dated_report_is_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        let report_dir = tmp.path().join("contracts").join("reports").join("retirement");
        fs::create_dir_all(&report_dir).unwrap();
        // 2020-01-01 → definitely older than 30 days from now.
        fs::write(report_dir.join("2020-01-01.json"), "{}").unwrap();
        write_ledger(
            tmp.path(),
            r#"{"r":{"criterion":"CR-L6","artifact_path":"contracts/reports/retirement/","artifact_kind":"directory_with_dated_json","max_age_days":30}}"#,
        );
        let findings = check_evidence_ledger(tmp.path()).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0].kind, FindingKind::Stale { .. }));
    }

    #[test]
    fn unknown_artifact_kind_is_flagged_as_warn() {
        let tmp = tempfile::tempdir().unwrap();
        let report_dir = tmp.path().join("contracts").join("reports").join("x");
        fs::create_dir_all(&report_dir).unwrap();
        write_ledger(
            tmp.path(),
            r#"{"r":{"criterion":"CR-X","artifact_path":"contracts/reports/x/","artifact_kind":"weird-kind","max_age_days":30}}"#,
        );
        let findings = check_evidence_ledger(tmp.path()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind.severity(), "WARN");
    }
}
