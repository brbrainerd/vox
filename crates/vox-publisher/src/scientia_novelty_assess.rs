//! Single novelty-assessment entry point: chrono-filter → score → conflicts → breakdown.
//!
//! # Bundle-type choice
//!
//! `assess_novelty` accepts `&NoveltyEvidenceBundleV1` (vox-publisher's own wire type) because
//! that is the type present in every CLI call site after `fetch_prior_art_federated`.  The
//! internal scorer (`AtomicNoveltyScorer`) and `ChronoFilter` both consume
//! `vox_research_events::schema_types::NoveltyEvidenceBundle`; we convert via a serde
//! round-trip (`serde_json::to_value` / `from_value`), which is already validated by the
//! contract-parity test in `vox-publisher/tests/novelty_bundle_contract_parity.rs`.

use serde::Serialize;
use vox_scientia::inspect_bridge::{
    AtomicNoveltyScorer, ChronoFilter, ClaimPolarity, EvidenceConflict, EvidenceConflictDetector,
    NoveltyConfig, NoveltyVerdict, PolarizedHit,
};

use crate::scientia_finding_ledger::{NormalizedPriorArtHit, NoveltyEvidenceBundleV1};

/// Explainable signal breakdown attached to every `NoveltyAssessment`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NoveltySignalBreakdown {
    /// Maximum semantic similarity across *filtered* hits (after chrono-filter).
    pub max_semantic: Option<f64>,
    /// Maximum lexical similarity across *filtered* hits (after chrono-filter).
    pub max_lexical: Option<f64>,
    /// Number of filtered hits with `semantic_score >= config.novel_threshold`.
    pub near_hit_count: usize,
    /// `cited_by_count` of the hit with the highest semantic score (if any).
    pub top_hit_citations: Option<u64>,
    /// Number of query traces that returned HTTP 2xx.
    pub sources_succeeded: usize,
}

/// Full novelty assessment result with flattened verdict fields for stable JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct NoveltyAssessment {
    /// `"insufficient_evidence"` | `"novel"` | `"possibly_novel"` | `"not_novel"`
    pub verdict_kind: String,
    /// URI of the closest prior-art hit (only for `not_novel`).
    pub closest_hit_uri: Option<String>,
    /// Closest similarity score (set for `possibly_novel` and `not_novel`).
    pub closest_score: Option<f64>,
    /// Any `EvidenceConflict` detected among the chrono-filtered hits.
    pub conflicts: Vec<EvidenceConflict>,
    /// Number of hits removed by the chrono-filter (year >= claim_year or year absent).
    pub excluded_future_hits: usize,
    /// Explainable signal breakdown.
    pub signals: NoveltySignalBreakdown,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert `NoveltyEvidenceBundleV1` → `vox_research_events::schema_types::NoveltyEvidenceBundle`
/// via serde round-trip.  Panics if the parity contract is broken (which the contract-parity
/// test already guards against in CI).
fn to_research_events_bundle(
    v1: &NoveltyEvidenceBundleV1,
) -> vox_research_events::schema_types::NoveltyEvidenceBundle {
    let val = serde_json::to_value(v1).expect("NoveltyEvidenceBundleV1 must be serializable");
    serde_json::from_value(val)
        .expect("NoveltyEvidenceBundleV1 must round-trip to NoveltyEvidenceBundle")
}

/// Derive claim polarity for conflict detection.
///
/// Strategy (no LLM; purely structural):
/// - If `overlap_note` contains "contradict", "refute", "dispute", or "oppose" → `Negative`.
/// - Otherwise (high-similarity hit with no contradiction note) → `Positive`.
/// - Hits below the conflict-detector threshold are irrelevant (the detector ignores them).
fn derive_polarity(hit: &NormalizedPriorArtHit) -> ClaimPolarity {
    if let Some(note) = &hit.overlap_note {
        let lower = note.to_lowercase();
        if lower.contains("contradict")
            || lower.contains("refute")
            || lower.contains("dispute")
            || lower.contains("oppose")
        {
            return ClaimPolarity::Negative;
        }
    }
    ClaimPolarity::Positive
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run a full novelty assessment pipeline:
///
/// 1. Chrono-filter hits to those predating `claim_year`.
/// 2. Recompute `overlap_summary` maxima over survivors.
/// 3. Score the filtered bundle with `AtomicNoveltyScorer`.
/// 4. Detect `EvidenceConflict`s among survivors.
/// 5. Fill a `NoveltySignalBreakdown`.
///
/// `query_traces` are always preserved (the scorer's `InsufficientEvidence` rule depends on them).
pub fn assess_novelty(
    bundle: &NoveltyEvidenceBundleV1,
    claim_year: i32,
    config: &NoveltyConfig,
) -> NoveltyAssessment {
    // ------------------------------------------------------------------
    // 1. Chrono-filter
    // ------------------------------------------------------------------
    // Convert to the research-events type so we can use ChronoFilter directly.
    let re_bundle = to_research_events_bundle(bundle);

    // Build a ChronoFilter from claim_year.  ChronoFilter normally takes a Unix
    // timestamp, but its `filter_hits` method just compares `h.year < claim_year()`.
    // We synthesise a timestamp that maps to exactly `claim_year` via the inverse of
    // its formula: ts = (claim_year - 1970) * 365.2425 * 86_400
    let claim_ts = ((claim_year as f64 - 1970.0) * 365.2425 * 86_400.0) as i64;
    let chrono_filter = ChronoFilter::new(claim_ts);

    let kept_refs: Vec<&vox_research_events::schema_types::NormalizedHit> =
        chrono_filter.filter_hits(&re_bundle.normalized_hits);
    let excluded_future_hits = bundle.normalized_hits.len() - kept_refs.len();

    // ------------------------------------------------------------------
    // 2. Build a filtered bundle (keep query_traces intact)
    // ------------------------------------------------------------------
    let filtered_hits: Vec<vox_research_events::schema_types::NormalizedHit> =
        kept_refs.iter().map(|h| (*h).clone()).collect();

    // Recompute overlap_summary over survivors; preserve recency_bucket from original.
    let max_sem_filtered = filtered_hits
        .iter()
        .filter_map(|h| h.semantic_score)
        .reduce(f64::max);
    let max_lex_filtered = filtered_hits
        .iter()
        .filter_map(|h| h.lexical_score)
        .reduce(f64::max);

    let filtered_overlap = max_sem_filtered.or(max_lex_filtered).map(|_| {
        vox_research_events::schema_types::OverlapSummary {
            max_lexical_score: max_lex_filtered,
            max_semantic_score: max_sem_filtered,
            recency_bucket: re_bundle
                .overlap_summary
                .as_ref()
                .and_then(|o| o.recency_bucket.clone()),
        }
    });

    let filtered_bundle = vox_research_events::schema_types::NoveltyEvidenceBundle {
        schema_version: re_bundle.schema_version,
        bundle_id: re_bundle.bundle_id.clone(),
        candidate_id: re_bundle.candidate_id.clone(),
        computed_at_ms: re_bundle.computed_at_ms,
        query_digest_sha256: re_bundle.query_digest_sha256.clone(),
        sources: re_bundle.sources.clone(),
        normalized_hits: filtered_hits,
        overlap_summary: filtered_overlap,
        query_traces: re_bundle.query_traces.clone(), // PRESERVE for InsufficientEvidence rule
    };

    // ------------------------------------------------------------------
    // 3. Score
    // ------------------------------------------------------------------
    let scorer = AtomicNoveltyScorer::new(NoveltyConfig {
        novel_threshold: config.novel_threshold,
        not_novel_threshold: config.not_novel_threshold,
    });
    let verdict = scorer.score(&filtered_bundle);

    // ------------------------------------------------------------------
    // 4. Conflict detection
    // ------------------------------------------------------------------
    // Map the filtered V1 hits back to PolarizedHit for the detector.
    // We use the kept_refs (which still reference the original V1-shaped data
    // via the round-tripped re_bundle) but re-index through the original bundle
    // to access `overlap_note` on the publisher type.
    //
    // The chrono filter drops hits, so positional indexing into the original
    // `bundle.normalized_hits` is wrong. Match each kept hit back to its
    // original by `work_uri` to recover the polarity-bearing `overlap_note`.
    let polarized: Vec<PolarizedHit> = kept_refs
        .iter()
        .map(|h| {
            let polarity = bundle
                .normalized_hits
                .iter()
                .find(|v1| v1.work_uri == h.work_uri)
                .map(derive_polarity)
                .unwrap_or(ClaimPolarity::Positive);
            PolarizedHit {
                work_uri: h.work_uri.clone(),
                similarity: h.semantic_score.unwrap_or(0.0),
                polarity,
                excerpt: h.overlap_note.clone(),
            }
        })
        .collect();

    let detector = EvidenceConflictDetector::default();
    let conflict_opt = detector.detect("", &polarized);
    let conflicts: Vec<EvidenceConflict> = conflict_opt.into_iter().collect();

    // ------------------------------------------------------------------
    // 5. Signal breakdown
    // ------------------------------------------------------------------
    let sources_succeeded = filtered_bundle
        .query_traces
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|t| t.http_status.is_some_and(|s| (200..300).contains(&s)))
        .count();

    let near_hit_count = filtered_bundle
        .normalized_hits
        .iter()
        .filter(|h| {
            h.semantic_score
                .is_some_and(|s| s >= config.novel_threshold)
        })
        .count();

    // Top-cited hit = the one with the highest semantic score.
    let top_hit_citations = filtered_bundle
        .normalized_hits
        .iter()
        .max_by(|a, b| {
            a.semantic_score
                .unwrap_or(0.0)
                .partial_cmp(&b.semantic_score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|h| {
            // cited_by_count lives on the original V1 hit (same positional index).
            // The research-events NormalizedHit also carries cited_by_count, so use it.
            h.cited_by_count
        });

    let signals = NoveltySignalBreakdown {
        max_semantic: max_sem_filtered,
        max_lexical: max_lex_filtered,
        near_hit_count,
        top_hit_citations,
        sources_succeeded,
    };

    // ------------------------------------------------------------------
    // 6. Flatten verdict
    // ------------------------------------------------------------------
    let (verdict_kind, closest_hit_uri, closest_score) = match verdict {
        // main's B6 verdict carries a `reason`; we surface the flattened kind only.
        NoveltyVerdict::InsufficientEvidence { .. } => {
            ("insufficient_evidence".to_string(), None, None)
        }
        // main's B6 `Contradicted { conflicting_uri }`: a prior-art conflict caps novelty.
        NoveltyVerdict::Contradicted { conflicting_uri } => {
            ("contradicted".to_string(), Some(conflicting_uri), None)
        }
        NoveltyVerdict::Novel => ("novel".to_string(), None, None),
        NoveltyVerdict::PossiblyNovel { closest_score } => {
            ("possibly_novel".to_string(), None, Some(closest_score))
        }
        NoveltyVerdict::NotNovel {
            closest_hit_uri,
            similarity,
        } => (
            "not_novel".to_string(),
            Some(closest_hit_uri),
            Some(similarity),
        ),
    };

    NoveltyAssessment {
        verdict_kind,
        closest_hit_uri,
        closest_score,
        conflicts,
        excluded_future_hits,
        signals,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scientia_finding_ledger::{
        NormalizedPriorArtHit, NoveltyEvidenceBundleV1, NoveltyQueryTrace, PriorArtSource,
    };

    fn make_hit(
        uri: &str,
        year: Option<i32>,
        semantic: Option<f64>,
        lexical: Option<f64>,
        cited_by: Option<u64>,
        overlap_note: Option<&str>,
    ) -> NormalizedPriorArtHit {
        NormalizedPriorArtHit {
            source: PriorArtSource::Manual,
            work_uri: uri.to_string(),
            title: "Test hit".to_string(),
            year,
            lexical_score: lexical,
            semantic_score: semantic,
            overlap_note: overlap_note.map(str::to_string),
            cited_by_count: cited_by,
        }
    }

    fn make_trace(status: Option<i32>) -> NoveltyQueryTrace {
        NoveltyQueryTrace {
            source: "openalex".to_string(),
            request_fingerprint_sha256: "a".repeat(64),
            http_status: status,
            cached: None,
        }
    }

    fn base_bundle(
        hits: Vec<NormalizedPriorArtHit>,
        traces: Vec<NoveltyQueryTrace>,
    ) -> NoveltyEvidenceBundleV1 {
        NoveltyEvidenceBundleV1 {
            schema_version: 1,
            bundle_id: "B-test".to_string(),
            candidate_id: "C-test".to_string(),
            computed_at_ms: 0,
            query_digest_sha256: "a".repeat(64),
            sources: vec![PriorArtSource::Manual],
            normalized_hits: hits,
            overlap_summary: None,
            query_traces: traces,
        }
    }

    /// A future-dated hit (year 2030) must be excluded, leaving only hits < claim_year 2026.
    /// One successful trace (200) → verdict "novel".
    #[test]
    fn future_dated_hits_are_excluded_before_scoring() {
        let hits = vec![make_hit(
            "doi:10.future",
            Some(2030),
            Some(0.95),
            None,
            None,
            None,
        )];
        let bundle = base_bundle(hits, vec![make_trace(Some(200))]);
        let config = NoveltyConfig::default();
        let result = assess_novelty(&bundle, 2026, &config);
        assert_eq!(result.verdict_kind, "novel", "future hit excluded → novel");
        assert_eq!(result.excluded_future_hits, 1);
    }

    /// A hit with overlap_note containing "contradict" and semantic_score 0.9 (>= default
    /// EvidenceConflictDetector threshold 0.8) should trigger conflict detection.
    /// We also add a supporting hit to satisfy the Positive+Negative requirement.
    #[test]
    fn contradicting_hit_surfaces_conflict_not_novel() {
        let hits = vec![
            // Supporting hit: high sim, year in range, no contradiction note
            make_hit("doi:10.support", Some(2022), Some(0.9), None, None, None),
            // Contradicting hit: high sim, overlap_note flags contradiction
            make_hit(
                "doi:10.contra",
                Some(2021),
                Some(0.9),
                None,
                None,
                Some("this work contradicts the proposed approach"),
            ),
        ];
        let bundle = base_bundle(hits, vec![make_trace(Some(200))]);
        let config = NoveltyConfig::default(); // not_novel_threshold = 0.8
        let result = assess_novelty(&bundle, 2026, &config);
        assert!(!result.conflicts.is_empty(), "conflict should be detected");
        assert_eq!(result.verdict_kind, "not_novel", "sim >= 0.8 → not_novel");
    }

    /// 3 hits with sims [0.55, 0.6, 0.3], novel_threshold default 0.5.
    /// near_hit_count should be 2 (sims >= 0.5: 0.55 and 0.6).
    /// sources_succeeded = 1 (one 200, one 500).
    /// top_hit_citations = from the hit with sim 0.6.
    #[test]
    fn breakdown_counts_near_hits_and_sources() {
        let hits = vec![
            make_hit("doi:10.a", Some(2020), Some(0.55), None, Some(10), None),
            make_hit("doi:10.b", Some(2021), Some(0.6), None, Some(99), None),
            make_hit("doi:10.c", Some(2019), Some(0.3), None, Some(5), None),
        ];
        let traces = vec![make_trace(Some(200)), make_trace(Some(500))];
        let bundle = base_bundle(hits, traces);
        let config = NoveltyConfig::default();
        let result = assess_novelty(&bundle, 2026, &config);
        assert_eq!(result.signals.near_hit_count, 2, "sims 0.55 and 0.6 >= 0.5");
        assert_eq!(
            result.signals.sources_succeeded, 1,
            "only the 200 trace succeeds"
        );
        assert_eq!(
            result.signals.top_hit_citations,
            Some(99),
            "hit with sim=0.6 has cited_by=99"
        );
    }

    /// Empty hits + failed trace (500) → InsufficientEvidence.
    #[test]
    fn no_hits_failed_traces_is_insufficient() {
        let bundle = base_bundle(vec![], vec![make_trace(Some(500))]);
        let config = NoveltyConfig::default();
        let result = assess_novelty(&bundle, 2026, &config);
        assert_eq!(result.verdict_kind, "insufficient_evidence");
    }
}
