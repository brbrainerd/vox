//! Optional `metadata_json.scientia_evidence` — ties publication manifests to Socrates, eval-gate, and benchmark artifacts.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod signals;
pub use signals::*;
pub mod markdown;
pub use markdown::*;
pub mod worthiness;
pub use worthiness::*;

use crate::scientific_metadata::ScientificPublicationMetadata;

/// Inline snapshot compatible with `vox_db::socrates_telemetry::SocratesSurfaceAggregate` JSON.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SocratesAggregateSnapshot {
    #[serde(default)]
    pub sample_size: usize,
    #[serde(default)]
    pub parsed_metadata_rows: usize,
    #[serde(default)]
    pub mean_hallucination_risk_proxy: f64,
    #[serde(default)]
    pub mean_confidence_estimate: f64,
    #[serde(default)]
    pub mean_contradiction_ratio: f64,
    #[serde(default)]
    pub answer_count: usize,
    #[serde(default)]
    pub ask_count: usize,
    #[serde(default)]
    pub abstain_count: usize,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct EvalGateSnapshot {
    pub passed: bool,
    #[serde(default)]
    pub gates_failed: usize,
    #[serde(default)]
    pub gates_total: usize,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct BenchmarkPairSnapshot {
    #[serde(default)]
    pub baseline_run_id: Option<String>,
    #[serde(default)]
    pub candidate_run_id: Option<String>,
    /// Repo-relative path to a benchmark manifest or run envelope (informational).
    #[serde(default)]
    pub manifest_repo_relative: Option<String>,
    #[serde(default)]
    pub pair_complete: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DraftPreparationHints {
    #[serde(default)]
    pub abstract_needed: bool,
    #[serde(default)]
    pub citations_needed: bool,
    #[serde(default)]
    pub reproducibility_details_needed: bool,
    #[serde(default)]
    pub ethics_statement_needed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_scholarly_venue: Option<String>,
}

/// Audit row for machine-filled facets in [`ScientiaEvidenceContext`].
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutofillProvenanceEntry {
    pub facet: String,
    pub origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Evidence bundle authors embed (or tools merge from live DB reads).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ScientiaEvidenceContext {
    #[serde(default)]
    pub socrates_aggregate: Option<SocratesAggregateSnapshot>,
    #[serde(default)]
    pub eval_gate: Option<EvalGateSnapshot>,
    #[serde(default)]
    pub benchmark: Option<BenchmarkPairSnapshot>,
    /// Machine-inferred reasons this draft may be worth preparing for publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovery_signals: Vec<DiscoverySignal>,
    /// Preparation work the system can tee up before a human writes the full paper/package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_preparation: Option<DraftPreparationHints>,
    /// Short machine-generated note summarizing why this draft was surfaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_note: Option<String>,
    /// Repo-relative directory passed to `vox mens eval-gate` / `check_run` (CLI only when `with_worthiness`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_gate_run_dir_repo_relative: Option<String>,
    /// Repo-relative JSON file with an [`EvalGateSnapshot`] (CLI + MCP); applied when `eval_gate` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_gate_report_repo_relative: Option<String>,
    /// Repo-relative JSON file with a [`BenchmarkPairSnapshot`]; applied when `benchmark` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_pair_report_repo_relative: Option<String>,
    /// Human attestation that there is a substantive novel result (never inferred from heuristics alone).
    #[serde(default)]
    pub human_meaningful_advance: bool,
    /// Human attestation that AI / generative-tool disclosure meets target-venue policy (sets compliance to 1.0 when true).
    #[serde(default)]
    pub human_ai_disclosure_complete: bool,
    /// Additional research doc / ADR paths linked into the evidence graph (repo-relative).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_doc_repo_relatives: Vec<String>,
    /// Repo-relative Mens scorecard or benchmark spec artifact (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mens_scorecard_repo_relative: Option<String>,
    /// Checksum or reproducibility manifest (repo-relative).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproducibility_manifest_repo_relative: Option<String>,
    /// Trust rollup or trust telemetry export (repo-relative).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_rollup_repo_relative: Option<String>,
    /// Detected inconsistencies between attestations and snapshots (never auto-resolved).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signal_conflicts: Vec<SignalConflict>,
    /// Best-effort outline from markdown headings (prepare / refresh).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc_section_hints: Vec<DocSectionHint>,
    /// Which machine facets were attached for audit (anti-slop transparency).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub autofill_provenance: Vec<AutofillProvenanceEntry>,
    /// Phase D wiring — summary of the most recent claim extraction +
    /// MiniCheck verification over the manuscript body. Written by
    /// `vox scientia publication-extract-claims` and consumed by
    /// `worthiness_extraction.rs` to derive
    /// [`WorthinessInputs::claim_evidence_coverage`] from a measured
    /// support ratio instead of a citation-presence heuristic.
    ///
    /// [`WorthinessInputs::claim_evidence_coverage`]: crate::publication_worthiness::WorthinessInputs::claim_evidence_coverage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_claims: Option<ExtractedClaimsSummary>,
}

/// Summary written to `metadata_json.scientia_evidence.extracted_claims` by
/// `publication-extract-claims`. The full per-claim verdicts live in
/// `scientia_claim_verdicts`; this struct is the aggregate that worthiness
/// derivation reads.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExtractedClaimsSummary {
    pub schema_version: u32,
    /// Total atomic claims that survived VeriScore + span checks.
    pub total_atomic: u32,
    /// Verdicts where `support_score >= promotion_threshold`.
    pub supported: u32,
    /// Verdicts where the verifier returned a contradicting verdict.
    pub refuted: u32,
    /// Verdicts where `support_score < abstain_threshold`.
    pub abstained: u32,
    /// Verifier identifier (e.g. `"mock"` or `"minicheck-ft5"`).
    pub verifier_model: String,
    pub abstain_threshold: f64,
    pub promotion_threshold: f64,
    pub extracted_at_ms: i64,
}

impl ExtractedClaimsSummary {
    /// Measured support ratio in `[0, 1]`. Used by worthiness derivation to
    /// override the citation-presence heuristic when present. Returns
    /// `0.0` when `total_atomic == 0` (no claims were extractable — the
    /// rubric treats this as "claim_evidence_coverage cannot be measured"
    /// and the caller falls back to the existing heuristic).
    pub fn support_ratio(&self) -> f64 {
        if self.total_atomic == 0 {
            0.0
        } else {
            self.supported as f64 / self.total_atomic as f64
        }
    }
}

impl ScientiaEvidenceContext {
    /// Repo-relative path for benchmark pair provenance: linked report path (manifest metadata), else inline snapshot path.
    #[must_use]
    pub fn benchmark_pair_repo_relative(&self) -> Option<String> {
        self.benchmark_pair_report_repo_relative
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                self.benchmark.as_ref().and_then(|b| {
                    b.manifest_repo_relative
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
            })
    }

    /// Baseline and candidate run ids joined for provenance when a pair snapshot is present.
    #[must_use]
    pub fn benchmark_pair_run_label(&self) -> Option<String> {
        signals::benchmark_pair_run_label(self.benchmark.as_ref())
    }
}

/// Read `scientia_evidence` from manifest metadata (best-effort).
#[must_use]
pub fn parse_scientia_evidence(metadata_json: Option<&str>) -> Option<ScientiaEvidenceContext> {
    let raw = metadata_json?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let root: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let v = root.get(signals::METADATA_KEY_SCIENTIA_EVIDENCE)?;
    serde_json::from_value(v.clone()).ok()
}

/// Derive [`DocSectionHint`] and [`AutofillProvenanceEntry`] rows after [`populate_candidate_context_defaults`].
pub fn attach_autofill_and_doc_hints(body_markdown: &str, evidence: &mut ScientiaEvidenceContext) {
    evidence.doc_section_hints = markdown::infer_doc_sections_from_markdown(body_markdown);
    let mut prov = Vec::new();
    if !evidence.doc_section_hints.is_empty() {
        prov.push(AutofillProvenanceEntry {
            facet: "doc_section_hints".into(),
            origin: "markdown_headings".into(),
            notes: Some(format!("count={}", evidence.doc_section_hints.len())),
        });
    }
    if evidence
        .candidate_note
        .as_ref()
        .is_some_and(|n: &String| n.starts_with("Draft candidate surfaced"))
    {
        prov.push(AutofillProvenanceEntry {
            facet: "candidate_note".into(),
            origin: "discovery_signal_summary".into(),
            notes: None,
        });
    }
    if let Some(ref dp) = evidence.draft_preparation {
        let mut flags: Vec<&'static str> = Vec::new();
        if dp.abstract_needed {
            flags.push("abstract_needed");
        }
        if dp.citations_needed {
            flags.push("citations_needed");
        }
        if dp.reproducibility_details_needed {
            flags.push("reproducibility_details_needed");
        }
        if dp.ethics_statement_needed {
            flags.push("ethics_statement_needed");
        }
        if let Some(ref v) = dp.recommended_scholarly_venue
            && !v.trim().is_empty()
        {
            flags.push("recommended_scholarly_venue");
        }
        if !flags.is_empty() {
            prov.push(AutofillProvenanceEntry {
                facet: "draft_preparation".into(),
                origin: "structural_gap_scan".into(),
                notes: Some(flags.join(",")),
            });
        }
    }
    evidence.autofill_provenance = prov;
}

/// Re-read `scientia_evidence` from `metadata_json`, re-run population + doc hints, and merge back (preserves sibling keys).
pub fn rebuild_scientia_evidence_metadata_json(
    metadata_json: Option<&str>,
    body_markdown: &str,
    abstract_text: Option<&str>,
    citations_json: Option<&str>,
    scientific: Option<&ScientificPublicationMetadata>,
    source_ref: Option<&str>,
    prepared_by: Option<&str>,
) -> serde_json::Result<String> {
    let mut evidence = parse_scientia_evidence(metadata_json).unwrap_or_default();
    populate_candidate_context_defaults(
        source_ref,
        abstract_text,
        citations_json,
        scientific,
        &mut evidence,
    );
    attach_autofill_and_doc_hints(body_markdown, &mut evidence);
    crate::scientific_metadata::merge_scientia_evidence_into_metadata_json(
        metadata_json,
        &evidence,
        prepared_by,
    )
}

/// Infer factual preparation gaps the system can tee up before human drafting.
#[must_use]
pub fn infer_draft_preparation_hints(
    abstract_text: Option<&str>,
    citations_json: Option<&str>,
    scientific: Option<&ScientificPublicationMetadata>,
    evidence: &ScientiaEvidenceContext,
) -> DraftPreparationHints {
    let reproducibility_present = scientific
        .and_then(|s| s.reproducibility.as_ref())
        .is_some_and(|r| {
            r.code_repository_url
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
                || r.data_repository_url
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty())
                || r.artifact_checksum_note
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty())
        });
    let ethics_present = scientific
        .and_then(|s| s.ethics_and_impact.as_ref())
        .is_some_and(|e| {
            e.broader_impact_statement
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
                || e.irb_or_human_subjects_note
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty())
        });
    let has_strong_signal = signals::infer_discovery_signals(None, evidence)
        .iter()
        .any(|s| s.strength == signals::DiscoverySignalStrength::Strong);
    DraftPreparationHints {
        abstract_needed: abstract_text.is_none_or(|s| s.trim().is_empty()),
        citations_needed: citations_json.is_none_or(|s| s.trim().is_empty()),
        reproducibility_details_needed: !reproducibility_present
            && (evidence.eval_gate.is_some() || evidence.benchmark.is_some()),
        ethics_statement_needed: !ethics_present && has_strong_signal,
        recommended_scholarly_venue: if has_strong_signal {
            Some("arxiv_assist".to_string())
        } else {
            None
        },
    }
}

/// Fill missing discovery signals / preparation hints / candidate note using current evidence and source context.
pub fn populate_candidate_context_defaults(
    source_ref: Option<&str>,
    abstract_text: Option<&str>,
    citations_json: Option<&str>,
    scientific: Option<&ScientificPublicationMetadata>,
    evidence: &mut ScientiaEvidenceContext,
) {
    if evidence.discovery_signals.is_empty() {
        evidence.discovery_signals = signals::infer_discovery_signals(source_ref, evidence);
    }
    if evidence.draft_preparation.is_none() {
        evidence.draft_preparation = Some(infer_draft_preparation_hints(
            abstract_text,
            citations_json,
            scientific,
            evidence,
        ));
    }
    if evidence.candidate_note.is_none() && !evidence.discovery_signals.is_empty() {
        let summary = evidence
            .discovery_signals
            .iter()
            .take(3)
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        evidence.candidate_note = Some(format!(
            "Draft candidate surfaced from structured signals. {summary}"
        ));
    }
    evidence.signal_conflicts = signals::detect_signal_conflicts(evidence);
}

fn trim_repo_rel_path(s: &str) -> String {
    s.trim()
        .trim_start_matches('/')
        .trim_start_matches('\\')
        .to_string()
}

/// Read sidecar JSON files referenced under `scientia_evidence` and merge into `metadata_json` (no `check_run`).
///
/// Runs after live Socrates merge and (in the CLI) after eval-gate directory checks when those set `eval_gate`.
/// Fills `eval_gate` / `benchmark` only when the corresponding snapshot field is still `None`.
pub fn enrich_metadata_json_with_repo_files(
    metadata_json: Option<&str>,
    repo_root: &Path,
) -> anyhow::Result<Option<String>> {
    let Some(raw) = metadata_json else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let mut root: serde_json::Value = serde_json::from_str(trimmed)
        .with_context(|| "parse metadata_json for scientia_evidence file hydration")?;

    let mut ev: ScientiaEvidenceContext = root
        .get(signals::METADATA_KEY_SCIENTIA_EVIDENCE)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let mut changed = false;

    if ev.eval_gate.is_none()
        && let Some(ref rel) = ev.eval_gate_report_repo_relative.clone()
    {
        let part = trim_repo_rel_path(rel);
        if !part.is_empty() {
            let p = repo_root.join(&part);
            if p.is_file() {
                let txt = vox_bounded_fs::read_utf8_path_capped(&p)
                    .with_context(|| format!("read eval_gate report {}", p.display()))?;
                let g: EvalGateSnapshot = serde_json::from_str(&txt)
                    .with_context(|| format!("parse eval_gate report JSON {}", p.display()))?;
                ev.eval_gate = Some(g);
                changed = true;
            }
        }
    }

    if ev.benchmark.is_none()
        && let Some(ref rel) = ev.benchmark_pair_report_repo_relative.clone()
    {
        let part = trim_repo_rel_path(rel);
        if !part.is_empty() {
            let p = repo_root.join(&part);
            if p.is_file() {
                let txt = vox_bounded_fs::read_utf8_path_capped(&p)
                    .with_context(|| format!("read benchmark pair report {}", p.display()))?;
                let b: BenchmarkPairSnapshot = serde_json::from_str(&txt)
                    .with_context(|| format!("parse benchmark pair report JSON {}", p.display()))?;
                ev.benchmark = Some(b);
                changed = true;
            }
        }
    }

    if !changed {
        return Ok(None);
    }

    root[signals::METADATA_KEY_SCIENTIA_EVIDENCE] = serde_json::to_value(&ev)?;
    Ok(Some(serde_json::to_string(&root)?))
}

#[cfg(test)]
mod tests;
