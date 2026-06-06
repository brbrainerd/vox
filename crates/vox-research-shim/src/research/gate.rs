//! Confidence gate + routing-tier selector. Phase 0a STUB — produces a flat
//! score derived purely from citation count; no claim-level scoring.
//!
//! Phase 2 wires this to the symbolic-verifier strategies and the prereg
//! enforcement layer. See:
//!   docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §5.

use serde::{Deserialize, Serialize};

use super::claims::Claim;
use super::types::RoutingTier;

/// Gate config. Phase 0a — placeholders for Phase 2 calibration knobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    pub min_citations_for_full_score: Option<usize>,
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
        } else if self.score == 0.0 {
            // PHASE_0a_STUB: exact-zero check is valid only while score_with_config
            // produces an integer-derived float (citation_count / 5.0). Phase 2
            // multi-signal fusion may produce non-zero scores with no retrieval hits;
            // replace with `input.no_retrieval_hits` passed through the call chain.
            // No evidence at all → cheapest tier (don't burn cycles on deep
            // research with nothing to verify against).
            RoutingTier::Direct
        } else {
            RoutingTier::DeepResearch
        }
    }
}

/// Score a gate input. Phase 0a stub — flat function of citation count.
///
/// Phase 2 replaces this with a fusion of symbolic-verifier strengths,
/// claim-evidence coverage, and contradiction ratio.
#[must_use]
pub fn score_with_config(input: &GateInput<'_>, _config: &GateConfig) -> ConfidenceSignal {
    // PHASE_0a_STUB: simple linear function of citation count, capped at 1.0.
    let raw = (input.citation_count as f32) / 5.0;
    ConfidenceSignal {
        score: raw.clamp(0.0, 1.0),
    }
}
