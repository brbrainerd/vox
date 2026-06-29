//! Deterministic publication discovery ranking, completion reporting, and destination previews.
//!
//! All scoring is structural — no LLM judgment. See `scientia_evidence` for signal catalog + conflicts.

use serde::{Deserialize, Serialize};

use crate::publication::PublicationManifest;
use crate::scientia_evidence::{
    self, DiscoverySignalStrength, ScientiaEvidenceContext, SignalConflict,
};
use crate::scientific_metadata::{METADATA_KEY_SCIENTIFIC, ScientificPublicationMetadata};

/// Optional gate for `publication-prepare` / MCP prepare (scientia only).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryIntakeGate {
    /// No intake gate (default).
    #[default]
    None,
    /// Require [`DiscoveryCandidateRank::auto_draft_eligible`] (≥1 strong signal, no structured conflicts).
    StrongSignalsOnly,
    /// Allow [`DiscoveryIntakeTier::StrongCandidate`] or [`DiscoveryIntakeTier::ReviewSuggested`]; block [`DiscoveryIntakeTier::LowSignal`].
    AllowReviewSuggested,
}

/// Whether `rank` satisfies `gate` for manifest intake.
#[must_use]
pub fn intake_gate_allows(gate: DiscoveryIntakeGate, rank: &DiscoveryCandidateRank) -> bool {
    match gate {
        DiscoveryIntakeGate::None => true,
        DiscoveryIntakeGate::StrongSignalsOnly => rank.auto_draft_eligible,
        DiscoveryIntakeGate::AllowReviewSuggested => {
            rank.intake_tier != DiscoveryIntakeTier::LowSignal
        }
    }
}

/// Intake tier for operator UX (`scan` / `explain`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryIntakeTier {
    /// At least one [`DiscoverySignalStrength::Strong`] signal, no unresolved conflicts.
    StrongCandidate,
    /// Supporting or informational only, or conflicts need manual reconciliation.
    ReviewSuggested,
    LowSignal,
}

/// Rank output for one manifest row (DB or in-memory).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveryCandidateRank {
    pub publication_id: String,
    pub rank_score: u32,
    pub intake_tier: DiscoveryIntakeTier,
    pub strong_signal_count: u32,
    pub supporting_signal_count: u32,
    pub informational_signal_count: u32,
    pub auto_draft_eligible: bool,
    pub machine_explanation: Vec<String>,
    pub signal_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<SignalConflict>,
    /// Finding-candidate class (ledger / explain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_class: Option<crate::scientia_finding_ledger::FindingCandidateClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_decomposition:
        Option<crate::scientia_finding_ledger::FindingConfidenceDecomposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance_axes: Option<crate::scientia_finding_ledger::SignificanceAxes>,
    /// Max prior-art lexical overlap when a novelty bundle is attached (0–1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_art_max_lexical_overlap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_art_max_semantic_overlap: Option<f64>,
    /// Blended prior-art overlap in \[0,1\] reflected in [`DiscoveryCandidateRank::rank_score`].
    #[serde(default)]
    pub novelty_overlap_effective: f32,
    /// When true, [`Self::novelty_overlap_effective`] defaulted to **0.5** (moderate overlap) because no scan was supplied.
    #[serde(default)]
    pub novelty_overlap_assumed_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldProvenanceEntry {
    pub field: String,
    pub origin: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestCompletionReport {
    pub completeness_0_100: u8,
    #[serde(default)]
    pub required_missing: Vec<String>,
    #[serde(default)]
    pub inferred_ok: Vec<String>,
    #[serde(default)]
    pub human_only_pending: Vec<String>,
    #[serde(default)]
    pub field_provenance: Vec<FieldProvenanceEntry>,
}

fn strength_weight(
    s: DiscoverySignalStrength,
    h: &crate::scientia_heuristics::ScientiaHeuristics,
) -> u32 {
    match s {
        DiscoverySignalStrength::Strong => h.rank_weight_strong,
        DiscoverySignalStrength::Supporting => h.rank_weight_supporting,
        DiscoverySignalStrength::Informational => h.rank_weight_informational,
    }
}

/// Evidence graph completeness (0–100), separate from publication preflight.
#[must_use]
pub fn evidence_completeness_score(
    evidence: &ScientiaEvidenceContext,
    h: &crate::scientia_heuristics::ScientiaHeuristics,
) -> u8 {
    let mut pts: u32 = 0;
    let max = h.evidence_completeness_max.max(1);
    if evidence.socrates_aggregate.is_some() {
        pts += 1;
    }
    if evidence.eval_gate.is_some() {
        pts += 1;
    }
    if evidence.benchmark.is_some() {
        pts += 1;
    }
    if !evidence.discovery_signals.is_empty() {
        pts += 1;
    }
    if evidence.draft_preparation.is_some() {
        pts += 1;
    }
    if evidence
        .eval_gate_report_repo_relative
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        pts += 1;
    }
    if evidence
        .benchmark_pair_report_repo_relative
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        pts += 1;
    }
    if evidence.human_meaningful_advance {
        pts += 1;
    }
    if !evidence.linked_doc_repo_relatives.is_empty()
        || evidence
            .mens_scorecard_repo_relative
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
    {
        pts += 1;
    }
    if !evidence.doc_section_hints.is_empty() {
        pts += 1;
    }
    if !evidence.autofill_provenance.is_empty() {
        pts += 1;
    }
    u8::try_from((pts.saturating_mul(100)) / max).unwrap_or(100)
}

/// Deterministic rank from evidence block + optional `source_ref` (markdown path).
///
/// Pass `novelty_overlap` only when a prior-art scan produced a blended overlap in \[0,1\].
/// Pass `None` when no scan has run; a **0.5** overlap default is applied (not treated as perfect novelty).
#[must_use]
pub fn rank_candidate(
    publication_id: &str,
    source_ref: Option<&str>,
    evidence: &ScientiaEvidenceContext,
    novelty_overlap: Option<f32>,
) -> DiscoveryCandidateRank {
    let default = crate::scientia_heuristics::ScientiaHeuristics::default();
    rank_candidate_heuristics(
        publication_id,
        source_ref,
        evidence,
        &default,
        novelty_overlap,
    )
}

/// Rank with explicit dynamics seed heuristics.
#[must_use]
pub fn rank_candidate_heuristics(
    publication_id: &str,
    source_ref: Option<&str>,
    evidence: &ScientiaEvidenceContext,
    h: &crate::scientia_heuristics::ScientiaHeuristics,
    novelty_overlap: Option<f32>,
) -> DiscoveryCandidateRank {
    let signals = if evidence.discovery_signals.is_empty() {
        scientia_evidence::infer_discovery_signals(source_ref, evidence)
    } else {
        evidence.discovery_signals.clone()
    };
    let conflicts = if evidence.signal_conflicts.is_empty() {
        scientia_evidence::detect_signal_conflicts(evidence)
    } else {
        evidence.signal_conflicts.clone()
    };

    let mut strong_n = 0_u32;
    let mut sup_n = 0_u32;
    let mut info_n = 0_u32;
    let mut score = 0_u32;
    let mut codes = Vec::new();
    for s in &signals {
        codes.push(s.code.clone());
        match s.strength {
            DiscoverySignalStrength::Strong => {
                strong_n += 1;
                score += strength_weight(s.strength, h);
            }
            DiscoverySignalStrength::Supporting => {
                sup_n += 1;
                score += strength_weight(s.strength, h);
            }
            DiscoverySignalStrength::Informational => {
                info_n += 1;
                score += strength_weight(s.strength, h);
            }
        }
    }
    if strong_n >= 2 {
        score = score.saturating_add(h.rank_bonus_strong_pair);
    }

    let novelty_overlap_assumed_default = novelty_overlap.is_none();
    let novelty_overlap_effective = novelty_overlap.unwrap_or(0.5_f32).clamp(0.0, 1.0);
    let overlap_penalty = ((f64::from(novelty_overlap_effective))
        * f64::from(h.rank_novelty_overlap_penalty_max))
    .round() as u32;
    score = score.saturating_sub(overlap_penalty);

    let unresolved_conflicts = !conflicts.is_empty();
    let tier = if strong_n > 0 && !unresolved_conflicts {
        DiscoveryIntakeTier::StrongCandidate
    } else if strong_n > 0 && unresolved_conflicts || sup_n > 0 || info_n > 0 {
        DiscoveryIntakeTier::ReviewSuggested
    } else {
        DiscoveryIntakeTier::LowSignal
    };

    let auto_draft_eligible = strong_n >= 1 && !unresolved_conflicts;
    let mut explanation = Vec::new();
    explanation.push(format!(
        "signals_total={} strong={} supporting={} informational={}",
        signals.len(),
        strong_n,
        sup_n,
        info_n
    ));
    if auto_draft_eligible {
        explanation.push(
            "Policy: at least one strong signal and no structured conflicts — eligible for automated draft intake."
                .into(),
        );
    } else if strong_n > 0 && unresolved_conflicts {
        explanation.push(
            "Strong signals present but structured conflicts require human reconciliation before auto-draft.".into(),
        );
    } else if strong_n == 0 {
        explanation.push(
            "No strong discovery signals — surface as review-suggested or low-signal only.".into(),
        );
    }
    for c in &conflicts {
        explanation.push(c.summary.clone());
    }

    let intake_low = tier == DiscoveryIntakeTier::LowSignal;
    let candidate_class = crate::scientia_finding_ledger::infer_candidate_class(&signals);
    let confidence_decomposition = Some(
        crate::scientia_finding_ledger::confidence_from_signal_counts(
            strong_n,
            sup_n,
            info_n,
            unresolved_conflicts,
            h,
        ),
    );
    let significance_axes = Some(
        crate::scientia_finding_ledger::significance_from_heuristics(
            score, strong_n, sup_n, intake_low, None, h,
        ),
    );

    DiscoveryCandidateRank {
        publication_id: publication_id.to_string(),
        rank_score: score,
        intake_tier: tier,
        strong_signal_count: strong_n,
        supporting_signal_count: sup_n,
        informational_signal_count: info_n,
        auto_draft_eligible,
        machine_explanation: explanation,
        signal_codes: codes,
        conflicts,
        candidate_class: Some(candidate_class),
        confidence_decomposition,
        significance_axes,
        prior_art_max_lexical_overlap: None,
        prior_art_max_semantic_overlap: None,
        novelty_overlap_effective,
        novelty_overlap_assumed_default,
    }
}

/// Fill prior-art overlap fields on `rank` from an embedded novelty bundle.
pub fn merge_novelty_overlap_into_rank(
    rank: &mut DiscoveryCandidateRank,
    bundle: &crate::scientia_finding_ledger::NoveltyEvidenceBundleV1,
    h: &crate::scientia_heuristics::ScientiaHeuristics,
) {
    rank.novelty_overlap_effective =
        crate::scientia_finding_ledger::novelty_overlap_blend_01(bundle, h) as f32;
    rank.novelty_overlap_assumed_default = false;
    if let Some(s) = &bundle.overlap_summary {
        rank.prior_art_max_lexical_overlap = s.max_lexical_score;
        rank.prior_art_max_semantic_overlap = s.max_semantic_score;
        return;
    }
    if bundle.normalized_hits.is_empty() {
        return;
    }
    let max_lex = bundle
        .normalized_hits
        .iter()
        .filter_map(|h| h.lexical_score)
        .fold(0.0_f64, |a: f64, b: f64| a.max(b));
    let max_sem = bundle
        .normalized_hits
        .iter()
        .filter_map(|h| h.semantic_score)
        .fold(0.0_f64, |a: f64, b: f64| a.max(b));
    rank.prior_art_max_lexical_overlap = Some(max_lex);
    rank.prior_art_max_semantic_overlap = Some(max_sem);
}

fn parse_scientific(meta: Option<&str>) -> Option<ScientificPublicationMetadata> {
    let raw = meta?;
    let root: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let block = root.get(METADATA_KEY_SCIENTIFIC)?;
    serde_json::from_value(block.clone()).ok()
}

fn parse_metadata_root(meta: Option<&str>) -> Option<serde_json::Value> {
    meta.filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str(s.trim()).ok())
}

fn scientia_block(meta: Option<&str>) -> Option<serde_json::Map<String, serde_json::Value>> {
    parse_metadata_root(meta).and_then(|v| v.get("scientia").and_then(|b| b.as_object().cloned()))
}

/// Zenodo archive fields tracked in [`ManifestCompletionReport::required_missing`].
/// Autofill (via [`crate::scientia_autofill`]) fills holes and records
/// [`FieldProvenanceEntry`] rows under `metadata_json.scientia.field_provenance`.
#[must_use]
pub fn zenodo_archive_required_missing(manifest: &PublicationManifest) -> Vec<String> {
    let scientia = scientia_block(manifest.metadata_json.as_deref());
    let scientific = parse_scientific(manifest.metadata_json.as_deref());
    let mut missing = Vec::new();

    let has_pub_date = scientia
        .as_ref()
        .and_then(|b| b.get("publication_date"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if !has_pub_date {
        missing.push("publication_date".into());
    }

    let has_keywords = scientia
        .as_ref()
        .and_then(|b| b.get("keywords"))
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    if !has_keywords {
        missing.push("keywords".into());
    }

    let has_version = scientia
        .as_ref()
        .and_then(|b| b.get("version"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if !has_version {
        missing.push("version".into());
    }

    let stored_related = scientia
        .as_ref()
        .and_then(|b| b.get("related_identifiers"))
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    let derived_related = scientific
        .as_ref()
        .and_then(|s| s.reproducibility.as_ref())
        .map(|r| {
            r.code_repository_url
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty())
                || r.data_repository_url
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty())
        })
        .unwrap_or(false)
        || scientia
            .as_ref()
            .and_then(|b| b.get("nanopub_uris"))
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
        || scientia
            .as_ref()
            .and_then(|b| b.get("swhid"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .is_some_and(|s| !s.is_empty());
    if !stored_related && !derived_related {
        missing.push("related_identifiers".into());
    }

    missing
}

/// Merge persisted autofill provenance from `metadata_json.scientia.field_provenance`
/// into a completion report (does not recompute missing fields).
#[must_use]
pub fn merge_field_provenance_from_metadata(
    mut report: ManifestCompletionReport,
    manifest: &PublicationManifest,
) -> ManifestCompletionReport {
    let Some(scientia) = scientia_block(manifest.metadata_json.as_deref()) else {
        return report;
    };
    let Some(arr) = scientia.get("field_provenance").and_then(|v| v.as_array()) else {
        return report;
    };
    for entry in arr {
        if let Ok(prov) = serde_json::from_value::<FieldProvenanceEntry>(entry.clone()) {
            if !report
                .field_provenance
                .iter()
                .any(|p| p.field == prov.field && p.origin == prov.origin)
            {
                report.field_provenance.push(prov);
            }
        }
    }
    report
}

/// Field completion + provenance hints (clerical; does not assert novelty).
#[must_use]
pub fn manifest_completion_report(manifest: &PublicationManifest) -> ManifestCompletionReport {
    let mut required_missing = Vec::new();
    let mut inferred_ok = Vec::new();
    let mut human_only_pending = Vec::new();
    let mut provenance = Vec::new();

    if manifest.title.trim().is_empty() || manifest.title == "Untitled" {
        required_missing.push("title".into());
    } else {
        inferred_ok.push("title".into());
        provenance.push(FieldProvenanceEntry {
            field: "title".into(),
            origin: "manifest".into(),
            notes: None,
        });
    }
    if manifest.author.trim().is_empty() {
        required_missing.push("author".into());
    } else {
        provenance.push(FieldProvenanceEntry {
            field: "author".into(),
            origin: "operator".into(),
            notes: Some("Must be human-attested for scholarly submissions.".into()),
        });
    }
    if manifest
        .abstract_text
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        required_missing.push("abstract_text".into());
        human_only_pending.push("abstract_final".into());
    }
    if manifest
        .citations_json
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        required_missing.push("citations_json".into());
    } else {
        inferred_ok.push("citations_json".into());
    }

    let scientific = parse_scientific(manifest.metadata_json.as_deref());
    if scientific.is_none() {
        human_only_pending.push("scientific_publication_metadata".into());
    } else {
        inferred_ok.push("scientific_publication".into());
        provenance.push(FieldProvenanceEntry {
            field: "scientific_publication".into(),
            origin: "metadata_json".into(),
            notes: None,
        });
    }

    let evidence = scientia_evidence::parse_scientia_evidence(manifest.metadata_json.as_deref());
    let h_mc = crate::scientia_heuristics::ScientiaHeuristics::default();
    let ev_score = evidence
        .as_ref()
        .map(|e| evidence_completeness_score(e, &h_mc))
        .unwrap_or(0);
    if evidence.is_some() {
        inferred_ok.push("scientia_evidence".into());
        provenance.push(FieldProvenanceEntry {
            field: "scientia_evidence".into(),
            origin: "metadata_json".into(),
            notes: None,
        });
    } else {
        human_only_pending.push("scientia_evidence_optional".into());
    }

    for field in zenodo_archive_required_missing(manifest) {
        if !required_missing.contains(&field) {
            required_missing.push(field);
        }
    }

    let manifest_score = 100_i32 - (required_missing.len() as i32 * 18).min(90);
    let manifest_score = manifest_score.clamp(10, 100) as u16;
    let completeness = ((manifest_score + u16::from(ev_score)) / 2).min(100) as u8;

    merge_field_provenance_from_metadata(
        ManifestCompletionReport {
            completeness_0_100: completeness,
            required_missing,
            inferred_ok,
            human_only_pending,
            field_provenance: provenance,
        },
        manifest,
    )
}

/// Loss map + preview payloads for scholarly/social destinations (non-authoritative).
#[must_use]
pub fn destination_transform_previews(
    manifest: &PublicationManifest,
    evidence: Option<&ScientiaEvidenceContext>,
) -> serde_json::Value {
    let abstract_stub = manifest
        .abstract_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("[abstract pending — human]");
    let loss_map = vec![
        serde_json::json!({
            "field": "novelty_claims",
            "scholarly": "not_auto_translated",
            "social": "requires_human_framing",
        }),
        serde_json::json!({
            "field": "venue_specific_ethics",
            "scholarly": "partial_from_scientific_publication",
            "social": "manual_disclosure_tone",
        }),
    ];
    let signal_codes = evidence
        .map(|e| {
            if e.discovery_signals.is_empty() {
                scientia_evidence::infer_discovery_signals(manifest.source_ref.as_deref(), e)
            } else {
                e.discovery_signals.clone()
            }
        })
        .unwrap_or_default();
    let codes: Vec<&str> = signal_codes.iter().map(|s| s.code.as_str()).collect();
    serde_json::json!({
        "schema_version": 1,
        "labels": {
            scientia_evidence::SCIENTIA_LABEL_MACHINE_SUGGESTED: true,
            scientia_evidence::SCIENTIA_LABEL_REQUIRES_HUMAN_REVIEW: true,
            scientia_evidence::SCIENTIA_LABEL_SOURCE_GROUNDED: manifest.source_ref.is_some(),
        },
        "arxiv_assist": {
            "title": manifest.title,
            "abstract_stub": abstract_stub,
            "handoff_note": "Use `publication-scholarly-staging-export` for main.tex + arxiv_handoff.json; operator uploads.",
        },
        "zenodo": {
            "title": manifest.title,
            "description_stub": abstract_stub,
        },
        "openreview": {
            "title": manifest.title,
            "abstract_stub": abstract_stub,
        },
        "social": {
            "short_post": format!("{} — {}", manifest.title, abstract_stub),
            "hn_assist": format!("Show HN: {} ({})", manifest.title, abstract_stub),
        },
        "discovery_signal_codes": codes,
        "loss_map": loss_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scientia_evidence::{
        BenchmarkPairSnapshot, EvalGateSnapshot, ScientiaEvidenceContext,
    };

    #[test]
    fn strong_signal_ranks_higher_without_conflicts() {
        let ev = ScientiaEvidenceContext {
            eval_gate: Some(EvalGateSnapshot {
                passed: true,
                gates_failed: 0,
                gates_total: 3,
            }),
            ..Default::default()
        };
        let r = rank_candidate("p1", None, &ev, Some(0.0));
        assert!(r.rank_score >= 10);
        assert_eq!(r.intake_tier, DiscoveryIntakeTier::StrongCandidate);
        assert!(r.auto_draft_eligible);
    }

    #[test]
    fn advance_with_failed_gate_not_auto_draft() {
        let ev = ScientiaEvidenceContext {
            eval_gate: Some(EvalGateSnapshot {
                passed: false,
                gates_failed: 2,
                gates_total: 3,
            }),
            human_meaningful_advance: true,
            ..Default::default()
        };
        let r = rank_candidate("p2", None, &ev, None);
        assert!(!r.auto_draft_eligible);
        assert!(!r.conflicts.is_empty());
    }

    #[test]
    fn incomplete_benchmark_conflicts_advance() {
        let ev = ScientiaEvidenceContext {
            benchmark: Some(BenchmarkPairSnapshot {
                pair_complete: false,
                ..Default::default()
            }),
            human_meaningful_advance: true,
            ..Default::default()
        };
        let r = rank_candidate("p3", None, &ev, None);
        assert!(!r.conflicts.is_empty());
    }

    #[test]
    fn intake_gate_strong_signals_only_respects_auto_draft() {
        let ev = ScientiaEvidenceContext {
            eval_gate: Some(EvalGateSnapshot {
                passed: true,
                gates_failed: 0,
                gates_total: 1,
            }),
            ..Default::default()
        };
        let r = rank_candidate("g1", None, &ev, None);
        assert!(intake_gate_allows(
            DiscoveryIntakeGate::StrongSignalsOnly,
            &r
        ));
        let low = rank_candidate("g2", None, &ScientiaEvidenceContext::default(), None);
        assert!(!intake_gate_allows(
            DiscoveryIntakeGate::StrongSignalsOnly,
            &low
        ));
        assert!(intake_gate_allows(DiscoveryIntakeGate::None, &low));
        assert!(!intake_gate_allows(
            DiscoveryIntakeGate::AllowReviewSuggested,
            &low
        ));
    }

    #[test]
    fn g1_unscanned_novelty_overlap_assumes_moderate_and_penalizes_rank() {
        let ev = ScientiaEvidenceContext {
            eval_gate: Some(EvalGateSnapshot {
                passed: true,
                gates_failed: 0,
                gates_total: 3,
            }),
            ..Default::default()
        };
        let h = crate::scientia_heuristics::ScientiaHeuristics::default();
        let with_none = rank_candidate_heuristics("x", None, &ev, &h, None);
        let with_zero = rank_candidate_heuristics("x", None, &ev, &h, Some(0.0));
        assert!(with_none.novelty_overlap_assumed_default);
        assert!(!with_zero.novelty_overlap_assumed_default);
        assert!(
            with_none.rank_score < with_zero.rank_score,
            "expected None (assumed 0.5 overlap) to score strictly below confirmed 0.0 overlap; none={} zero={}",
            with_none.rank_score,
            with_zero.rank_score
        );
    }

    #[test]
    fn zenodo_archive_fields_surface_in_required_missing() {
        let m = PublicationManifest {
            publication_id: "p".into(),
            content_type: "scientia".into(),
            source_ref: None,
            title: "Neural Networks and Deep Learning".into(),
            author: "Ada".into(),
            abstract_text: Some("Abs".into()),
            body_markdown: "b".into(),
            citations_json: Some("[]".into()),
            metadata_json: None,
        };
        let report = manifest_completion_report(&m);
        for field in [
            "publication_date",
            "keywords",
            "version",
            "related_identifiers",
        ] {
            assert!(
                report.required_missing.contains(&field.to_string()),
                "expected {field} in required_missing: {:?}",
                report.required_missing
            );
        }
    }

    #[test]
    fn autofill_clears_zenodo_required_missing_with_provenance() {
        let mut manifest = PublicationManifest {
            publication_id: "p".into(),
            content_type: "scientia".into(),
            source_ref: None,
            title: "Neural Networks and Deep Learning".into(),
            author: "Ada".into(),
            abstract_text: Some("Abs".into()),
            body_markdown: "b".into(),
            citations_json: Some("[]".into()),
            metadata_json: None,
        };
        let before = manifest_completion_report(&manifest);
        assert!(
            before
                .required_missing
                .contains(&"publication_date".to_string())
        );

        let plan = crate::scientia_autofill::compute_autofill(
            &manifest,
            None,
            Some("MIT"),
            Some("https://github.com/org/repo"),
            Some(env!("CARGO_PKG_VERSION")),
        );
        let new_meta = crate::scientia_autofill::apply_autofill(
            manifest.metadata_json.as_deref(),
            &mut manifest.abstract_text,
            &plan,
        )
        .expect("apply autofill");
        manifest.metadata_json = Some(new_meta);

        let after = manifest_completion_report(&manifest);
        assert!(
            !after
                .required_missing
                .contains(&"publication_date".to_string()),
            "publication_date should be filled: {:?}",
            after.required_missing
        );
        assert!(
            !after.required_missing.contains(&"keywords".to_string()),
            "keywords should be filled: {:?}",
            after.required_missing
        );
        assert!(
            after
                .field_provenance
                .iter()
                .any(|p| p.field == "publication_date" && p.origin.starts_with("autofill:")),
            "expected autofill provenance for publication_date: {:?}",
            after.field_provenance
        );
    }
}
