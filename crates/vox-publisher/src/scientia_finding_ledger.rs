//! SCIENTIA finding-candidate and novelty-evidence ledger types (`contracts/scientia/*v1.schema.json`).
//!
//! Helpers here depend only on [`crate::scientia_evidence::DiscoverySignal`] (no `scientia_discovery` import) to avoid crate internal cycles.

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::scientia_evidence::{DiscoverySignal, DiscoverySignalFamily};
use crate::scientia_heuristics::ScientiaHeuristics;

/// `candidate_class` in `finding-candidate.v1.schema.json`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FindingCandidateClass {
    AlgorithmicImprovement,
    ReproducibilityInfra,
    PolicyGovernance,
    TelemetryTrust,
    #[default]
    Other,
}

/// Confidence decomposition attached to ledger records and discovery explain output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FindingConfidenceDecomposition {
    #[serde(default)]
    pub signal_strength: f64,
    #[serde(default)]
    pub contradiction_risk: f64,
    #[serde(default)]
    pub reproducibility_support: f64,
}

/// Multi-axis significance (0–1 each); inspectable in preflight / explain JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SignificanceAxes {
    #[serde(default)]
    pub practical_impact: f64,
    #[serde(default)]
    pub delta_magnitude: f64,
    #[serde(default)]
    pub reproducibility_quality: f64,
    #[serde(default)]
    pub transferability: f64,
    #[serde(default)]
    pub harm_if_wrong: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FindingCandidateV1 {
    pub schema_version: i32,
    pub candidate_id: String,
    pub candidate_class: FindingCandidateClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_hint: Option<String>,
    pub internal_signals: Vec<DiscoverySignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub novelty_evidence_bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worthiness_decision_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<FindingConfidenceDecomposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance: Option<SignificanceAxes>,
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<i64>,
}

/// Prior-art source labels (must match `novelty-evidence-bundle.v1.schema.json`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PriorArtSource {
    Openalex,
    Crossref,
    SemanticScholar,
    Manual,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedPriorArtHit {
    pub source: PriorArtSource,
    pub work_uri: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexical_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap_note: Option<String>,
    /// Citation count from upstream when available (OpenAlex `cited_by_count`, S2 `citationCount`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cited_by_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoveltyOverlapSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lexical_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_semantic_score: Option<f64>,
    #[serde(default)]
    pub recency_bucket: NoveltyRecencyBucket,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoveltyRecencyBucket {
    #[default]
    Unknown,
    Stale,
    Recent,
    VeryRecent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoveltyQueryTrace {
    pub source: String,
    pub request_fingerprint_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoveltyEvidenceBundleV1 {
    pub schema_version: i32,
    pub bundle_id: String,
    pub candidate_id: String,
    pub computed_at_ms: i64,
    pub query_digest_sha256: String,
    pub sources: Vec<PriorArtSource>,
    pub normalized_hits: Vec<NormalizedPriorArtHit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap_summary: Option<NoveltyOverlapSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_traces: Vec<NoveltyQueryTrace>,
}

/// Contract SSOT type from `contracts/scientia/novelty-evidence-bundle.v1.schema.json`.
pub type NoveltyEvidenceBundleContract = vox_research_events::schema_types::NoveltyEvidenceBundle;

/// Convert publisher wire type → contract type via validated serde round-trip.
pub fn to_contract_bundle(v1: &NoveltyEvidenceBundleV1) -> NoveltyEvidenceBundleContract {
    let val = serde_json::to_value(v1).expect("NoveltyEvidenceBundleV1 must be serializable");
    serde_json::from_value(val)
        .expect("NoveltyEvidenceBundleV1 must round-trip to NoveltyEvidenceBundle")
}

/// Derive a stable candidate id from publication id (or arbitrary stem).
#[must_use]
pub fn default_candidate_id(publication_id: &str) -> String {
    format!("fc.{publication_id}")
}

/// Heuristic class from discovery signal families.
#[must_use]
pub fn infer_candidate_class(signals: &[DiscoverySignal]) -> FindingCandidateClass {
    let mut saw_telemetry = false;
    let mut saw_trust = false;
    let mut saw_eval = false;
    let mut saw_repro = false;
    let mut saw_docs = false;
    for s in signals {
        match s.family {
            DiscoverySignalFamily::TelemetryAggregate => saw_telemetry = true,
            DiscoverySignalFamily::TrustRollup => saw_trust = true,
            DiscoverySignalFamily::EvalGate
            | DiscoverySignalFamily::BenchmarkPair
            | DiscoverySignalFamily::MensScorecard => saw_eval = true,
            DiscoverySignalFamily::ReproducibilityArtifact => saw_repro = true,
            DiscoverySignalFamily::Documentation | DiscoverySignalFamily::LinkedCorpus => {
                saw_docs = true;
            }
            DiscoverySignalFamily::OperatorAttestation
            | DiscoverySignalFamily::Unspecified
            | DiscoverySignalFamily::FindingCandidateSignal => {}
        }
    }
    if saw_telemetry || saw_trust {
        return FindingCandidateClass::TelemetryTrust;
    }
    if saw_docs && !saw_eval {
        return FindingCandidateClass::PolicyGovernance;
    }
    if saw_repro && !saw_eval {
        return FindingCandidateClass::ReproducibilityInfra;
    }
    if saw_eval {
        return FindingCandidateClass::AlgorithmicImprovement;
    }
    FindingCandidateClass::Other
}

/// Build confidence decomposition from ranked signal counts.
#[must_use]
pub fn confidence_from_signal_counts(
    strong_n: u32,
    sup_n: u32,
    info_n: u32,
    had_conflicts: bool,
    h: &ScientiaHeuristics,
) -> FindingConfidenceDecomposition {
    let n = (strong_n + sup_n + info_n).max(1);
    let weighted = (strong_n as f64 * h.confidence_weight_strong
        + sup_n as f64 * h.confidence_weight_supporting
        + info_n as f64 * h.confidence_weight_informational)
        / f64::from(n);
    FindingConfidenceDecomposition {
        signal_strength: weighted.clamp(0.0, 1.0),
        contradiction_risk: if had_conflicts {
            h.confidence_contradiction_high
        } else {
            h.confidence_contradiction_low
        },
        reproducibility_support: if sup_n > 0 || strong_n > 0 {
            (h.confidence_repro_sup_strong + 0.2 * f64::from(strong_n)).clamp(0.0, 1.0)
        } else {
            h.confidence_repro_sup_only
        },
    }
}

/// Heuristic significance from rank heuristics (deterministic, no LLM).
#[must_use]
pub fn significance_from_heuristics(
    rank_score: u32,
    strong_n: u32,
    sup_n: u32,
    intake_is_low_signal: bool,
    title_hint: Option<&str>,
    h: &ScientiaHeuristics,
) -> SignificanceAxes {
    let base = (f64::from(rank_score) / h.significance_rank_divisor).clamp(0.0, 1.0);
    let title_boost = title_hint
        .map(|t| {
            (t.len() as f64 / h.significance_title_len_divisor)
                .clamp(0.0, h.significance_title_boost_max)
        })
        .unwrap_or(0.0);
    SignificanceAxes {
        practical_impact: (base * 0.7 + title_boost).clamp(0.0, 1.0),
        delta_magnitude: if strong_n >= 2 {
            0.75
        } else if strong_n == 1 {
            0.55
        } else {
            0.35
        },
        reproducibility_quality: if strong_n + sup_n > 0 { 0.6 } else { 0.35 },
        transferability: (base * 0.5 + 0.15).clamp(0.0, 1.0),
        harm_if_wrong: if intake_is_low_signal { 0.25 } else { 0.45 },
    }
}

/// Build a v1 finding-candidate row from discovery context.
#[must_use]
pub fn build_finding_candidate(
    publication_id: Option<&str>,
    title_hint: Option<String>,
    signals: Vec<DiscoverySignal>,
    publication_row_id: &str,
    strong_n: u32,
    sup_n: u32,
    info_n: u32,
    rank_score: u32,
    intake_is_low_signal: bool,
    had_conflicts: bool,
    now_ms: i64,
    heuristics: &ScientiaHeuristics,
) -> FindingCandidateV1 {
    let candidate_id = publication_id
        .map(default_candidate_id)
        .unwrap_or_else(|| format!("fc.{publication_row_id}"));
    let class = infer_candidate_class(&signals);
    FindingCandidateV1 {
        schema_version: 1,
        candidate_id,
        candidate_class: class,
        publication_id: publication_id.map(std::string::ToString::to_string),
        title_hint: title_hint.clone(),
        internal_signals: signals,
        novelty_evidence_bundle_id: None,
        worthiness_decision_ref: None,
        confidence: Some(confidence_from_signal_counts(
            strong_n,
            sup_n,
            info_n,
            had_conflicts,
            heuristics,
        )),
        significance: Some(significance_from_heuristics(
            rank_score,
            strong_n,
            sup_n,
            intake_is_low_signal,
            title_hint.as_deref(),
            heuristics,
        )),
        created_at_ms: now_ms,
        updated_at_ms: None,
    }
}

/// Max lexical / semantic overlap scores from a novelty bundle (each in \[0,1\]).
#[must_use]
pub fn bundle_max_overlap_scores(bundle: &NoveltyEvidenceBundleV1) -> (f64, f64) {
    let max_lex = bundle
        .overlap_summary
        .as_ref()
        .and_then(|o| o.max_lexical_score)
        .or_else(|| {
            bundle
                .normalized_hits
                .iter()
                .filter_map(|h: &NormalizedPriorArtHit| h.lexical_score)
                .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        })
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let max_sem = bundle
        .overlap_summary
        .as_ref()
        .and_then(|o| o.max_semantic_score)
        .or_else(|| {
            bundle
                .normalized_hits
                .iter()
                .filter_map(|h: &NormalizedPriorArtHit| h.semantic_score)
                .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        })
        .unwrap_or(max_lex)
        .clamp(0.0, 1.0);
    (max_lex, max_sem)
}

/// Blended prior-art overlap in \[0,1\] (used by discovery rank + worthiness novelty proxy).
#[must_use]
pub fn novelty_overlap_blend_01(bundle: &NoveltyEvidenceBundleV1, h: &ScientiaHeuristics) -> f64 {
    let (max_lex, max_sem) = bundle_max_overlap_scores(bundle);
    let w_lex = h.novelty_blend_lexical.clamp(0.0, 1.0);
    let w_sem = h.novelty_blend_semantic.clamp(0.0, 1.0);
    (max_lex * w_lex + max_sem * w_sem).clamp(0.0, 1.0)
}

/// Merge overlap summary into novelty proxy for worthiness; returns `(novelty_proxy, notes)`.
#[must_use]
pub fn novelty_inputs_adjustment(
    bundle: &NoveltyEvidenceBundleV1,
    h: &ScientiaHeuristics,
) -> (f64, Vec<String>) {
    let mut reasons = Vec::new();
    let (max_lex, max_sem) = bundle_max_overlap_scores(bundle);
    let overlap = novelty_overlap_blend_01(bundle, h);
    let novelty_proxy = (1.0 - overlap).clamp(0.0, 1.0);
    if overlap >= h.novelty_high_threshold {
        reasons.push(format!(
            "machine_prior_art_overlap_high: max_lex={max_lex:.3} max_sem={max_sem:.3}"
        ));
    } else if overlap >= h.novelty_moderate_threshold {
        reasons.push(format!(
            "machine_prior_art_overlap_moderate: max_lex={max_lex:.3} max_sem={max_sem:.3}"
        ));
    }
    (novelty_proxy, reasons)
}

/// Assist-only readership / citation projection from comparable prior-art hits (not a publish gate).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImpactReadershipProjectionV1 {
    pub schema_version: u32,
    pub assist_only: bool,
    pub not_publish_gate: bool,
    pub comparable_works_analyzed: usize,
    pub works_with_citation_data: usize,
    pub max_cited_by_count: Option<u64>,
    pub mean_citations_per_year: Option<f64>,
    /// Coarse bucket vs max hit: `below_field_proxy`, `near_field_proxy`, `above_field_proxy`, or `unknown`.
    pub field_impact_bucket: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_notes: Vec<String>,
}

/// Build projection from bundle hits; uses [`ScientiaHeuristics`] percentile thresholds as field proxy.
#[must_use]
pub fn impact_readership_projection_v1(
    bundle: &NoveltyEvidenceBundleV1,
    h: &ScientiaHeuristics,
) -> ImpactReadershipProjectionV1 {
    let mut coverage = Vec::new();
    coverage.push(
        "assist-only: prioritization signal; do not treat as peer-review outcome".to_string(),
    );
    let mut counts = Vec::new();
    let mut velocities = Vec::new();
    let cy = chrono::Utc::now().date_naive().year();
    for hit in &bundle.normalized_hits {
        let Some(c) = hit.cited_by_count else {
            continue;
        };
        counts.push(c);
        if let Some(y) = hit.year {
            let age = (cy - y).max(1) as f64;
            velocities.push(c as f64 / age);
        }
    }
    if counts.is_empty() {
        coverage
            .push("no cited_by_count on normalized hits (source APIs omitted or offline)".into());
    }
    let max_c = counts.iter().copied().max();
    let mean_v = if velocities.is_empty() {
        None
    } else {
        Some(velocities.iter().sum::<f64>() / velocities.len() as f64)
    };
    // Without a field-normalized corpus, treat seed percentiles as coarse citation cutoffs (see research SSOT).
    let below_cut = u64::from(h.field_bucket_below_upper);
    let near_cut = u64::from(h.field_bucket_near_upper.max(h.field_bucket_below_upper));
    let bucket = match max_c {
        None => "unknown".to_string(),
        Some(m) if m < below_cut => "below_field_proxy".to_string(),
        Some(m) if m < near_cut => "near_field_proxy".to_string(),
        Some(_) => "above_field_proxy".to_string(),
    };

    ImpactReadershipProjectionV1 {
        schema_version: 1,
        assist_only: true,
        not_publish_gate: true,
        comparable_works_analyzed: bundle.normalized_hits.len(),
        works_with_citation_data: counts.len(),
        max_cited_by_count: max_c,
        mean_citations_per_year: mean_v,
        field_impact_bucket: bucket,
        coverage_notes: coverage,
    }
}

/// Optional envelope for calibration / telemetry (`contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json`).
///
/// Emitted inside `publication-novelty-happy-path` stdout JSON as `calibration_telemetry`. Human operators
/// may later set the `_*_reported` fields when reconciling model drift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScientiaNoveltyDecisionCalibrationV1 {
    pub schema_version: u32,
    pub publication_id: String,
    pub candidate_id: String,
    pub bundle_id: String,
    pub decision_latency_ms: u64,
    pub offline_prior_art_fetch: bool,
    pub prior_art_hit_count: usize,
    pub max_lexical_overlap: f64,
    pub worthiness_decision: String,
    pub worthiness_score: f64,
    pub hard_metrics_ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_novelty_alarm_reported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missed_prior_art_reported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_disagreement_reported: Option<bool>,
}

/// Build a v1 calibration row from a happy-path run (deterministic fields only unless caller overrides).
#[must_use]
pub fn novelty_decision_calibration_v1(
    publication_id: &str,
    candidate_id: &str,
    bundle: &NoveltyEvidenceBundleV1,
    decision_latency_ms: u64,
    offline_prior_art_fetch: bool,
    worthiness_decision: &str,
    worthiness_score: f64,
    hard_metrics_ok: bool,
    prior_art_max_lexical_overlap: Option<f64>,
) -> ScientiaNoveltyDecisionCalibrationV1 {
    let from_bundle = bundle
        .overlap_summary
        .as_ref()
        .and_then(|o| o.max_lexical_score)
        .unwrap_or(0.0);
    let max_lexical_overlap = prior_art_max_lexical_overlap
        .unwrap_or(from_bundle)
        .clamp(0.0, 1.0);
    ScientiaNoveltyDecisionCalibrationV1 {
        schema_version: 1,
        publication_id: publication_id.to_string(),
        candidate_id: candidate_id.to_string(),
        bundle_id: bundle.bundle_id.clone(),
        decision_latency_ms,
        offline_prior_art_fetch,
        prior_art_hit_count: bundle.normalized_hits.len(),
        max_lexical_overlap,
        worthiness_decision: worthiness_decision.to_string(),
        worthiness_score,
        hard_metrics_ok,
        false_novelty_alarm_reported: None,
        missed_prior_art_reported: None,
        reviewer_disagreement_reported: None,
    }
}
