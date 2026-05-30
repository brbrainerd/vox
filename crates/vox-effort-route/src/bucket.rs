//! Deterministic grouping of findings by the structural fix that prevents them.

use crate::load::LoadedFinding;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BucketKey {
    pub waste_category: String,   // Debug-formatted enum (stable, sortable)
    pub remediation_kind: String,
    pub primary_crate: String,
}

#[derive(Debug, Clone)]
pub struct Bucket {
    pub key: BucketKey,
    pub members: Vec<LoadedFinding>,
}

/// Derive the owning crate from a finding's evidence pointers (preferred) or
/// shape histogram (fallback). Returns "<workspace-root>" when no crate path found.
pub fn primary_crate(f: &LoadedFinding) -> String {
    let finding = f.row.finding.as_ref();
    if let Some(finding) = finding {
        for ptr in &finding.evidence_pointers {
            if let Some(c) = crate_from_path(ptr) { return c; }
        }
    }
    // Fallback: nothing usable.
    "<workspace-root>".to_string()
}

fn crate_from_path(path: &str) -> Option<String> {
    // "crates/<name>/..." → "<name>"
    let path = path.split(':').next().unwrap_or(path); // strip ":line"
    let mut parts = path.split('/');
    while let Some(p) = parts.next() {
        if p == "crates" {
            if let Some(name) = parts.next() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub fn group(findings: Vec<LoadedFinding>) -> Vec<Bucket> {
    let mut map: BTreeMap<BucketKey, Vec<LoadedFinding>> = BTreeMap::new();
    for f in findings {
        let finding = f.row.finding.as_ref().expect("filtered to Some");
        let key = BucketKey {
            waste_category: format!("{:?}", finding.waste_category),
            remediation_kind: format!("{:?}", finding.suggested_remediation_kind),
            primary_crate: primary_crate(&f),
        };
        map.entry(key).or_default().push(f);
    }
    map.into_iter().map(|(key, members)| Bucket { key, members }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vox_effort_audit::hybrid::MeasuredCost;
    use vox_effort_audit::judge::schema::{JudgeFinding, RemediationKind, WasteCategory};
    use vox_effort_audit::output::{FindingRow, JudgeMeta};
    use vox_effort_audit::shape::{CommitKind, ShapeFeatures};

    /// Build a LoadedFinding with a chosen category, remediation kind, and
    /// evidence pointer (the latter drives `primary_crate`).
    fn loaded(
        sha: &str,
        category: WasteCategory,
        kind: RemediationKind,
        evidence: &str,
    ) -> LoadedFinding {
        let row = FindingRow {
            schema_version: "1.0".into(),
            commit_sha: sha.into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(),
            message_first_line: "test commit".into(),
            shape: ShapeFeatures {
                additions: 1,
                deletions: 0,
                files_changed: 1,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0,
                is_lockfile_only: false,
                is_generated_only: false,
                is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: JudgeMeta {
                model_id: "mock".into(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: Some(JudgeFinding {
                waste_score: 8,
                waste_category: category,
                suggested_remediation_kind: kind,
                rationale_one_line: "r".into(),
                evidence_pointers: vec![evidence.into()],
            }),
        };
        LoadedFinding { row }
    }

    #[test]
    fn crate_from_evidence_pointer() {
        assert_eq!(crate_from_path("crates/vox-config/src/timeouts.rs:8"), Some("vox-config".into()));
        assert_eq!(crate_from_path("README.md"), None);
    }

    #[test]
    fn identical_keys_join_one_bucket() {
        // Two findings with the same (waste_category, remediation_kind, crate),
        // plus one different → group() yields 2 buckets, the matched one holding 2.
        let a = loaded(
            "aaa",
            WasteCategory::MechanicalSweep,
            RemediationKind::ScriptAutomation,
            "crates/vox-config/src/timeouts.rs:8",
        );
        let b = loaded(
            "bbb",
            WasteCategory::MechanicalSweep,
            RemediationKind::ScriptAutomation,
            "crates/vox-config/src/limits.rs:12",
        );
        let c = loaded(
            "ccc",
            WasteCategory::LinterGap,
            RemediationKind::LinterRule,
            "crates/vox-actor-runtime/src/llm.rs:3",
        );

        let buckets = group(vec![a, b, c]);
        assert_eq!(buckets.len(), 2);
        let merged = buckets
            .iter()
            .find(|bk| bk.key.remediation_kind == "ScriptAutomation")
            .expect("ScriptAutomation bucket present");
        assert_eq!(merged.members.len(), 2);
        assert_eq!(merged.key.primary_crate, "vox-config");
    }
}
