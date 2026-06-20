//! Inference configuration shared by registry resolution (`registry_model_resolve`).

use serde::{Deserialize, Serialize};

use crate::config::CostPreference;
use crate::attention::ApprovalTier;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Modalities {
    pub vision: bool,
    pub web_search: bool,
    pub structured_output: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    Flash,
    #[default]
    Balanced,
    Premium,
}

impl QualityLevel {
    /// Map quality level to a cost preference for model selection.
    ///
    /// Free-by-default policy: `Flash` and `Balanced` both resolve to `Economy`
    /// so that the two most common quality tiers prefer free/cheap models.
    /// Only `Premium` opts in to `Performance` (paid-model-preferred) routing.
    #[must_use]
    pub fn to_cost_preference(self) -> CostPreference {
        match self {
            Self::Flash | Self::Balanced => CostPreference::Economy,
            Self::Premium => CostPreference::Performance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TierProfile {
    #[default]
    Automatic,
    Manual(String),
    BringYourOwnKey {
        provider: String,
    },
}

// superseded by ClutchProfile; migrate vox-research-shim (scorer.rs:61,178-195) then remove
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionModeProfile {
    Efficient,
    LegacyDefault,
    Fast,
    Verbose,
    Precision,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InferenceConfig {
    pub modalities: Modalities,
    pub quality: QualityLevel,
    pub tier: TierProfile,
    #[serde(default)]
    pub free_only: bool,
}

impl InferenceConfig {
    #[must_use]
    #[inline]
    pub fn is_free_only(&self) -> bool {
        self.free_only
    }
}

// ── Task 1: ClutchProfile ────────────────────────────────────────────────────

/// Budget-gate aggressiveness selected by the clutch. Maps onto the existing
/// downgrade@80%/halt@95% gate (`budget_gate.rs`): `Aggressive` lowers thresholds,
/// `Relaxed` converts halt into a warn (Genius keeps going, with consent surfaced by the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetAggressiveness {
    Aggressive,
    Default,
    Relaxed,
}

/// Resolved control knobs for one clutch detent. Pure data — no I/O.
/// `axes` is the (cost, responsiveness, intelligence) triple consumed by
/// `SelectionAxes` at the scorer candidate boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedClutch {
    pub quality: QualityLevel,
    pub cost_preference: CostPreference,
    pub axes: (u8, u8, u8),
    pub force_free_pool: bool,
    pub always_delegate_free: bool,
    pub delegate_free_when_simple: bool,
    pub budget_gate: BudgetAggressiveness,
}

/// User-facing "how much gas" control. Single SSOT for the four detents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClutchProfile {
    Free,
    #[default]
    Efficiency,
    Balanced,
    Genius,
}

impl ClutchProfile {
    #[must_use]
    pub fn resolve(self) -> ResolvedClutch {
        match self {
            Self::Free => ResolvedClutch {
                quality: QualityLevel::Flash,
                cost_preference: CostPreference::Economy,
                axes: (70, 15, 15),
                force_free_pool: true,
                always_delegate_free: true,
                delegate_free_when_simple: true,
                budget_gate: BudgetAggressiveness::Aggressive,
            },
            Self::Efficiency => ResolvedClutch {
                quality: QualityLevel::Flash,
                cost_preference: CostPreference::Economy,
                axes: (70, 15, 15),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: true,
                budget_gate: BudgetAggressiveness::Default,
            },
            Self::Balanced => ResolvedClutch {
                quality: QualityLevel::Balanced,
                cost_preference: CostPreference::Economy,
                axes: (33, 33, 34),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: false,
                budget_gate: BudgetAggressiveness::Default,
            },
            Self::Genius => ResolvedClutch {
                quality: QualityLevel::Premium,
                cost_preference: CostPreference::Performance,
                axes: (15, 15, 70),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: false,
                budget_gate: BudgetAggressiveness::Relaxed,
            },
        }
    }
}

// ── Task 2: RiskPosture ─────────────────────────────────────────────────────

/// How strongly to gate completion by human/auto approval. Maps onto the existing
/// `ApprovalTier` (`attention/budget.rs`) at the attention gate.
/// `AutoApproveMore→AutoApprove`, `Confirm→Confirm`, `Review→Review` (no `Blocked` here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalLean {
    AutoApproveMore,
    Confirm,
    Review,
}

impl ApprovalLean {
    /// Map risk-posture intent to the canonical `ApprovalTier` for the attention gate.
    #[must_use]
    pub fn to_approval_tier(self) -> ApprovalTier {
        match self {
            Self::AutoApproveMore => ApprovalTier::AutoApprove,
            Self::Confirm => ApprovalTier::Confirm,
            Self::Review => ApprovalTier::Review,
        }
    }
}

/// Whether risk nudges the model choice independent of the clutch. `Intelligence`
/// overrides a cheap clutch pick toward an intelligence-weighted candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelLean {
    Neutral,
    Intelligence,
}

/// Resolved safety gates for one risk posture. Pure data — no I/O.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedRisk {
    pub approval: ApprovalLean,
    pub grounding_enforce: bool,
    pub socrates_enforce: bool,
    pub safety_token_multiplier: f32,
    pub model_lean: ModelLean,
}

/// User-facing acceptable-risk control. Higher risk = break things, spend less on safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskPosture {
    High,
    #[default]
    Moderate,
    Low,
}

impl RiskPosture {
    #[must_use]
    pub fn resolve(self) -> ResolvedRisk {
        match self {
            Self::High => ResolvedRisk {
                approval: ApprovalLean::AutoApproveMore,
                grounding_enforce: false,
                socrates_enforce: false,
                safety_token_multiplier: 1.0,
                model_lean: ModelLean::Neutral,
            },
            Self::Moderate => ResolvedRisk {
                approval: ApprovalLean::Confirm,
                grounding_enforce: true,
                socrates_enforce: false,
                safety_token_multiplier: 1.0,
                model_lean: ModelLean::Neutral,
            },
            Self::Low => ResolvedRisk {
                approval: ApprovalLean::Review,
                grounding_enforce: true,
                socrates_enforce: true,
                safety_token_multiplier: 1.5,
                model_lean: ModelLean::Intelligence,
            },
        }
    }
}

// ── Task 3: effective_axes ──────────────────────────────────────────────────

/// The (cost, responsiveness, intelligence) triple the scorer should use, after risk
/// overrides the clutch. `ModelLean::Intelligence` (Low risk) forces an
/// intelligence-weighted axis regardless of a cheaper clutch detent.
#[must_use]
pub fn effective_axes(clutch: ClutchProfile, risk: RiskPosture) -> (u8, u8, u8) {
    let base = clutch.resolve().axes;
    match risk.resolve().model_lean {
        ModelLean::Intelligence => (15, 15, 70),
        ModelLean::Neutral => base,
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    // Catches: regression that maps Balanced (the default tier) to Performance,
    // silently routing the most common quality tier to paid models and breaking
    // the free-by-default product directive.
    #[test]
    fn to_cost_preference_balanced_and_flash_are_economy_premium_is_performance() {
        assert_eq!(
            QualityLevel::Flash.to_cost_preference(),
            CostPreference::Economy
        );
        assert_eq!(
            QualityLevel::Balanced.to_cost_preference(),
            CostPreference::Economy
        );
        assert_eq!(
            QualityLevel::Premium.to_cost_preference(),
            CostPreference::Performance
        );
    }

    // Catches: is_free_only returning a hardcoded constant instead of reflecting
    // the free_only field, which would let paid models leak past a free-only gate.
    #[test]
    fn is_free_only_reflects_field() {
        let mut cfg = InferenceConfig::default();
        assert!(!cfg.is_free_only()); // default is false
        cfg.free_only = true;
        assert!(cfg.is_free_only());
    }
}

#[cfg(test)]
mod clutch_tests {
    use super::*;
    use crate::config::CostPreference;

    #[test]
    fn clutch_resolves_each_detent() {
        let free = ClutchProfile::Free.resolve();
        assert_eq!(free.quality, QualityLevel::Flash);
        assert_eq!(free.cost_preference, CostPreference::Economy);
        assert!(free.force_free_pool);
        assert!(free.always_delegate_free);
        assert_eq!(free.axes, (70, 15, 15));

        let eff = ClutchProfile::Efficiency.resolve();
        assert_eq!(eff.quality, QualityLevel::Flash);
        assert!(!eff.force_free_pool);
        assert!(!eff.always_delegate_free);
        assert!(eff.delegate_free_when_simple);
        assert_eq!(eff.budget_gate, BudgetAggressiveness::Default);

        let bal = ClutchProfile::Balanced.resolve();
        assert_eq!(bal.quality, QualityLevel::Balanced);
        assert_eq!(bal.axes, (33, 33, 34));

        let genius = ClutchProfile::Genius.resolve();
        assert_eq!(genius.quality, QualityLevel::Premium);
        assert_eq!(genius.cost_preference, CostPreference::Performance);
        assert_eq!(genius.axes, (15, 15, 70));
        assert_eq!(genius.budget_gate, BudgetAggressiveness::Relaxed);
        assert!(!genius.delegate_free_when_simple);
    }
}

#[cfg(test)]
mod risk_tests {
    use super::*;

    #[test]
    fn risk_resolves_each_posture() {
        let high = RiskPosture::High.resolve();
        assert_eq!(high.approval, ApprovalLean::AutoApproveMore);
        assert!(!high.grounding_enforce);
        assert!(!high.socrates_enforce);
        assert_eq!(high.safety_token_multiplier, 1.0);
        assert_eq!(high.model_lean, ModelLean::Neutral);

        let mid = RiskPosture::Moderate.resolve();
        assert_eq!(mid.approval, ApprovalLean::Confirm);
        assert!(mid.grounding_enforce);
        assert!(!mid.socrates_enforce);
        assert_eq!(mid.safety_token_multiplier, 1.0);

        let low = RiskPosture::Low.resolve();
        assert_eq!(low.approval, ApprovalLean::Review);
        assert!(low.grounding_enforce);
        assert!(low.socrates_enforce);
        assert!(low.safety_token_multiplier > 1.0);
        assert_eq!(low.model_lean, ModelLean::Intelligence);
    }

    #[test]
    fn approval_lean_maps_to_approval_tier() {
        use crate::attention::ApprovalTier;
        assert_eq!(ApprovalLean::AutoApproveMore.to_approval_tier(), ApprovalTier::AutoApprove);
        assert_eq!(ApprovalLean::Confirm.to_approval_tier(), ApprovalTier::Confirm);
        assert_eq!(ApprovalLean::Review.to_approval_tier(), ApprovalTier::Review);
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn low_risk_overrides_efficiency_cheap_pick() {
        let axes = effective_axes(ClutchProfile::Efficiency, RiskPosture::Low);
        assert_eq!(axes, (15, 15, 70));
    }

    #[test]
    fn high_risk_keeps_clutch_axes() {
        let axes = effective_axes(ClutchProfile::Efficiency, RiskPosture::High);
        assert_eq!(axes, (70, 15, 15));
    }

    #[test]
    fn genius_already_intelligent_unchanged_by_low_risk() {
        let axes = effective_axes(ClutchProfile::Genius, RiskPosture::Low);
        assert_eq!(axes, (15, 15, 70));
    }
}
