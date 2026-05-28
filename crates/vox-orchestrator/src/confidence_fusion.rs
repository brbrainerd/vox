//! Composite confidence fusion for Socrates invocation decision (D3).
//!
//! Blends five evidence signals into a single score, then applies thresholds
//! from `FusionConfig` to decide whether to invoke Socrates or answer directly.
//! Thresholds mirror `contracts/orchestration/socrates-fusion.v1.yaml`.
//! All checks are pure: no async, no I/O.

use serde::{Deserialize, Serialize};

use crate::socrates::SocratesTaskContext;

/// Weights applied to each evidence signal. Must sum to 1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionWeights {
    pub evidence_quality: f64,
    pub citation_coverage: f64,
    pub logprob_entropy: f64,
    pub sep_estimate: f64,
    pub self_consistency: f64,
}

impl Default for FusionWeights {
    fn default() -> Self {
        Self {
            evidence_quality: 0.15,
            citation_coverage: 0.15,
            logprob_entropy: 0.30,
            sep_estimate: 0.20,
            self_consistency: 0.20,
        }
    }
}

/// Raw signal values extracted before weighting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionInputs {
    /// Retrieval-side evidence quality proxy in `[0, 1]`.
    pub evidence_quality: f64,
    /// Retrieval-side citation coverage proxy in `[0, 1]`.
    pub citation_coverage: f64,
    /// Model log probability entropy.
    pub logprob_entropy: f64,
    /// Semantic entropy estimate based on meaning clustering.
    pub sep_estimate: f64,
    /// Self-consistency across multiple samples.
    pub self_consistency: f64,
}

impl FusionInputs {
    /// Build inputs from a [`SocratesTaskContext`] and an optional completion text entropy.
    #[must_use]
    pub fn from_task_context(
        ctx: &SocratesTaskContext,
        completion_entropy_score: Option<f64>,
    ) -> Self {
        let logprob_entropy = completion_entropy_score.unwrap_or(0.5).clamp(0.0, 1.0);
        let sep_estimate = (ctx.source_diversity as f64 / 5.0).clamp(0.0, 1.0);
        let self_consistency = match ctx.contradiction_hints {
            0 => 1.0,
            1 => 0.7,
            2 => 0.4,
            _ => 0.1,
        };
        Self {
            evidence_quality: ctx.evidence_quality.clamp(0.0, 1.0),
            citation_coverage: ctx.citation_coverage.clamp(0.0, 1.0),
            logprob_entropy,
            sep_estimate,
            self_consistency,
        }
    }
}

/// Outcome of the fusion step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FusionDecision {
    Ship,
    Resample,
    Retrieve,
    SpawnSocrates,
    Abstain,
}

impl std::fmt::Display for FusionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ship => write!(f, "ship"),
            Self::Resample => write!(f, "resample"),
            Self::Retrieve => write!(f, "retrieve"),
            Self::SpawnSocrates => write!(f, "spawn-socrates"),
            Self::Abstain => write!(f, "abstain"),
        }
    }
}

/// Threshold configuration. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    pub ship: f64,
    pub resample: f64,
    pub retrieve: f64,
    pub spawn_socrates: f64,
    pub abstain: f64,
    pub weights: FusionWeights,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            ship: 0.85,
            resample: 0.70,
            retrieve: 0.50,
            spawn_socrates: 0.30,
            abstain: 0.0,
            weights: FusionWeights::default(),
        }
    }
}

/// Pure, allocation-free confidence fuser.
pub struct ConfidenceFuser {
    config: FusionConfig,
}

impl ConfidenceFuser {
    pub fn new(config: FusionConfig) -> Self {
        Self { config }
    }

    /// Compute the composite confidence score from pre-computed inputs.
    ///
    /// Score = Σ(signal_i × weight_i) where contradiction is inverted first.
    #[must_use]
    #[inline]
    pub fn score(&self, inputs: &FusionInputs) -> f64 {
        let w = &self.config.weights;
        (inputs.evidence_quality * w.evidence_quality
            + inputs.citation_coverage * w.citation_coverage
            + inputs.logprob_entropy * w.logprob_entropy
            + inputs.sep_estimate * w.sep_estimate
            + inputs.self_consistency * w.self_consistency)
            .clamp(0.0, 1.0)
    }

    /// Apply thresholds to a pre-computed score and return the routing decision.
    #[must_use]
    #[inline]
    pub fn decide(&self, score: f64) -> FusionDecision {
        if score >= self.config.ship {
            FusionDecision::Ship
        } else if score >= self.config.resample {
            FusionDecision::Resample
        } else if score >= self.config.retrieve {
            FusionDecision::Retrieve
        } else if score >= self.config.spawn_socrates {
            FusionDecision::SpawnSocrates
        } else {
            FusionDecision::Abstain
        }
    }

    /// Convenience: score + decide in one call.
    #[must_use]
    #[inline]
    pub fn evaluate(&self, inputs: &FusionInputs) -> (f64, FusionDecision) {
        let score = self.score(inputs);
        (score, self.decide(score))
    }
}

/// Metric payload emitted when a fusion decision is made.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct FusionEvent {
    pub metric_type: &'static str,
    pub score: f64,
    pub decision: String,
    pub session_id: Option<String>,
}

impl FusionEvent {
    pub fn new(score: f64, decision: FusionDecision, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_SOCRATES_FUSION,
            score,
            decision: decision.to_string(),
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fuser() -> ConfidenceFuser {
        ConfidenceFuser::new(FusionConfig::default())
    }

    fn high_quality_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.9,
            citation_coverage: 0.85,
            logprob_entropy: 0.9,
            sep_estimate: 0.8,
            self_consistency: 1.0,
        }
    }

    fn low_quality_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.1,
            citation_coverage: 0.1,
            logprob_entropy: 0.1,
            sep_estimate: 0.0,
            self_consistency: 0.1,
        }
    }

    #[test]
    fn high_quality_ships() {
        let f = fuser();
        let (_, decision) = f.evaluate(&high_quality_inputs());
        assert_eq!(decision, FusionDecision::Ship);
    }

    #[test]
    fn low_quality_abstains() {
        let f = fuser();
        let (_, decision) = f.evaluate(&low_quality_inputs());
        assert_eq!(decision, FusionDecision::Abstain);
    }

    #[test]
    fn mid_quality_spawns_socrates() {
        let f = fuser();
        let inputs = FusionInputs {
            evidence_quality: 0.4,
            citation_coverage: 0.4,
            logprob_entropy: 0.4,
            sep_estimate: 0.4,
            self_consistency: 0.4,
        };
        let (_, decision) = f.evaluate(&inputs);
        assert_eq!(decision, FusionDecision::SpawnSocrates);
    }

    #[test]
    fn score_clamped_between_zero_and_one() {
        let f = fuser();
        let extreme = FusionInputs {
            evidence_quality: 2.0,
            citation_coverage: 2.0,
            logprob_entropy: 2.0,
            sep_estimate: -1.0,
            self_consistency: 2.0,
        };
        let score = f.score(&extreme);
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn from_task_context_maps_fields() {
        let ctx = SocratesTaskContext {
            evidence_quality: 0.7,
            citation_coverage: 0.6,
            source_diversity: 3,
            contradiction_hints: 1,
            ..Default::default()
        };
        let inputs = FusionInputs::from_task_context(&ctx, Some(0.65));
        assert!((inputs.evidence_quality - 0.7).abs() < 1e-9);
        assert!((inputs.citation_coverage - 0.6).abs() < 1e-9);
        assert!((inputs.logprob_entropy - 0.65).abs() < 1e-9);
        assert!((inputs.sep_estimate - 0.6).abs() < 1e-9);
        assert!((inputs.self_consistency - 0.7).abs() < 1e-9);
    }

    #[test]
    fn fusion_event_has_correct_metric_type() {
        let event = FusionEvent::new(0.9, FusionDecision::Ship, None);
        assert_eq!(event.metric_type, "orch.socrates.fusion");
    }

    #[test]
    fn decide_at_exact_ship_threshold() {
        let f = fuser();
        assert_eq!(f.decide(0.85), FusionDecision::Ship);
    }

    #[test]
    fn decide_just_below_ship_threshold() {
        let f = fuser();
        assert_eq!(f.decide(0.84), FusionDecision::Resample);
    }

    #[test]
    fn decide_at_exact_spawn_socrates_threshold() {
        let f = fuser();
        assert_eq!(f.decide(0.30), FusionDecision::SpawnSocrates);
    }

    #[test]
    fn decide_just_below_spawn_socrates_threshold() {
        let f = fuser();
        assert_eq!(f.decide(0.29), FusionDecision::Abstain);
    }
}
