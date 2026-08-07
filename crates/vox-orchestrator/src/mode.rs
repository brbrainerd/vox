//! Inference configuration shared by registry resolution (`registry_model_resolve`).

use serde::{Deserialize, Serialize};

use crate::attention::ApprovalTier;
use crate::config::CostPreference;

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

impl BudgetAggressiveness {
    /// `(downgrade_fraction, halt_fraction)` for the orchestrator budget gate
    /// (`budget_gate.rs`). `Default` mirrors the global gate (0.80 / 0.95);
    /// `Aggressive` trips earlier (cheaper, for Free clutch); `Relaxed` lets
    /// Genius keep going (halt only at full budget).
    #[must_use]
    pub fn thresholds(self) -> (f64, f64) {
        match self {
            Self::Aggressive => (0.70, 0.90),
            Self::Default => (0.80, 0.95),
            Self::Relaxed => (0.90, 1.0),
        }
    }
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
    /// Parse a GUI-supplied label into a profile. Case-insensitive; matches
    /// `lib/driveConsole.ts` `ClutchId` exactly (`free`|`efficiency`|`balanced`|`genius`).
    /// Unknown labels return `None`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "free" => Some(Self::Free),
            "efficiency" => Some(Self::Efficiency),
            "balanced" => Some(Self::Balanced),
            "genius" => Some(Self::Genius),
            _ => None,
        }
    }

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
    /// Parse a GUI-supplied label into a posture. Case-insensitive; matches
    /// `lib/driveConsole.ts` `RiskId` exactly (`high`|`moderate`|`low`).
    /// Unknown labels return `None`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "high" => Some(Self::High),
            "moderate" => Some(Self::Moderate),
            "low" => Some(Self::Low),
            _ => None,
        }
    }

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

// ── Task: TriggerSource ─────────────────────────────────────────────────────

/// Who/what started a task — orthogonal to `TaskCategory` (what kind of work).
/// `Interactive`: a live chat/editor feature. `Automated`: CI/CD, scheduled, or
/// background-poll dispatch. `Subagent`: spawned by another agent. `Mesh`:
/// delivered via A2A from another node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSource {
    #[default]
    Interactive,
    Automated,
    Subagent,
    Mesh,
}

impl TriggerSource {
    /// Parse a hint/GUI-supplied label. Case-insensitive. Unknown labels return `None`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "interactive" => Some(Self::Interactive),
            "automated" => Some(Self::Automated),
            "subagent" => Some(Self::Subagent),
            "mesh" => Some(Self::Mesh),
            _ => None,
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
        assert_eq!(
            ApprovalLean::AutoApproveMore.to_approval_tier(),
            ApprovalTier::AutoApprove
        );
        assert_eq!(
            ApprovalLean::Confirm.to_approval_tier(),
            ApprovalTier::Confirm
        );
        assert_eq!(
            ApprovalLean::Review.to_approval_tier(),
            ApprovalTier::Review
        );
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

#[cfg(test)]
mod trigger_source_tests {
    use super::*;

    #[test]
    fn from_label_parses_all_four_case_insensitive() {
        assert_eq!(
            TriggerSource::from_label("interactive"),
            Some(TriggerSource::Interactive)
        );
        assert_eq!(
            TriggerSource::from_label("Automated"),
            Some(TriggerSource::Automated)
        );
        assert_eq!(
            TriggerSource::from_label("SUBAGENT"),
            Some(TriggerSource::Subagent)
        );
        assert_eq!(TriggerSource::from_label("mesh"), Some(TriggerSource::Mesh));
    }

    #[test]
    fn from_label_unknown_returns_none() {
        assert_eq!(TriggerSource::from_label("turbo"), None);
    }

    #[test]
    fn default_is_interactive() {
        assert_eq!(TriggerSource::default(), TriggerSource::Interactive);
    }
}

// ── Task: per-task-type cost/model policy resolver ──────────────────────────

/// A compiled-in default policy for one `TaskCategory`. The production table
/// (`DEFAULT_CATEGORY_POLICY`) starts empty — seeding real defaults is a
/// separate, low-risk follow-up (editable live via the GUI once this lands)
/// rather than a behavior change bundled into this plan.
#[derive(Debug, Clone, Copy)]
pub struct TaskCategoryPolicy {
    pub category: crate::types::TaskCategory,
    pub clutch: ClutchProfile,
    pub risk: RiskPosture,
}

/// A compiled-in default policy for one `TriggerSource`. Same seeding note as
/// [`TaskCategoryPolicy`].
#[derive(Debug, Clone, Copy)]
pub struct TriggerSourcePolicy {
    pub source: TriggerSource,
    pub clutch: ClutchProfile,
    pub risk: RiskPosture,
}

/// Compiled-in `TaskCategory` defaults. Empty at first landing (see
/// [`TaskCategoryPolicy`] doc) — extend via a dedicated follow-up PR, not by
/// editing this plan's tasks after the fact.
pub const DEFAULT_CATEGORY_POLICY: &[TaskCategoryPolicy] = &[];

/// Compiled-in `TriggerSource` defaults. Empty at first landing (see
/// [`TriggerSourcePolicy`] doc).
pub const DEFAULT_SOURCE_POLICY: &[TriggerSourcePolicy] = &[];

/// Pure precedence resolver: explicit > category policy > source policy > the
/// existing global default (`Balanced`/`Moderate`). Each of the three levels
/// takes clutch and risk as SEPARATE `Option`s (not a paired tuple) so an
/// override that only sets one axis lets the other keep falling through —
/// callers compute the category/source arguments via
/// `effective_category_policy()`/`effective_source_policy()` (added in a
/// later task), which return the same per-axis shape.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn resolve_task_policy(
    explicit_clutch: Option<ClutchProfile>,
    explicit_risk: Option<RiskPosture>,
    category_clutch: Option<ClutchProfile>,
    category_risk: Option<RiskPosture>,
    source_clutch: Option<ClutchProfile>,
    source_risk: Option<RiskPosture>,
) -> (ClutchProfile, RiskPosture) {
    let clutch = explicit_clutch
        .or(category_clutch)
        .or(source_clutch)
        .unwrap_or(ClutchProfile::Balanced);
    let risk = explicit_risk
        .or(category_risk)
        .or(source_risk)
        .unwrap_or(RiskPosture::Moderate);
    (clutch, risk)
}

#[cfg(test)]
mod task_policy_resolver_tests {
    use super::*;

    #[test]
    fn explicit_wins_over_everything() {
        let (clutch, risk) = resolve_task_policy(
            Some(ClutchProfile::Genius),
            Some(RiskPosture::Low),
            Some(ClutchProfile::Free),
            Some(RiskPosture::High),
            Some(ClutchProfile::Efficiency),
            Some(RiskPosture::Moderate),
        );
        assert_eq!(clutch, ClutchProfile::Genius);
        assert_eq!(risk, RiskPosture::Low);
    }

    #[test]
    fn category_policy_wins_over_source_policy() {
        let (clutch, risk) = resolve_task_policy(
            None,
            None,
            Some(ClutchProfile::Balanced),
            Some(RiskPosture::Moderate),
            Some(ClutchProfile::Free),
            Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn source_policy_wins_when_no_category_policy() {
        let (clutch, risk) = resolve_task_policy(
            None,
            None,
            None,
            None,
            Some(ClutchProfile::Free),
            Some(RiskPosture::High),
        );
        assert_eq!(clutch, ClutchProfile::Free);
        assert_eq!(risk, RiskPosture::High);
    }

    #[test]
    fn falls_back_to_global_default_when_nothing_set() {
        let (clutch, risk) = resolve_task_policy(None, None, None, None, None, None);
        assert_eq!(clutch, ClutchProfile::Balanced);
        assert_eq!(risk, RiskPosture::Moderate);
    }

    #[test]
    fn axes_resolve_independently_across_levels_real_case() {
        // A category policy supplies ONLY clutch (its risk axis is None). Risk
        // must fall through past category to source, NOT default straight to
        // Moderate.
        let (clutch, risk) = resolve_task_policy(
            None,
            None,
            Some(ClutchProfile::Efficiency),
            None,
            None,
            Some(RiskPosture::High),
        );
        assert_eq!(
            clutch,
            ClutchProfile::Efficiency,
            "category's clutch axis wins"
        );
        assert_eq!(
            risk,
            RiskPosture::High,
            "category had no risk axis, so source's risk axis is used, not the global default"
        );
    }

    #[test]
    fn explicit_clutch_and_category_risk_combine_across_different_levels() {
        let (clutch, risk) = resolve_task_policy(
            Some(ClutchProfile::Genius),
            None,
            None,
            Some(RiskPosture::Low),
            Some(ClutchProfile::Free),
            Some(RiskPosture::High),
        );
        assert_eq!(
            clutch,
            ClutchProfile::Genius,
            "explicit clutch wins outright"
        );
        assert_eq!(
            risk,
            RiskPosture::Low,
            "explicit risk unset, category risk wins over source risk"
        );
    }
}

/// Shared merge logic for [`effective_category_policy`] and
/// [`effective_source_policy`]: combine an optional live override entry with
/// the matching compiled-default axis values.
///
/// `warn_context` yields `(field_name, key)` for the `tracing::warn!` call
/// and is only invoked (lazily) when a warning is actually needed, so
/// callers don't have to build the key string on every call.
fn merge_policy_entry(
    entry: Option<&crate::config::TaskPolicyEntry>,
    default: Option<(ClutchProfile, RiskPosture)>,
    warn_context: impl FnOnce() -> (&'static str, String),
) -> (Option<ClutchProfile>, Option<RiskPosture>) {
    if let Some(entry) = entry {
        let clutch = entry.clutch.as_deref().and_then(ClutchProfile::from_label);
        let risk = entry.risk.as_deref().and_then(RiskPosture::from_label);
        // Warn per-axis: an entry with one valid label and one typo'd label
        // (e.g. {clutch: "turbo", risk: "high"}) must still surface the bad
        // axis, not just the case where both fail to parse.
        let clutch_failed_to_parse = entry.clutch.is_some() && clutch.is_none();
        let risk_failed_to_parse = entry.risk.is_some() && risk.is_none();
        if clutch_failed_to_parse || risk_failed_to_parse {
            let (field, key) = warn_context();
            tracing::warn!(field, key = %key, clutch = ?entry.clutch, risk = ?entry.risk, "task_policy override has an unparseable clutch/risk label; falling through to compiled default");
        }
        if clutch.is_some() || risk.is_some() {
            return (
                clutch.or_else(|| default.map(|(c, _)| c)),
                risk.or_else(|| default.map(|(_, r)| r)),
            );
        }
    }
    match default {
        Some((c, r)) => (Some(c), Some(r)),
        None => (None, None),
    }
}

/// Merge the live Vox.toml override (if any and parseable) with the compiled
/// `DEFAULT_CATEGORY_POLICY` for one category. Returns each axis
/// INDEPENDENTLY as its own `Option` — an override that sets only `clutch`
/// resolves `(Some(_), None)`, not a forced pair, so `resolve_task_policy`
/// can let the unset axis fall through to the source-level policy. Logs a
/// `tracing::warn!` each time this is called with an entry whose
/// clutch/risk label doesn't parse — acceptable since resolution is not a
/// tight hot loop, but not deduplicated across calls.
#[must_use]
pub fn effective_category_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    category: crate::types::TaskCategory,
) -> (Option<ClutchProfile>, Option<RiskPosture>) {
    let key = format!("{category:?}");
    let entry = overrides.category.get(&key);
    let default = DEFAULT_CATEGORY_POLICY
        .iter()
        .find(|p| p.category == category)
        .map(|p| (p.clutch, p.risk));
    merge_policy_entry(entry, default, || ("category", key))
}

/// Same merge as [`effective_category_policy`], for `TriggerSource`. Logs a
/// `tracing::warn!` each time this is called with an entry whose
/// clutch/risk label doesn't parse — acceptable since resolution is not a
/// tight hot loop, but not deduplicated across calls.
#[must_use]
pub fn effective_source_policy(
    overrides: &crate::config::TaskPolicyOverrides,
    source: TriggerSource,
) -> (Option<ClutchProfile>, Option<RiskPosture>) {
    let key = format!("{source:?}");
    let entry = overrides.source.get(&key);
    let default = DEFAULT_SOURCE_POLICY
        .iter()
        .find(|p| p.source == source)
        .map(|p| (p.clutch, p.risk));
    merge_policy_entry(entry, default, || ("source", key))
}

/// Effective risk policy for `task`: explicit hint > category policy >
/// source policy, returning `None` when nothing applies anywhere — distinct
/// from [`resolve_task_policy`]'s always-concrete `RiskPosture`, which
/// collapses "nothing configured" into the same value as "configured to the
/// default." Callers that need to tell those two cases apart (e.g. to
/// preserve an existing global toggle rather than overriding it with a
/// coincidentally-matching default) use this instead of hand-rolling the
/// same `effective_category_policy`/`effective_source_policy`/`.or()` chain.
/// Shared by `socrates.rs`'s grounding/Socrates enforcement and
/// `attention_fields.rs`'s approval-tier escalation.
#[must_use]
pub fn effective_risk_for_task(
    task: &crate::types::AgentTask,
    overrides: &crate::config::TaskPolicyOverrides,
) -> Option<RiskPosture> {
    let (_, category_risk) = effective_category_policy(overrides, task.task_category);
    let source = task.trigger_source.unwrap_or(TriggerSource::Interactive);
    let (_, source_risk) = effective_source_policy(overrides, source);
    task.risk_posture.or(category_risk).or(source_risk)
}

#[cfg(test)]
mod effective_policy_tests {
    use super::*;
    use crate::config::{TaskPolicyEntry, TaskPolicyOverrides};
    use crate::types::TaskCategory;
    use std::collections::HashMap;

    #[test]
    fn override_wins_over_compiled_default_for_category() {
        let mut category = HashMap::new();
        category.insert(
            "CodeGen".to_string(),
            TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category,
            source: HashMap::new(),
        };
        let (clutch, risk) = effective_category_policy(&overrides, TaskCategory::CodeGen);
        assert_eq!(clutch, Some(ClutchProfile::Free));
        assert_eq!(risk, Some(RiskPosture::High));
    }

    #[test]
    fn missing_category_override_and_no_compiled_default_is_none() {
        let overrides = TaskPolicyOverrides::default();
        assert_eq!(
            effective_category_policy(&overrides, TaskCategory::Research),
            (None, None)
        );
    }

    #[test]
    fn malformed_override_label_falls_through_to_none() {
        let mut source = HashMap::new();
        source.insert(
            "Automated".to_string(),
            TaskPolicyEntry {
                clutch: Some("turbo".to_string()),
                risk: None,
            },
        );
        let overrides = TaskPolicyOverrides {
            category: HashMap::new(),
            source,
        };
        assert_eq!(
            effective_source_policy(&overrides, TriggerSource::Automated),
            (None, None)
        );
    }

    #[test]
    fn one_bad_label_alongside_one_good_label_still_resolves_the_good_axis() {
        // Regression test for `merge_policy_entry`'s warn condition, which
        // used to only fire when BOTH axes failed to parse — silently
        // swallowing a partially-malformed entry with no diagnostic. This
        // test asserts the (already-correct) per-axis return value: the
        // valid `risk` label must still resolve even though `clutch` is a
        // typo, regardless of whether a warning is logged for it.
        let mut source = HashMap::new();
        source.insert(
            "Automated".to_string(),
            TaskPolicyEntry {
                clutch: Some("turbo".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category: HashMap::new(),
            source,
        };
        assert_eq!(
            effective_source_policy(&overrides, TriggerSource::Automated),
            (None, Some(RiskPosture::High)),
            "a typo'd clutch label must not prevent the valid risk label from resolving"
        );
    }

    #[test]
    fn partial_override_sets_one_axis_and_leaves_the_other_none() {
        let mut category = HashMap::new();
        category.insert(
            "Research".to_string(),
            TaskPolicyEntry {
                clutch: Some("genius".to_string()),
                risk: None,
            },
        );
        let overrides = TaskPolicyOverrides {
            category,
            source: HashMap::new(),
        };
        assert_eq!(
            effective_category_policy(&overrides, TaskCategory::Research),
            (Some(ClutchProfile::Genius), None),
            "a clutch-only override must resolve clutch and leave risk as None, not force a paired default"
        );
    }

    #[test]
    fn unknown_category_with_no_override_falls_through_to_none() {
        // The override map has an entry, but under a key that doesn't match
        // the category being looked up ("NotARealCategory" vs "CodeGen"), so
        // `effective_category_policy` sees no entry for this key at all and
        // falls through to the compiled default. This does NOT exercise the
        // "entry present but unparseable label" warn path — this crate has
        // no tracing-capture test helper, so that path isn't asserted here.
        let mut category = HashMap::new();
        category.insert(
            "NotARealCategory".to_string(),
            TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category,
            source: HashMap::new(),
        };
        assert_eq!(
            effective_category_policy(&overrides, TaskCategory::CodeGen),
            (None, None)
        );
    }

    #[test]
    fn effective_risk_for_task_prefers_explicit_over_category_over_source() {
        let mut task = crate::types::AgentTask::new(
            crate::types::TaskId(1),
            "t",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        task.task_category = TaskCategory::Research;
        task.trigger_source = Some(TriggerSource::Automated);
        task.risk_posture = Some(RiskPosture::Low);

        let mut category = HashMap::new();
        category.insert(
            "Research".to_string(),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("high".to_string()),
            },
        );
        let mut source = HashMap::new();
        source.insert(
            "Automated".to_string(),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("moderate".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides { category, source };

        assert_eq!(
            effective_risk_for_task(&task, &overrides),
            Some(RiskPosture::Low),
            "explicit risk_posture must win over both category and source policy"
        );
    }

    #[test]
    fn effective_risk_for_task_returns_none_when_nothing_configured() {
        let task = crate::types::AgentTask::new(
            crate::types::TaskId(1),
            "t",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        let overrides = TaskPolicyOverrides::default();
        assert_eq!(
            effective_risk_for_task(&task, &overrides),
            None,
            "no explicit hint and no category/source policy must return None, not a default"
        );
    }
}
