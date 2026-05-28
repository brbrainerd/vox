use super::*;
use serde::{Deserialize, Serialize};

/// Root `PublicationManifest::metadata_json` key for this block.
pub const METADATA_KEY_SCIENTIA_EVIDENCE: &str = "scientia_evidence";

/// Optional [`NoveltyEvidenceBundleV1`](crate::scientia_finding_ledger::NoveltyEvidenceBundleV1) under manifest metadata (prior-art scan results).
pub const METADATA_KEY_SCIENTIA_NOVELTY_BUNDLE: &str = "scientia_novelty_bundle";

/// Machine suggestion / contract labels (anti-slop: never treated as ground truth).
pub const SCIENTIA_LABEL_MACHINE_SUGGESTED: &str = "machine_suggested";
pub const SCIENTIA_LABEL_REQUIRES_HUMAN_REVIEW: &str = "requires_human_review";
pub const SCIENTIA_LABEL_SOURCE_GROUNDED: &str = "source_grounded";

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySignalStrength {
    #[default]
    Supporting,
    Strong,
    /// Non-dispositive context (telemetry paths, informational-only artifacts).
    Informational,
}

/// Typed signal family for contracts and deterministic ranking (no LLM labels).
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySignalFamily {
    #[default]
    Unspecified,
    EvalGate,
    BenchmarkPair,
    Documentation,
    TelemetryAggregate,
    OperatorAttestation,
    MensScorecard,
    TrustRollup,
    ReproducibilityArtifact,
    LinkedCorpus,
    /// Synthetic / ledger-aligned signal (finding-candidate v1).
    FindingCandidateSignal,
}

/// Best-effort provenance for audit trails (`contracts/scientia/discovery-signal.schema.json`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DiscoverySignalProvenance {
    /// High-level origin, e.g. `repository_path`, `manifest_metadata`, `database_snapshot`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DiscoverySignal {
    pub code: String,
    pub summary: String,
    #[serde(default)]
    pub strength: DiscoverySignalStrength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub family: DiscoverySignalFamily,
    #[serde(default)]
    pub provenance: DiscoverySignalProvenance,
}

/// When structured evidence points in incompatible directions for the same manifest.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SignalConflict {
    #[serde(default)]
    pub codes: Vec<String>,
    #[serde(default)]
    pub summary: String,
}

/// Detect contradictory structured evidence (explicitly excludes raw git churn heuristics).
#[must_use]
pub fn detect_signal_conflicts(evidence: &ScientiaEvidenceContext) -> Vec<SignalConflict> {
    let mut v = Vec::new();
    if evidence.human_meaningful_advance && evidence.eval_gate.as_ref().is_some_and(|g| !g.passed) {
        v.push(SignalConflict {
            codes: vec![
                "operator_marked_meaningful_advance".to_string(),
                "eval_gate_failed".to_string(),
            ],
            summary: "Human attested meaningful advance but the attached eval-gate snapshot did not pass — reconcile before claiming scientific readiness.".to_string(),
        });
    }
    if evidence.human_meaningful_advance
        && evidence.benchmark.as_ref().is_some_and(|b| {
            !b.pair_complete
                || b.baseline_run_id
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
                || b.candidate_run_id
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
        })
    {
        v.push(SignalConflict {
            codes: vec![
                "operator_marked_meaningful_advance".to_string(),
                "benchmark_pair_incomplete".to_string(),
            ],
            summary: "Human attested meaningful advance but benchmark pair is incomplete or missing run identifiers — fill benchmark evidence or clear the attestation.".to_string(),
        });
    }
    if evidence
        .eval_gate
        .as_ref()
        .is_some_and(|g| g.passed && g.gates_failed > 0)
    {
        v.push(SignalConflict {
            codes: vec!["eval_gate_passed".to_string(), "eval_gate_failures_nonzero".to_string()],
            summary: "Eval-gate snapshot reports passed=true alongside gates_failed>0 — verify the eval-gate export integrity.".to_string(),
        });
    }
    v
}

/// Infer a first-pass discovery signal set from structured evidence and source location.
#[must_use]
pub fn infer_discovery_signals(
    source_ref: Option<&str>,
    evidence: &ScientiaEvidenceContext,
) -> Vec<DiscoverySignal> {
    let mut out = Vec::new();
    let source_ref = source_ref.map(str::trim).filter(|s| !s.is_empty());
    if let Some(src) = source_ref {
        let normalized = src.replace('\\', "/").to_ascii_lowercase();
        if normalized.starts_with("docs/src/adr/") || normalized.contains("/docs/src/adr/") {
            push_signal_unique(
                &mut out,
                DiscoverySignal {
                    code: "adr_writeup_present".to_string(),
                    summary: "An ADR-style writeup already exists for this candidate.".to_string(),
                    strength: DiscoverySignalStrength::Supporting,
                    source_ref: Some(src.to_string()),
                    family: DiscoverySignalFamily::Documentation,
                    provenance: DiscoverySignalProvenance {
                        origin: Some("repository_path".into()),
                        repo_path: Some(src.to_string()),
                        ..Default::default()
                    },
                },
            );
        }
        if normalized.starts_with("docs/src/research/")
            || normalized.contains("/docs/src/research/")
            || normalized.starts_with("docs/src/architecture/")
            || normalized.contains("/docs/src/architecture/")
            || normalized.starts_with("docs/src/reference/")
            || normalized.contains("/docs/src/reference/")
        {
            push_signal_unique(
                &mut out,
                DiscoverySignal {
                    code: "research_writeup_present".to_string(),
                    summary:
                        "A repository writeup already exists and can seed the publication draft."
                            .to_string(),
                    strength: DiscoverySignalStrength::Supporting,
                    source_ref: Some(src.to_string()),
                    family: DiscoverySignalFamily::Documentation,
                    provenance: DiscoverySignalProvenance {
                        origin: Some("repository_path".into()),
                        repo_path: Some(src.to_string()),
                        ..Default::default()
                    },
                },
            );
        }
    }

    if let Some(ref g) = evidence.eval_gate {
        if g.passed {
            push_signal_unique(
                &mut out,
                DiscoverySignal {
                    code: "eval_gate_passed".to_string(),
                    summary: "An attached eval-gate snapshot passed.".to_string(),
                    strength: DiscoverySignalStrength::Strong,
                    source_ref: evidence.eval_gate_report_repo_relative.clone(),
                    family: DiscoverySignalFamily::EvalGate,
                    provenance: DiscoverySignalProvenance {
                        origin: Some("manifest_metadata".into()),
                        repo_path: evidence.eval_gate_report_repo_relative.clone(),
                        metric_type: Some("eval_gate".into()),
                        ..Default::default()
                    },
                },
            );
        } else {
            push_signal_unique(
                &mut out,
                DiscoverySignal {
                    code: "eval_gate_failed".to_string(),
                    summary: "Attached eval-gate snapshot did not pass (blocking for auto-draft)."
                        .to_string(),
                    strength: DiscoverySignalStrength::Supporting,
                    source_ref: evidence.eval_gate_report_repo_relative.clone(),
                    family: DiscoverySignalFamily::EvalGate,
                    provenance: DiscoverySignalProvenance {
                        origin: Some("manifest_metadata".into()),
                        repo_path: evidence.eval_gate_report_repo_relative.clone(),
                        metric_type: Some("eval_gate".into()),
                        ..Default::default()
                    },
                },
            );
        }
    }

    if evidence.benchmark.as_ref().is_some_and(|b| {
        b.pair_complete
            && b.baseline_run_id
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
            && b.candidate_run_id
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
    }) {
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "benchmark_pair_complete".to_string(),
                summary: "A baseline/candidate benchmark pair is attached and marked complete."
                    .to_string(),
                strength: DiscoverySignalStrength::Strong,
                source_ref: evidence.benchmark_pair_report_repo_relative.clone(),
                family: DiscoverySignalFamily::BenchmarkPair,
                provenance: DiscoverySignalProvenance {
                    origin: Some("manifest_metadata".into()),
                    repo_path: evidence.benchmark_pair_repo_relative(),
                    run_id: evidence.benchmark_pair_run_label(),
                    metric_type: Some("benchmark_pair".into()),
                    ..Default::default()
                },
            },
        );
    }

    if evidence
        .socrates_aggregate
        .as_ref()
        .is_some_and(|a| a.sample_size > 0 || a.parsed_metadata_rows > 0 || a.answer_count > 0)
    {
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "socrates_telemetry_present".to_string(),
                summary: "Socrates telemetry aggregate is attached (supporting context only)."
                    .to_string(),
                strength: DiscoverySignalStrength::Informational,
                source_ref: None,
                family: DiscoverySignalFamily::TelemetryAggregate,
                provenance: DiscoverySignalProvenance {
                    origin: Some("database_snapshot".into()),
                    metric_type: Some("socrates_surface_aggregate".into()),
                    ..Default::default()
                },
            },
        );
    }

    if evidence
        .mens_scorecard_repo_relative
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        let rel = evidence.mens_scorecard_repo_relative.clone();
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "mens_scorecard_path_present".to_string(),
                summary: "Mens scorecard or spec path is linked for reproducibility.".to_string(),
                strength: DiscoverySignalStrength::Supporting,
                source_ref: rel.clone(),
                family: DiscoverySignalFamily::MensScorecard,
                provenance: DiscoverySignalProvenance {
                    origin: Some("repository_path".into()),
                    repo_path: rel,
                    metric_type: Some("mens_scorecard".into()),
                    ..Default::default()
                },
            },
        );
    }

    if evidence
        .reproducibility_manifest_repo_relative
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        let rel = evidence.reproducibility_manifest_repo_relative.clone();
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "reproducibility_manifest_present".to_string(),
                summary: "A reproducibility or checksum manifest path is attached.".to_string(),
                strength: DiscoverySignalStrength::Supporting,
                source_ref: rel.clone(),
                family: DiscoverySignalFamily::ReproducibilityArtifact,
                provenance: DiscoverySignalProvenance {
                    origin: Some("repository_path".into()),
                    repo_path: rel,
                    ..Default::default()
                },
            },
        );
    }

    if evidence
        .trust_rollup_repo_relative
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        let rel = evidence.trust_rollup_repo_relative.clone();
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "trust_rollup_path_present".to_string(),
                summary: "Trust rollup export path is linked (informational).".to_string(),
                strength: DiscoverySignalStrength::Informational,
                source_ref: rel.clone(),
                family: DiscoverySignalFamily::TrustRollup,
                provenance: DiscoverySignalProvenance {
                    origin: Some("repository_path".into()),
                    repo_path: rel,
                    metric_type: Some("trust_rollup".into()),
                    ..Default::default()
                },
            },
        );
    }

    if !evidence.linked_doc_repo_relatives.is_empty() {
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "linked_research_docs_present".to_string(),
                summary: "Additional research/doc paths are linked into the evidence graph."
                    .to_string(),
                strength: DiscoverySignalStrength::Supporting,
                source_ref: evidence.linked_doc_repo_relatives.first().cloned(),
                family: DiscoverySignalFamily::LinkedCorpus,
                provenance: DiscoverySignalProvenance {
                    origin: Some("manifest_metadata".into()),
                    ..Default::default()
                },
            },
        );
    }

    if evidence.human_meaningful_advance {
        push_signal_unique(
            &mut out,
            DiscoverySignal {
                code: "operator_marked_meaningful_advance".to_string(),
                summary: "A human explicitly marked this work as a meaningful advance candidate."
                    .to_string(),
                strength: DiscoverySignalStrength::Strong,
                source_ref: None,
                family: DiscoverySignalFamily::OperatorAttestation,
                provenance: DiscoverySignalProvenance {
                    origin: Some("operator_attestation".into()),
                    ..Default::default()
                },
            },
        );
    }
    out
}

fn push_signal_unique(out: &mut Vec<DiscoverySignal>, signal: DiscoverySignal) {
    if out.iter().any(|existing| existing.code == signal.code) {
        return;
    }
    out.push(signal);
}

pub fn benchmark_pair_run_label(
    b: Option<&crate::scientia_evidence::BenchmarkPairSnapshot>,
) -> Option<String> {
    let b = b?;
    Some(format!(
        "{}|{}",
        b.baseline_run_id.as_deref().unwrap_or(""),
        b.candidate_run_id.as_deref().unwrap_or("")
    ))
}
