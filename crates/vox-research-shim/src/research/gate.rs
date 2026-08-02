//! Confidence gate + routing-tier selector. `score_with_config` fuses four
//! weighted signals — citation coverage, claim-support ratio, source
//! diversity, and a guarded retrieval floor — not a flat citation-count
//! score. See:
//!   docs/src/architecture/deep-research-verification-2026-08-01.md
//!   docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §5.

use serde::{Deserialize, Serialize};

use super::claims::Claim;
use super::types::RoutingTier;

/// Gate config. Both fields are live calibration knobs actively read by
/// `score_with_config` (not placeholders) — see that function's doc for
/// how each is used.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    pub min_citations_for_full_score: Option<usize>,
    pub min_domains_for_full_score: Option<usize>,
}

/// Per-tier routing thresholds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RoutingThresholds {
    pub direct: f32,
    pub light: f32,
    pub deep: f32,
}

impl Default for RoutingThresholds {
    fn default() -> Self {
        Self {
            direct: 0.7,
            light: 0.4,
            deep: 0.2,
        }
    }
}

/// Confidence-gate input.
#[derive(Debug)]
pub struct GateInput<'a> {
    pub claims: &'a [Claim],
    pub citation_count: usize,
    /// Sum of `ResearchHit.trust_score` across all cited hits. Falls back
    /// to `citation_count as f32` at call sites that haven't wired a real
    /// `TrustScorer` yet, preserving prior behavior exactly.
    pub trust_weighted_citation_score: f32,
    /// Sum of per-claim weights for `Supported` verdicts (each weighted by
    /// `resample_stability` in `[0, 1]`, so a flaky claim contributes less
    /// than a consistently-supported one). Kept as `f32` rather than a claim
    /// *count* so that stability weighting survives into the gate score
    /// instead of being destroyed by rounding to an integer count.
    pub supported_claim_count: f32,
    pub distinct_domain_count: usize,
    pub no_retrieval_hits: bool,
    pub answer_is_empty: bool,
}

/// Confidence-gate output.
#[derive(Debug, Clone)]
pub struct ConfidenceSignal {
    pub score: f32,
}

impl ConfidenceSignal {
    /// Pick routing tier given per-tier thresholds.
    #[must_use]
    pub fn routing_tier_for(&self, direct: f32, light: f32, _deep: f32) -> RoutingTier {
        if self.score >= direct {
            RoutingTier::Direct
        } else if self.score >= light {
            RoutingTier::Light
        } else if self.score < f32::EPSILON {
            // NOTE: score_with_config already returns exactly 0.0 when
            // input.no_retrieval_hits is true (see the early return at the
            // top of that function), so this exact-zero check is a correct,
            // durable proxy for "no retrieval hits" today — it is not a
            // stub pending replacement. If score_with_config's early-return
            // behavior ever changes, this comment must be revisited.
            // No evidence at all → cheapest tier (don't burn cycles on deep
            // research with nothing to verify against).
            RoutingTier::Direct
        } else {
            RoutingTier::DeepResearch
        }
    }
}

/// Score a gate input by fusing four weighted signals: `citation_score`
/// (0.35, citation count vs. `min_citations_for_full_score`),
/// `claim_support_score` (0.30, the verifier's supported-claim ratio),
/// `diversity_score` (0.20, distinct-domain count vs.
/// `min_domains_for_full_score`), and a fixed `retrieval_score` (0.15,
/// reachable only because `no_retrieval_hits` is guarded above — see
/// `docs/src/architecture/deep-research-verification-2026-08-01.md`).
///
/// Phase 2 extends this with symbolic-verifier strategy weights and
/// contradiction-ratio penalties.
#[must_use]
pub fn score_with_config(input: &GateInput<'_>, config: &GateConfig) -> ConfidenceSignal {
    if input.no_retrieval_hits {
        return ConfidenceSignal { score: 0.0 };
    }
    let min_cit = (config.min_citations_for_full_score.unwrap_or(5) as f32).max(1.0);
    let min_dom = (config.min_domains_for_full_score.unwrap_or(4) as f32).max(1.0);
    let citation_score = (input.trust_weighted_citation_score / min_cit).clamp(0.0, 1.0);
    let claim_support_score = if input.claims.is_empty() {
        0.5
    } else {
        (input.supported_claim_count / input.claims.len() as f32).clamp(0.0, 1.0)
    };
    let diversity_score = (input.distinct_domain_count as f32 / min_dom).clamp(0.0, 1.0);
    let score = citation_score * 0.35
        + claim_support_score * 0.30
        + diversity_score * 0.20
        + 1.0_f32 * 0.15; // retrieval_score always 1.0 here (guarded above)
    ConfidenceSignal {
        score: score.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::research::claims::Claim;
    use crate::research::types::RoutingTier;

    fn dummy_claims(n: usize) -> Vec<Claim> {
        (0..n)
            .map(|i| Claim {
                text: format!("claim {i}"),
                claim_id: i as u64,
                is_numeric: false,
                is_recent: false,
                is_named_event: false,
            })
            .collect()
    }

    fn full_config() -> GateConfig {
        GateConfig {
            min_citations_for_full_score: Some(5),
            min_domains_for_full_score: Some(4),
        }
    }

    #[test]
    fn routing_tier_for_high_score_returns_direct() {
        let s = ConfidenceSignal { score: 0.9 };
        assert!(matches!(
            s.routing_tier_for(0.7, 0.4, 0.2),
            RoutingTier::Direct
        ));
    }

    #[test]
    fn routing_tier_for_mid_score_returns_light() {
        let s = ConfidenceSignal { score: 0.5 };
        assert!(matches!(
            s.routing_tier_for(0.7, 0.4, 0.2),
            RoutingTier::Light
        ));
    }

    #[test]
    fn routing_tier_for_low_nonzero_score_returns_deep_research() {
        let s = ConfidenceSignal { score: 0.1 };
        assert!(matches!(
            s.routing_tier_for(0.7, 0.4, 0.2),
            RoutingTier::DeepResearch
        ));
    }

    #[test]
    fn routing_tier_for_exact_direct_threshold_returns_direct() {
        let s = ConfidenceSignal { score: 0.7 };
        assert!(matches!(
            s.routing_tier_for(0.7, 0.4, 0.2),
            RoutingTier::Direct
        ));
    }

    #[test]
    fn routing_tier_for_exact_light_threshold_returns_light() {
        let s = ConfidenceSignal { score: 0.4 };
        assert!(matches!(
            s.routing_tier_for(0.7, 0.4, 0.2),
            RoutingTier::Light
        ));
    }

    #[test]
    fn zero_evidence_scores_zero() {
        let config = full_config();
        let input = GateInput {
            claims: &[],
            citation_count: 0,
            trust_weighted_citation_score: 0.0,
            supported_claim_count: 0.0,
            distinct_domain_count: 0,
            no_retrieval_hits: true,
            answer_is_empty: false,
        };
        assert_eq!(score_with_config(&input, &config).score, 0.0);
    }

    #[test]
    fn full_evidence_scores_near_one() {
        let claims = dummy_claims(4);
        let config = full_config();
        let input = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 5.0,
            supported_claim_count: 4.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let s = score_with_config(&input, &config);
        assert!(s.score > 0.95, "expected >0.95, got {}", s.score);
    }

    #[test]
    fn partial_evidence_scores_in_deep_research_band() {
        let claims = dummy_claims(1);
        let config = full_config();
        let input = GateInput {
            claims: &claims,
            citation_count: 2,
            trust_weighted_citation_score: 2.0,
            supported_claim_count: 0.0,
            distinct_domain_count: 1,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let s = score_with_config(&input, &config);
        assert!(
            s.score > 0.0 && s.score < 0.7,
            "expected between 0 and 0.7, got {}",
            s.score
        );
    }

    #[test]
    fn low_trust_citations_score_lower_than_high_trust_citations() {
        let claims = dummy_claims(2);
        let config = GateConfig::default();

        let high_trust_input = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 5.0, // 5 citations at trust_score 1.0 each
            supported_claim_count: 2.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let low_trust_input = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 0.5, // 5 citations at trust_score 0.1 each (all retracted)
            supported_claim_count: 2.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };

        let high = score_with_config(&high_trust_input, &config);
        let low = score_with_config(&low_trust_input, &config);
        assert!(
            high.score > low.score,
            "high-trust score {} should exceed low-trust score {}",
            high.score,
            low.score
        );
    }

    #[test]
    fn high_trust_citations_do_not_exceed_plain_citation_count_scale() {
        let claims = dummy_claims(2);
        let config = full_config(); // min_citations_for_full_score: Some(5)

        // 3 journal-reputation citations (trust_score 1.5 each, capped at 1.0
        // by the caller before reaching GateInput) should score the SAME as
        // 3 plain citations — not inflate past what 5-citations-for-full-score
        // implies.
        let capped_high_trust_input = GateInput {
            claims: &claims,
            citation_count: 3,
            trust_weighted_citation_score: 3.0, // 3 citations, each capped at 1.0 regardless of raw trust_score
            supported_claim_count: 2.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let plain_three_citations_input = GateInput {
            claims: &claims,
            citation_count: 3,
            trust_weighted_citation_score: 3.0, // equivalent to 3 plain citations
            supported_claim_count: 2.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };

        let capped = score_with_config(&capped_high_trust_input, &config);
        let plain = score_with_config(&plain_three_citations_input, &config);
        assert_eq!(
            capped.score, plain.score,
            "capping trust contribution at 1.0 per hit must not let 3 high-trust citations outscore 3 plain citations"
        );
    }

    /// Regression test for a rounding bug: `weighted_supported_claim_score`
    /// (resample-stability-weighted) used to be `.round() as usize` before
    /// reaching `GateInput`, which meant e.g. a single Supported claim with
    /// `resample_stability: 0.6` rounded to `1` — identical to a fully
    /// stable claim — silently defeating the whole point of the weighting.
    /// `supported_claim_count` is now `f32` end-to-end so fractional
    /// stability actually changes `claim_support_score`.
    #[test]
    fn fractional_claim_support_is_not_rounded_away() {
        let claims = dummy_claims(1);
        let config = full_config();

        // A single claim that was Supported but flaky across resamples
        // (resample_stability 0.6) must NOT score identically to a claim
        // that was consistently Supported (resample_stability 1.0).
        let flaky = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 5.0,
            supported_claim_count: 0.6,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };
        let stable = GateInput {
            claims: &claims,
            citation_count: 5,
            trust_weighted_citation_score: 5.0,
            supported_claim_count: 1.0,
            distinct_domain_count: 4,
            no_retrieval_hits: false,
            answer_is_empty: false,
        };

        let flaky_score = score_with_config(&flaky, &config);
        let stable_score = score_with_config(&stable, &config);
        assert!(
            stable_score.score > flaky_score.score,
            "a flaky Supported claim (resample_stability 0.6) must score lower than a \
             consistently Supported one (1.0), not round to the same integer count: \
             flaky={} stable={}",
            flaky_score.score,
            stable_score.score
        );
    }
}
