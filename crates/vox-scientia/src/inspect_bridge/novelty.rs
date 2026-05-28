//! Atomic-NEI novelty scoring over `NoveltyEvidenceBundle`.
//!
//! Uses `overlap_summary.max_semantic_score` as the primary signal.
//! SPECTER2 integration is a stub — Phase 8 wires the actual model.

use vox_research_events::schema_types::NoveltyEvidenceBundle;

/// The result of an atomic-NEI novelty assessment.
#[derive(Debug, Clone, PartialEq)]
pub enum NoveltyVerdict {
    /// No high-similarity prior art found (max_semantic_score < novel_threshold).
    Novel,
    /// Borderline: similarity above the novel threshold but below the not-novel threshold.
    PossiblyNovel { closest_score: f64 },
    /// Clear prior art found: similarity >= not_novel_threshold.
    NotNovel {
        closest_hit_uri: String,
        similarity: f64,
    },
}

/// Thresholds controlling the novelty classification boundaries.
pub struct NoveltyConfig {
    /// Max semantic score below which the claim is considered Novel (default 0.5).
    pub novel_threshold: f64,
    /// Max semantic score at or above which the claim is NotNovel (default 0.8).
    pub not_novel_threshold: f64,
}

impl Default for NoveltyConfig {
    fn default() -> Self {
        Self {
            novel_threshold: 0.5,
            not_novel_threshold: 0.8,
        }
    }
}

/// Scores a `NoveltyEvidenceBundle` and returns a `NoveltyVerdict`.
#[derive(Default)]
pub struct AtomicNoveltyScorer {
    pub config: NoveltyConfig,
}

impl AtomicNoveltyScorer {
    pub fn new(config: NoveltyConfig) -> Self {
        Self { config }
    }

    /// Score a bundle.
    ///
    /// Decision ladder (uses `overlap_summary.max_semantic_score`):
    /// - `None` or `< novel_threshold`  → `Novel`
    /// - `>= not_novel_threshold`        → `NotNovel` (URI from the hit with the highest
    ///   `semantic_score`, falling back to the first hit if scores are absent)
    /// - otherwise                       → `PossiblyNovel { closest_score }`
    pub fn score(&self, bundle: &NoveltyEvidenceBundle) -> NoveltyVerdict {
        // Derive max score from overlap_summary if present, otherwise scan hits directly.
        let max_score = bundle
            .overlap_summary
            .as_ref()
            .and_then(|s| s.max_semantic_score)
            .or_else(|| {
                bundle
                    .normalized_hits
                    .iter()
                    .filter_map(|h| h.semantic_score)
                    .reduce(f64::max)
            });

        match max_score {
            None => NoveltyVerdict::Novel,
            Some(score) if score < self.config.novel_threshold => NoveltyVerdict::Novel,
            Some(score) if score >= self.config.not_novel_threshold => {
                // Find the URI of the hit with the highest semantic_score.
                let closest_uri = bundle
                    .normalized_hits
                    .iter()
                    .max_by(|a, b| {
                        a.semantic_score
                            .unwrap_or(0.0)
                            .partial_cmp(&b.semantic_score.unwrap_or(0.0))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|h| h.work_uri.clone())
                    .unwrap_or_default();
                NoveltyVerdict::NotNovel {
                    closest_hit_uri: closest_uri,
                    similarity: score,
                }
            }
            Some(score) => NoveltyVerdict::PossiblyNovel {
                closest_score: score,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::schema_types::{
        NormalizedHit, NoveltyEvidenceBundle, NoveltySource, OverlapSummary,
    };

    fn make_bundle(hits: Vec<NormalizedHit>, max_semantic: Option<f64>) -> NoveltyEvidenceBundle {
        NoveltyEvidenceBundle {
            schema_version: 1,
            bundle_id: "B-test".to_string(),
            candidate_id: "C-test".to_string(),
            computed_at_ms: 0,
            query_digest_sha256: "a".repeat(64),
            sources: vec![NoveltySource::Manual],
            normalized_hits: hits,
            overlap_summary: max_semantic.map(|s| OverlapSummary {
                max_lexical_score: None,
                max_semantic_score: Some(s),
                recency_bucket: None,
            }),
            query_traces: None,
        }
    }

    fn hit(work_uri: &str, semantic_score: Option<f64>) -> NormalizedHit {
        NormalizedHit {
            source: NoveltySource::Manual,
            work_uri: work_uri.to_string(),
            title: "Test hit".to_string(),
            year: None,
            lexical_score: None,
            semantic_score,
            overlap_note: None,
            cited_by_count: None,
        }
    }

    #[test]
    fn empty_bundle_is_novel() {
        let bundle = make_bundle(vec![], None);
        let scorer = AtomicNoveltyScorer::default();
        assert_eq!(scorer.score(&bundle), NoveltyVerdict::Novel);
    }

    #[test]
    fn low_score_is_novel() {
        let bundle = make_bundle(vec![hit("doi:10.1/low", Some(0.3))], Some(0.3));
        let scorer = AtomicNoveltyScorer::default();
        assert_eq!(scorer.score(&bundle), NoveltyVerdict::Novel);
    }

    #[test]
    fn high_score_is_not_novel() {
        let bundle = make_bundle(vec![hit("doi:10.x", Some(0.85))], Some(0.85));
        let scorer = AtomicNoveltyScorer::default();
        assert!(matches!(
            scorer.score(&bundle),
            NoveltyVerdict::NotNovel { closest_hit_uri, similarity }
            if closest_hit_uri == "doi:10.x" && (similarity - 0.85).abs() < 1e-9
        ));
    }

    #[test]
    fn mid_score_is_possibly_novel() {
        let bundle = make_bundle(vec![hit("doi:10.mid", Some(0.65))], Some(0.65));
        let scorer = AtomicNoveltyScorer::default();
        assert!(matches!(
            scorer.score(&bundle),
            NoveltyVerdict::PossiblyNovel { closest_score }
            if (closest_score - 0.65).abs() < 1e-9
        ));
    }

    #[test]
    fn no_summary_no_scores_is_novel() {
        let bundle = make_bundle(vec![hit("doi:10.test", None)], None);
        let scorer = AtomicNoveltyScorer::default();
        let verdict = scorer.score(&bundle);
        assert_eq!(verdict, NoveltyVerdict::Novel);
    }
}
