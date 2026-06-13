//! Machine-readable publication-worthiness policy (`contracts/scientia/*.yaml`) and evaluation.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

/// Default contract path relative to repository root.
pub const DEFAULT_CONTRACT_REL_PATH: &str =
    "contracts/scientia/publication-worthiness.default.yaml";

/// JSON Schema path relative to repository root (validated by `vox ci scientia-worthiness-contract`).
pub const CONTRACT_SCHEMA_REL_PATH: &str = "contracts/scientia/publication-worthiness.schema.json";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PublicationWorthinessContract {
    pub version: u32,
    pub decision_labels: DecisionLabels,
    pub hard_red_lines: Vec<HardRedLine>,
    pub thresholds: Thresholds,
    pub weights: Weights,
    /// Advisory venue notes only; `evaluate_worthiness` does not execute these checks yet.
    pub venue_profiles: std::collections::BTreeMap<String, VenueProfile>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DecisionLabels {
    pub publish: String,
    pub ask_for_evidence: String,
    #[serde(rename = "abstain_do_not_publish")]
    pub abstain_do_not_publish: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct HardRedLine {
    pub id: String,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Thresholds {
    pub claim_evidence_coverage_min: f64,
    pub artifact_replayability_min: f64,
    pub before_after_pair_integrity_min: f64,
    pub metadata_completeness_min: f64,
    pub ai_disclosure_compliance_exact: f64,
    pub publish_score_min: f64,
    pub abstain_score_max: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Weights {
    pub epistemic: f64,
    pub reproducibility: f64,
    pub novelty: f64,
    pub reliability: f64,
    pub metadata_policy: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct VenueProfile {
    pub description: String,
    pub required_checks: Vec<String>,
}

/// Inputs for [`evaluate_worthiness`]; typically deserialized from JSON.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WorthinessInputs {
    /// Red-line ids that the caller attests were violated (must match enabled contract rows).
    #[serde(default)]
    pub red_line_violation_ids: Vec<String>,
    #[serde(default)]
    pub repeated_unresolved_contradiction: bool,
    pub claim_evidence_coverage: f64,
    /// Operator-declared replayability. When `artifact_replayability_measured`
    /// is `Some`, that measured value supersedes this declared one for
    /// hard-gate checks. Producers of this struct should keep both populated
    /// so consumers can compare declared vs measured during diagnostics.
    pub artifact_replayability: f64,
    /// Phase B: measured replayability written back by `vox-replay-runner`
    /// after sandboxed re-execution of the manifest's RO-Crate `mainEntity`.
    /// `None` means "replay has not been measured yet"; downstream gates
    /// fall back to `artifact_replayability` (declared).
    #[serde(default)]
    pub artifact_replayability_measured: Option<f64>,
    pub before_after_pair_integrity: f64,
    pub metadata_completeness: f64,
    pub ai_disclosure_compliance: f64,
    pub epistemic: f64,
    pub reproducibility: f64,
    pub novelty: f64,
    pub reliability: f64,
    pub metadata_policy: f64,
    /// When true, `mdl_gain_proxy` / `delta_signal_to_noise` (or human review) supports a real advance.
    #[serde(default)]
    pub meaningful_advance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorthinessDecision {
    Publish,
    AskForEvidence,
    AbstainDoNotPublish,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorthinessEvaluation {
    pub decision: WorthinessDecision,
    pub decision_label: String,
    pub worthiness_score: f64,
    pub hard_metrics_ok: bool,
    pub reasons: Vec<String>,
}

/// Parse the worthiness contract YAML (repo file body).
pub fn load_contract_from_str(yaml: &str) -> Result<PublicationWorthinessContract> {
    serde_yaml::from_str(yaml).context("parse publication worthiness YAML")
}

/// Structural checks beyond JSON Schema (weights, ordering).
pub fn validate_contract_invariants(c: &PublicationWorthinessContract) -> Result<()> {
    let sum = c.weights.epistemic
        + c.weights.reproducibility
        + c.weights.novelty
        + c.weights.reliability
        + c.weights.metadata_policy;
    if (sum - 1.0).abs() > 1e-5 {
        return Err(anyhow!(
            "weights must sum to 1.0 (got {sum}; epistemic={}, repro={}, novelty={}, reliability={}, metadata_policy={})",
            c.weights.epistemic,
            c.weights.reproducibility,
            c.weights.novelty,
            c.weights.reliability,
            c.weights.metadata_policy
        ));
    }
    if c.thresholds.publish_score_min <= c.thresholds.abstain_score_max {
        return Err(anyhow!(
            "publish_score_min ({}) must be greater than abstain_score_max ({})",
            c.thresholds.publish_score_min,
            c.thresholds.abstain_score_max
        ));
    }
    for (name, v) in [
        (
            "claim_evidence_coverage_min",
            c.thresholds.claim_evidence_coverage_min,
        ),
        (
            "artifact_replayability_min",
            c.thresholds.artifact_replayability_min,
        ),
        (
            "before_after_pair_integrity_min",
            c.thresholds.before_after_pair_integrity_min,
        ),
        (
            "metadata_completeness_min",
            c.thresholds.metadata_completeness_min,
        ),
        (
            "ai_disclosure_compliance_exact",
            c.thresholds.ai_disclosure_compliance_exact,
        ),
        ("publish_score_min", c.thresholds.publish_score_min),
        ("abstain_score_max", c.thresholds.abstain_score_max),
    ] {
        range_01(name, v)?;
    }
    for (name, v) in [
        ("weight.epistemic", c.weights.epistemic),
        ("weight.reproducibility", c.weights.reproducibility),
        ("weight.novelty", c.weights.novelty),
        ("weight.reliability", c.weights.reliability),
        ("weight.metadata_policy", c.weights.metadata_policy),
    ] {
        range_01(name, v)?;
    }
    Ok(())
}

fn range_01(name: &str, v: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&v) {
        return Err(anyhow!("{name} must be in [0,1] (got {v})"));
    }
    Ok(())
}

fn label_for(decision: WorthinessDecision, c: &PublicationWorthinessContract) -> String {
    match decision {
        WorthinessDecision::Publish => c.decision_labels.publish.clone(),
        WorthinessDecision::AskForEvidence => c.decision_labels.ask_for_evidence.clone(),
        WorthinessDecision::AbstainDoNotPublish => c.decision_labels.abstain_do_not_publish.clone(),
    }
}

/// Apply the default rubric: red lines and low aggregate abstain; metric floors gate publish; else ask.
pub fn evaluate_worthiness(
    c: &PublicationWorthinessContract,
    inputs: &WorthinessInputs,
) -> WorthinessEvaluation {
    let mut reasons: Vec<String> = Vec::new();

    let enabled_ids: std::collections::HashSet<&str> = c
        .hard_red_lines
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.id.as_str())
        .collect();

    let mut active_violations: Vec<&str> = Vec::new();
    for id in &inputs.red_line_violation_ids {
        if enabled_ids.contains(id.as_str()) {
            active_violations.push(id.as_str());
        }
    }
    if !active_violations.is_empty() {
        reasons.push(format!(
            "enabled hard red-line violations: {}",
            active_violations.join(", ")
        ));
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: aggregate_score(c, inputs),
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    if inputs.repeated_unresolved_contradiction {
        reasons.push("repeated_unresolved_contradiction".to_string());
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: aggregate_score(c, inputs),
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    let score = aggregate_score(c, inputs);
    if score < c.thresholds.abstain_score_max {
        reasons.push(format!(
            "worthiness_score {score:.4} < abstain_score_max {}",
            c.thresholds.abstain_score_max
        ));
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: score,
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    let hard_ok = hard_metrics_ok(c, inputs);
    if !hard_ok {
        reasons.push("one_or_more_hard_metric_minimums_not_met".to_string());
        return WorthinessEvaluation {
            decision: WorthinessDecision::AskForEvidence,
            decision_label: label_for(WorthinessDecision::AskForEvidence, c),
            worthiness_score: score,
            hard_metrics_ok: false,
            reasons,
        };
    }

    if score >= c.thresholds.publish_score_min && inputs.meaningful_advance {
        reasons.push("hard_metrics_ok_and_publish_score_with_meaningful_advance".to_string());
        WorthinessEvaluation {
            decision: WorthinessDecision::Publish,
            decision_label: label_for(WorthinessDecision::Publish, c),
            worthiness_score: score,
            hard_metrics_ok: true,
            reasons,
        }
    } else {
        if score < c.thresholds.publish_score_min {
            reasons.push(format!(
                "worthiness_score {score:.4} < publish_score_min {}",
                c.thresholds.publish_score_min
            ));
        }
        if !inputs.meaningful_advance {
            reasons.push("meaningful_advance_required_for_publish".to_string());
        }
        WorthinessEvaluation {
            decision: WorthinessDecision::AskForEvidence,
            decision_label: label_for(WorthinessDecision::AskForEvidence, c),
            worthiness_score: score,
            hard_metrics_ok: true,
            reasons,
        }
    }
}

fn hard_metrics_ok(c: &PublicationWorthinessContract, inputs: &WorthinessInputs) -> bool {
    inputs.claim_evidence_coverage >= c.thresholds.claim_evidence_coverage_min
        && effective_replayability(inputs) >= c.thresholds.artifact_replayability_min
        && inputs.before_after_pair_integrity >= c.thresholds.before_after_pair_integrity_min
        && inputs.metadata_completeness >= c.thresholds.metadata_completeness_min
        && (inputs.ai_disclosure_compliance - c.thresholds.ai_disclosure_compliance_exact).abs()
            < 1e-9
}

/// The replayability value the worthiness rubric should actually gate on:
/// the measured value when `vox-replay-runner` has populated it, otherwise
/// the operator-declared value.
pub fn effective_replayability(inputs: &WorthinessInputs) -> f64 {
    inputs
        .artifact_replayability_measured
        .unwrap_or(inputs.artifact_replayability)
}

fn aggregate_score(c: &PublicationWorthinessContract, inputs: &WorthinessInputs) -> f64 {
    c.weights.epistemic * inputs.epistemic
        + c.weights.reproducibility * inputs.reproducibility
        + c.weights.novelty * inputs.novelty
        + c.weights.reliability * inputs.reliability
        + c.weights.metadata_policy * inputs.metadata_policy
}

/// Low novelty cap applied when the typed decision layer rules a candidate
/// non-novel for a hard reason (insufficient evidence or contradiction).
const NOVELTY_NON_NOVEL_CAP: f64 = 0.1;

/// Adapt the publisher-local [`crate::scientia_finding_ledger::NoveltyEvidenceBundleV1`]
/// into the vox-scientia retrieval-events [`NoveltyEvidenceBundle`] the typed
/// decision layer scores over.
///
/// The two schemas are field-isomorphic; this maps the local enums onto their
/// `vox_research_events` counterparts. `schema_version` widens `i32 -> u32`
/// (negative values clamp to 0). An empty `query_traces` vec maps to `None`
/// (the scorer treats "no traces" as genuine no-signal rather than failed
/// retrieval). No fields are fabricated; nothing in V1 is dropped.
fn adapt_bundle_v1(
    bundle: &crate::scientia_finding_ledger::NoveltyEvidenceBundleV1,
) -> vox_research_events::schema_types::NoveltyEvidenceBundle {
    use vox_research_events::schema_types as st;

    fn map_source(s: crate::scientia_finding_ledger::PriorArtSource) -> st::NoveltySource {
        use crate::scientia_finding_ledger::PriorArtSource as P;
        match s {
            P::Openalex => st::NoveltySource::Openalex,
            P::Crossref => st::NoveltySource::Crossref,
            P::SemanticScholar => st::NoveltySource::SemanticScholar,
            P::Manual => st::NoveltySource::Manual,
            P::Other => st::NoveltySource::Other,
        }
    }
    fn map_recency(r: crate::scientia_finding_ledger::NoveltyRecencyBucket) -> st::RecencyBucket {
        use crate::scientia_finding_ledger::NoveltyRecencyBucket as B;
        match r {
            B::Unknown => st::RecencyBucket::Unknown,
            B::Stale => st::RecencyBucket::Stale,
            B::Recent => st::RecencyBucket::Recent,
            B::VeryRecent => st::RecencyBucket::VeryRecent,
        }
    }

    let normalized_hits = bundle
        .normalized_hits
        .iter()
        .map(|h| st::NormalizedHit {
            source: map_source(h.source),
            work_uri: h.work_uri.clone(),
            title: h.title.clone(),
            year: h.year,
            lexical_score: h.lexical_score,
            semantic_score: h.semantic_score,
            overlap_note: h.overlap_note.clone(),
            cited_by_count: h.cited_by_count,
        })
        .collect();

    let overlap_summary = bundle.overlap_summary.as_ref().map(|o| st::OverlapSummary {
        max_lexical_score: o.max_lexical_score,
        max_semantic_score: o.max_semantic_score,
        recency_bucket: Some(map_recency(o.recency_bucket)),
    });

    let query_traces = if bundle.query_traces.is_empty() {
        None
    } else {
        Some(
            bundle
                .query_traces
                .iter()
                .map(|t| st::QueryTrace {
                    source: t.source.clone(),
                    request_fingerprint_sha256: t.request_fingerprint_sha256.clone(),
                    http_status: t.http_status,
                    cached: t.cached,
                })
                .collect(),
        )
    };

    st::NoveltyEvidenceBundle {
        schema_version: u32::try_from(bundle.schema_version).unwrap_or(0),
        bundle_id: bundle.bundle_id.clone(),
        candidate_id: bundle.candidate_id.clone(),
        computed_at_ms: bundle.computed_at_ms,
        query_digest_sha256: bundle.query_digest_sha256.clone(),
        sources: bundle.sources.iter().copied().map(map_source).collect(),
        normalized_hits,
        overlap_summary,
        query_traces,
    }
}

/// Derive opposing-polarity [`PolarizedHit`]s from a hit's `overlap_note`.
///
/// Polarity is read from a `polarity:<support|refute>` marker (the SCIENTIA
/// claim-extractor convention) or, failing that, from substring cues. Hits
/// with no polarity cue are `Neutral` and cannot, on their own, create a
/// conflict. Similarity uses `semantic_score` (falling back to `lexical_score`).
fn polarized_hits_from_bundle(
    bundle: &vox_research_events::schema_types::NoveltyEvidenceBundle,
) -> Vec<vox_scientia::inspect_bridge::PolarizedHit> {
    use vox_scientia::inspect_bridge::{ClaimPolarity, PolarizedHit};
    bundle
        .normalized_hits
        .iter()
        .map(|h| {
            let note = h.overlap_note.as_deref().unwrap_or("").to_ascii_lowercase();
            let polarity = if note.contains("refute")
                || note.contains("contradic")
                || note.contains("oppos")
                || note.contains("polarity:negative")
            {
                ClaimPolarity::Negative
            } else if note.contains("support")
                || note.contains("confirm")
                || note.contains("polarity:positive")
            {
                ClaimPolarity::Positive
            } else {
                ClaimPolarity::Neutral
            };
            PolarizedHit {
                work_uri: h.work_uri.clone(),
                similarity: h.semantic_score.or(h.lexical_score).unwrap_or(0.0),
                polarity,
                excerpt: h.overlap_note.clone(),
            }
        })
        .collect()
}

/// Conservative cap on [`WorthinessInputs::novelty`] from a live prior-art bundle.
///
/// B5: the typed vox-scientia decision layer
/// ([`AtomicNoveltyScorer`](vox_scientia::inspect_bridge::AtomicNoveltyScorer) /
/// [`ChronoFilter`](vox_scientia::inspect_bridge::ChronoFilter) /
/// [`EvidenceConflictDetector`](vox_scientia::inspect_bridge::EvidenceConflictDetector))
/// is the **gate**; the scalar `novelty_inputs_adjustment` is the **magnitude**.
///
/// Flow: adapt V1 bundle → drop future-dated hits (ChronoFilter, claim = now) →
/// detect opposing-polarity conflict → score → translate the 5-variant
/// [`NoveltyVerdict`](vox_scientia::inspect_bridge::NoveltyVerdict) to a cap.
/// `InsufficientEvidence`/`Contradicted` cap novelty low and explain;
/// `Novel`/`PossiblyNovel`/`NotNovel` keep the existing scalar `min` behavior.
#[must_use]
pub fn apply_prior_art_to_worthiness_inputs(
    inputs: &mut WorthinessInputs,
    bundle: Option<&crate::scientia_finding_ledger::NoveltyEvidenceBundleV1>,
    heuristics: Option<&crate::scientia_heuristics::ScientiaHeuristics>,
) -> Vec<String> {
    use vox_scientia::inspect_bridge::{
        AtomicNoveltyScorer, ChronoFilter, EvidenceConflictDetector, NoveltyVerdict,
    };

    let Some(bundle) = bundle else {
        return vec![];
    };
    // A genuinely empty bundle with no query traces carries no signal at all.
    if bundle.normalized_hits.is_empty() && bundle.query_traces.is_empty() {
        return vec![];
    }

    let mut out: Vec<String> = Vec::new();

    // 1. Adapt to the decision-layer schema.
    let mut adapted = adapt_bundle_v1(bundle);

    // 2. ChronoFilter: drop hits that cannot predate the claim (claim = now).
    let now_secs = chrono::Utc::now().timestamp();
    let chrono = ChronoFilter::new(now_secs);
    let before_hits = adapted.normalized_hits.len();
    let kept: Vec<_> = chrono
        .filter_hits(&adapted.normalized_hits)
        .into_iter()
        .cloned()
        .collect();
    let dropped = before_hits - kept.len();
    if dropped > 0 {
        out.push(format!(
            "novelty_chrono_filtered: dropped {dropped} future-dated hit(s) (claim={now_secs})"
        ));
    }
    adapted.normalized_hits = kept;
    // The pre-computed overlap_summary may reflect now-dropped hits; if every
    // hit was filtered out, clear it so the scorer reads no prior-art signal.
    if adapted.normalized_hits.is_empty() {
        adapted.overlap_summary = None;
    }

    // 3. Conflict detection over polarized (post-chrono) hits.
    let polarized = polarized_hits_from_bundle(&adapted);
    let conflict = EvidenceConflictDetector::default().detect(&adapted.candidate_id, &polarized);

    // 4. Score, then let a detected conflict override to Contradicted.
    let verdict = match conflict {
        Some(c) => {
            let uri = c
                .contradicting_hits
                .first()
                .map(|h| h.work_uri.clone())
                .unwrap_or_default();
            NoveltyVerdict::Contradicted {
                conflicting_uri: uri,
            }
        }
        None => AtomicNoveltyScorer::default().score(&adapted),
    };

    // 5. Translate the verdict to a novelty cap.
    let before = inputs.novelty;
    match verdict {
        NoveltyVerdict::InsufficientEvidence { reason } => {
            inputs.novelty = inputs.novelty.min(NOVELTY_NON_NOVEL_CAP);
            out.push(format!(
                "novelty_gate_insufficient_evidence: {reason}; novelty capped before={before:.4} after={:.4}",
                inputs.novelty
            ));
        }
        NoveltyVerdict::Contradicted { conflicting_uri } => {
            inputs.novelty = inputs.novelty.min(NOVELTY_NON_NOVEL_CAP);
            out.push(format!(
                "novelty_gate_contradicted: conflicting_uri={conflicting_uri}; novelty capped before={before:.4} after={:.4}",
                inputs.novelty
            ));
        }
        NoveltyVerdict::Novel
        | NoveltyVerdict::PossiblyNovel { .. }
        | NoveltyVerdict::NotNovel { .. } => {
            // Verdict permits novelty; scalar heuristic sets the magnitude.
            // (No surviving hits → no prior-art proxy adjustment.)
            if adapted.normalized_hits.is_empty() {
                out.push(format!(
                    "novelty_gate_{}: no surviving prior-art hits; novelty unchanged={before:.4}",
                    verdict_label(&verdict)
                ));
            } else {
                let fallback = crate::scientia_heuristics::ScientiaHeuristics::default();
                let h = heuristics.unwrap_or(&fallback);
                let (proxy, scalar_notes) =
                    crate::scientia_finding_ledger::novelty_inputs_adjustment(bundle, h);
                out.extend(scalar_notes);
                inputs.novelty = before.min(proxy);
                out.push(format!(
                    "novelty_after_prior_art_min: before={before:.4} after={:.4}",
                    inputs.novelty
                ));
            }
        }
    }
    out
}

/// Short stable label for a permissive verdict (for telemetry notes).
fn verdict_label(v: &vox_scientia::inspect_bridge::NoveltyVerdict) -> &'static str {
    use vox_scientia::inspect_bridge::NoveltyVerdict as V;
    match v {
        V::Novel => "novel",
        V::PossiblyNovel { .. } => "possibly_novel",
        V::NotNovel { .. } => "not_novel",
        V::InsufficientEvidence { .. } => "insufficient_evidence",
        V::Contradicted { .. } => "contradicted",
    }
}

/// Advisory venue checks: map `venue_profiles.required_checks` to concrete preflight outcomes (partial).
#[must_use]
pub fn machine_venue_profile_violations(
    contract: &PublicationWorthinessContract,
    profile_id: &str,
    report: &crate::publication_preflight::PreflightReport,
) -> Vec<String> {
    let Some(vp) = contract.venue_profiles.get(profile_id) else {
        return vec![];
    };
    let mut out = Vec::new();
    for check in &vp.required_checks {
        if check.as_str() == "double_blind_anonymization" {
            let bad = report.findings.iter().any(|f| {
                f.code.starts_with("double_blind_")
                    && f.severity == crate::publication_preflight::PreflightSeverity::Error
            });
            if bad {
                out.push("venue_profile:double_blind_anonymization:not_met".to_string());
            }
        }
    }
    out
}

/// Aggregate worthiness score for [`crate::PublisherConfig::worthiness_score`] (per-channel policy floors).
///
/// Matches the orchestrator news service probe: default contract under `repo_root`, `PreflightProfile::Default`.
pub fn worthiness_score_for_publication_manifest(
    manifest: &crate::publication::PublicationManifest,
    repo_root: &Path,
) -> Result<f64> {
    let path = repo_root.join(DEFAULT_CONTRACT_REL_PATH);
    let yaml = vox_bounded_fs::read_utf8_path_capped(&path)
        .with_context(|| format!("read worthiness contract {}", path.display()))?;
    let contract = load_contract_from_str(&yaml)?;
    validate_contract_invariants(&contract)?;
    let preflight = crate::publication_preflight::run_preflight(
        manifest,
        crate::publication_preflight::PreflightProfile::Default,
    );
    let h = crate::scientia_heuristics::ScientiaHeuristics::default();
    let inputs = crate::publication_preflight::worthiness_inputs_from_manifest_and_preflight(
        manifest,
        &preflight,
        Some(&h),
    );
    Ok(evaluate_worthiness(&contract, &inputs).worthiness_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> PublicationWorthinessContract {
        let yaml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/publication-worthiness.default.yaml"
        ));
        let c = load_contract_from_str(yaml).expect("default contract");
        validate_contract_invariants(&c).expect("invariants");
        c
    }

    fn sample_inputs_publish_ready() -> WorthinessInputs {
        WorthinessInputs {
            red_line_violation_ids: vec![],
            repeated_unresolved_contradiction: false,
            claim_evidence_coverage: 0.95,
            artifact_replayability: 0.9,
            artifact_replayability_measured: None,
            before_after_pair_integrity: 0.95,
            metadata_completeness: 0.95,
            ai_disclosure_compliance: 1.0,
            epistemic: 0.9,
            reproducibility: 0.9,
            novelty: 0.88,
            reliability: 0.9,
            metadata_policy: 0.95,
            meaningful_advance: true,
        }
    }

    #[test]
    fn default_contract_loads_and_evaluates_publish() {
        let c = sample_contract();
        let r = evaluate_worthiness(&c, &sample_inputs_publish_ready());
        assert_eq!(r.decision, WorthinessDecision::Publish);
        assert!(r.hard_metrics_ok);
    }

    #[test]
    fn red_line_abstains() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.red_line_violation_ids = vec!["fabricated_citation".to_string()];
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AbstainDoNotPublish);
    }

    #[test]
    fn low_score_abstains() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.epistemic = 0.1;
        i.reproducibility = 0.1;
        i.novelty = 0.1;
        i.reliability = 0.1;
        i.metadata_policy = 0.1;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AbstainDoNotPublish);
    }

    #[test]
    fn missing_metric_floor_asks() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.claim_evidence_coverage = 0.5;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AskForEvidence);
    }

    #[test]
    fn no_meaningful_advance_asks() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.meaningful_advance = false;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AskForEvidence);
    }

    // ── Phase B: effective_replayability + measured-supersedes-declared ──────

    #[test]
    fn effective_replayability_falls_back_to_declared_when_measured_is_none() {
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.42;
        i.artifact_replayability_measured = None;
        assert!((effective_replayability(&i) - 0.42).abs() < 1e-12);
    }

    #[test]
    fn effective_replayability_uses_measured_when_present() {
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.99; // operator-declared (optimistic)
        i.artifact_replayability_measured = Some(0.0); // runner: hash mismatch
        assert_eq!(effective_replayability(&i), 0.0);
    }

    #[test]
    fn measured_failure_overrides_declared_pass_in_hard_metrics() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        // Declared value is well above the contract floor.
        i.artifact_replayability = 0.95;
        // But the runner measured a hash mismatch.
        i.artifact_replayability_measured = Some(0.0);
        let r = evaluate_worthiness(&c, &i);
        assert!(
            !r.hard_metrics_ok,
            "measured 0.0 must override declared 0.95 and fail the hard-gate"
        );
        assert_ne!(r.decision, WorthinessDecision::Publish);
    }

    #[test]
    fn measured_pass_alongside_declared_pass_still_publishes() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.9;
        i.artifact_replayability_measured = Some(1.0);
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::Publish);
        assert!(r.hard_metrics_ok);
    }
}
