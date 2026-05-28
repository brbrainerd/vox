//! Four-dimension risk scorer and HITL escalation matrix (D5 + D9).
//!
//! Scores an action on irreversibility, blast radius, compliance exposure, and
//! confidence deficit, then maps the composite to a `RiskGrade` and `HitlAction`.
//! Thresholds mirror `contracts/orchestration/risk-confidence-matrix.v1.yaml`.
//! All logic is pure: no async, no I/O.

use serde::{Deserialize, Serialize};

/// Four raw risk dimensions in `[0, 1]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskDimensions {
    /// How hard the action is to undo (0 = trivially reversible, 1 = permanent).
    pub irreversibility: f64,
    /// How broadly the action affects other systems / users (0 = isolated, 1 = global).
    pub blast_radius: f64,
    /// Regulatory / compliance exposure (0 = none, 1 = high).
    pub compliance_exposure: f64,
    /// `1 - confidence` — lower confidence means higher risk contribution.
    pub confidence_deficit: f64,
}

impl Default for RiskDimensions {
    fn default() -> Self {
        Self {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence_deficit: 0.25,
        }
    }
}

/// Discrete risk grade derived from composite score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskGrade {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// HITL action mapped from the risk grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitlAction {
    /// Proceed without interrupting the human.
    Proceed,
    /// Log a warning to context and continue, but surface to pilot.
    WarnContext,
    /// Pause and request human approval before continuing.
    Escalate,
    /// Hard block: do not execute the action; require explicit override.
    BlockAndEscalate,
}

impl std::fmt::Display for HitlAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Proceed => write!(f, "proceed"),
            Self::WarnContext => write!(f, "warn-context"),
            Self::Escalate => write!(f, "escalate"),
            Self::BlockAndEscalate => write!(f, "block-and-escalate"),
        }
    }
}

/// Score and grade thresholds. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMatrixConfig {
    /// Composite score ≥ this → Critical (1.0 = never from score alone, use per-dim overrides).
    pub critical_threshold: f64,
    /// Composite score ≥ this → High.
    pub high_threshold: f64,
    /// Composite score ≥ this → Medium.
    pub medium_threshold: f64,
    /// Any single dimension ≥ this → forced Critical regardless of composite.
    pub hard_critical_dimension: f64,
    /// Weights for computing composite: [irreversibility, blast_radius, compliance, confidence_deficit].
    pub weights: [f64; 4],
}

impl Default for RiskMatrixConfig {
    fn default() -> Self {
        Self {
            critical_threshold: 0.75,
            high_threshold: 0.50,
            medium_threshold: 0.25,
            hard_critical_dimension: 0.90,
            weights: [0.35, 0.30, 0.20, 0.15],
        }
    }
}

/// Pure four-dimension risk matrix.
pub struct RiskMatrix {
    config: RiskMatrixConfig,
}

impl RiskMatrix {
    pub fn new(config: RiskMatrixConfig) -> Self {
        Self { config }
    }

    /// Compute composite risk score in `[0, 1]`.
    ///
    /// `score = Σ(dim_i × weight_i)` then clamped.
    #[must_use]
    #[inline]
    pub fn score(&self, dims: &RiskDimensions) -> f64 {
        let w = &self.config.weights;
        (dims.irreversibility * w[0]
            + dims.blast_radius * w[1]
            + dims.compliance_exposure * w[2]
            + dims.confidence_deficit * w[3])
            .clamp(0.0, 1.0)
    }

    /// Map a composite score to a `RiskGrade`, with hard-override if any single dimension
    /// exceeds `hard_critical_dimension`.
    #[must_use]
    #[inline]
    pub fn grade(&self, dims: &RiskDimensions) -> RiskGrade {
        let any_hard_critical = dims.irreversibility >= self.config.hard_critical_dimension
            || dims.blast_radius >= self.config.hard_critical_dimension
            || dims.compliance_exposure >= self.config.hard_critical_dimension
            || dims.confidence_deficit >= self.config.hard_critical_dimension;

        if any_hard_critical {
            return RiskGrade::Critical;
        }

        let s = self.score(dims);
        if s >= self.config.critical_threshold {
            RiskGrade::Critical
        } else if s >= self.config.high_threshold {
            RiskGrade::High
        } else if s >= self.config.medium_threshold {
            RiskGrade::Medium
        } else {
            RiskGrade::Low
        }
    }

    /// Map a `RiskGrade` to the HITL action the orchestrator should take.
    #[must_use]
    #[inline]
    pub fn hitl_action(&self, grade: RiskGrade) -> HitlAction {
        match grade {
            RiskGrade::Low => HitlAction::Proceed,
            RiskGrade::Medium => HitlAction::WarnContext,
            RiskGrade::High => HitlAction::Escalate,
            RiskGrade::Critical => HitlAction::BlockAndEscalate,
        }
    }

    /// Convenience: score → grade → action in one call.
    #[must_use]
    #[inline]
    pub fn evaluate(&self, dims: &RiskDimensions) -> (f64, RiskGrade, HitlAction) {
        let grade = self.grade(dims);
        let action = self.hitl_action(grade);
        (self.score(dims), grade, action)
    }
}

/// Metric payload emitted when a risk score is computed.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct RiskScoreEvent {
    pub metric_type: &'static str,
    pub score: f64,
    pub grade: String,
    pub session_id: Option<String>,
}

impl RiskScoreEvent {
    pub fn new(score: f64, grade: RiskGrade, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_RISK_SCORE,
            score,
            grade: grade.to_string(),
            session_id,
        }
    }
}

/// Metric payload emitted when HITL escalation is triggered.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct HitlInterruptEvent {
    pub metric_type: &'static str,
    pub action: String,
    pub grade: String,
    pub action_description: String,
    pub session_id: Option<String>,
}

impl HitlInterruptEvent {
    pub fn new(
        action: HitlAction,
        grade: RiskGrade,
        action_description: impl Into<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_HITL_INTERRUPT,
            action: action.to_string(),
            grade: grade.to_string(),
            action_description: action_description.into(),
            session_id,
        }
    }
}

/// Merge AgentOS `mutation_kind` signals into raw [`RiskDimensions`] before matrix scoring.
pub fn apply_agentos_mutation_risk(dim: &mut RiskDimensions, mutation_kind: &str) {
    match mutation_kind {
        "external_side_effect" => {
            dim.blast_radius = dim.blast_radius.max(0.55);
            dim.irreversibility = dim.irreversibility.max(0.35);
        }
        "local_mutation" => {
            dim.irreversibility = dim.irreversibility.max(0.28);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> RiskMatrix {
        RiskMatrix::new(RiskMatrixConfig::default())
    }

    #[test]
    fn zero_risk_gives_low_grade_and_proceed() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence_deficit: 0.0,
        };
        let (_, grade, action) = m.evaluate(&dims);
        assert_eq!(grade, RiskGrade::Low);
        assert_eq!(action, HitlAction::Proceed);
    }

    #[test]
    fn high_irreversibility_forces_critical() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.95,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence_deficit: 0.0,
        };
        assert_eq!(m.grade(&dims), RiskGrade::Critical);
        assert_eq!(
            m.hitl_action(RiskGrade::Critical),
            HitlAction::BlockAndEscalate
        );
    }

    #[test]
    fn high_blast_radius_forces_critical() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.91,
            compliance_exposure: 0.0,
            confidence_deficit: 0.0,
        };
        assert_eq!(m.grade(&dims), RiskGrade::Critical);
    }

    #[test]
    fn high_confidence_deficit_forces_critical() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence_deficit: 0.95,
        };
        assert_eq!(m.grade(&dims), RiskGrade::Critical);
    }

    #[test]
    fn high_compliance_forces_critical() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.90,
            confidence_deficit: 0.0,
        };
        assert_eq!(m.grade(&dims), RiskGrade::Critical);
    }

    #[test]
    fn medium_composite_gives_medium_grade_and_warn() {
        let m = matrix();
        // score ≈ 0.3 (medium band 0.25–0.50)
        let dims = RiskDimensions {
            irreversibility: 0.4,
            blast_radius: 0.2,
            compliance_exposure: 0.2,
            confidence_deficit: 0.3,
        };
        let (score, grade, action) = m.evaluate(&dims);
        assert!((0.25..0.50).contains(&score), "score={score}");
        assert_eq!(grade, RiskGrade::Medium);
        assert_eq!(action, HitlAction::WarnContext);
    }

    #[test]
    fn high_composite_gives_high_grade_and_escalate() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 0.7,
            blast_radius: 0.7,
            compliance_exposure: 0.3,
            confidence_deficit: 0.5,
        };
        let (_, grade, action) = m.evaluate(&dims);
        assert_eq!(grade, RiskGrade::High);
        assert_eq!(action, HitlAction::Escalate);
    }

    #[test]
    fn score_clamped_to_one() {
        let m = matrix();
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 1.0,
            compliance_exposure: 1.0,
            confidence_deficit: 1.0,
        };
        assert!(m.score(&dims) <= 1.0);
    }

    #[test]
    fn risk_score_event_has_correct_metric_type() {
        let ev = RiskScoreEvent::new(0.5, RiskGrade::High, None);
        assert_eq!(ev.metric_type, "orch.risk.score");
    }

    #[test]
    fn hitl_interrupt_event_has_correct_metric_type() {
        let ev = HitlInterruptEvent::new(HitlAction::Escalate, RiskGrade::High, "drop table", None);
        assert_eq!(ev.metric_type, "orch.hitl.interrupt");
    }

    #[test]
    fn agentos_external_boosts_blast_radius() {
        let mut d = RiskDimensions::default();
        apply_agentos_mutation_risk(&mut d, "external_side_effect");
        assert!(d.blast_radius >= 0.55);
    }
}
