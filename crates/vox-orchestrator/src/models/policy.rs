//! Ordered model-selection policy chain.
//!
//! A [`SelectionPolicy`] is a user-orderable list of [`SelectionStep`]s. The
//! resolver in [`resolve_policy`] walks the steps **in order** and returns the
//! first step that yields a model. This lets a user express selection as a
//! priority chain mixing characteristic emphasis, specific-model pins, free-tier
//! preference, and fallback conditions — e.g.
//!
//! > "intelligence first, then efficiency; pin model X; but fall back to a free
//! > model when out of tokens."
//!
//! Each step maps onto the **existing** selection machinery in
//! [`super::select`] / [`super::registry`]; the policy layer adds ordering, it
//! does not invent new scoring.
//!
//! ## Backwards compatibility
//!
//! [`SelectionPolicy::default`] produces an **empty** step list. When the policy
//! is empty the resolver returns `None`, signalling callers to fall through to
//! the pre-existing [`super::select::select`] cascade. Absence of a policy
//! therefore changes nothing about today's selection behavior.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use super::select::{SelectionAxes, SelectionIntent, SelectionOutcome, SelectionReason};
use crate::models::{ModelRegistry, ProviderType};

/// Process-global active policy. Installed once at daemon startup from the
/// persisted `selection_policy` user preference (mirrors how the daemon installs
/// `VOX_AUTO_ROUTING_PRIORITY` from the `routing_priority` pref). `None` means
/// "no policy" → callers use the pre-existing cascade unchanged.
static ACTIVE_POLICY: RwLock<Option<SelectionPolicy>> = RwLock::new(None);

/// Install (or clear) the process-global active policy. An empty policy is
/// stored as `None` so the resolver short-circuits to the existing cascade.
pub fn install_active_policy(policy: Option<SelectionPolicy>) {
    let normalized = policy.filter(|p| !p.is_empty());
    if let Ok(mut guard) = ACTIVE_POLICY.write() {
        *guard = normalized;
    }
}

/// Return a clone of the process-global active policy, if any.
#[must_use]
pub fn active_policy() -> Option<SelectionPolicy> {
    ACTIVE_POLICY.read().ok().and_then(|g| g.clone())
}

/// Which user-facing axis an [`SelectionStep::EmphasizeAxis`] step emphasizes.
///
/// Reuses the 3-axis vocabulary of [`SelectionAxes`]
/// (cost / responsiveness / intelligence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionAxisKind {
    /// Maps to [`SelectionAxes::intelligence`] (model capability / precision).
    Intelligence,
    /// Maps to [`SelectionAxes::cost`] (cheaper-is-better).
    Efficiency,
    /// Maps to [`SelectionAxes::responsiveness`] (latency).
    Responsiveness,
}

/// Condition guarding a [`SelectionStep::FallbackWhen`] branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackCondition {
    /// The token / spend budget is exhausted. Evaluated via
    /// [`PolicyContext::budget_exhausted`] (fed from the tier-cascade
    /// `CompositeSignal.budget_exhausted` or the
    /// `VOX_EXPLORATION_BUDGET_EXHAUSTED` env gate).
    OutOfTokens,
    /// The prior step's chosen model would cost more than this many USD per
    /// (typical ~1k-token) call. Evaluated via the existing max-cost filter.
    CostAboveUsdPerCall(f64),
    /// The prior step returned no candidate.
    NoCandidate,
    /// A required provider is unavailable (e.g. `RemainingBudget` hint says the
    /// paid provider is out of quota). Evaluated via
    /// [`PolicyContext::provider_unavailable`].
    ProviderUnavailable,
}

/// One step in an ordered [`SelectionPolicy`] chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStep {
    /// Emphasize one axis with the given weight (0-100). Multiple
    /// `EmphasizeAxis` steps **compose in priority order** — the first one seen
    /// keeps its full weight, later ones are damped so earlier = higher
    /// priority. After accumulating, the existing scorer path runs.
    EmphasizeAxis {
        axis: SelectionAxisKind,
        weight: u8,
    },
    /// Pin a specific model by id. Selected iff present + eligible in the
    /// registry; otherwise the step yields nothing and the chain continues.
    PinModel(String),
    /// Prefer a free-tier model via the live registry free selectors. Yields
    /// nothing (chain continues) if no free model is eligible.
    PreferFree,
    /// Evaluate `then` only when `condition` holds for the current context /
    /// prior-step result.
    FallbackWhen {
        condition: FallbackCondition,
        then: Box<SelectionStep>,
    },
}

/// An ordered priority chain of [`SelectionStep`]s.
///
/// [`Default`] is an **empty** chain — see the module docs: empty means "use the
/// pre-existing cascade", so default behavior is unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SelectionPolicy {
    pub steps: Vec<SelectionStep>,
}

impl SelectionPolicy {
    /// True if this policy carries no steps (the resolver should defer to the
    /// pre-existing [`super::select::select`] cascade).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Parse a [`SelectionPolicy`] from a JSON string (the persisted form).
    ///
    /// # Errors
    /// Returns the serde error if the JSON does not match the schema.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Serialize to the persisted JSON form.
    ///
    /// # Errors
    /// Returns the serde error if serialization fails (practically never).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Build a [`SelectionPolicy`] that realizes a [`FreeRoutingProfile`].
///
/// - [`Free`](FreeRoutingProfile::Free): `[PreferFree]` — free selectors only.
/// - [`Mixed`](FreeRoutingProfile::Mixed): `[PreferFree, FallbackWhen{NoCandidate,
///   <scorer/paid>}]` — free-preferred, paid fallback. (The paid fallback is the
///   empty chain's pre-existing cascade: an empty `then` is represented by
///   omitting the step, so Mixed = `[PreferFree]` followed by chain fall-through.)
/// - [`Performance`](FreeRoutingProfile::Performance): empty — pre-existing paid path.
/// - [`Local`](FreeRoutingProfile::Local): `[PreferFree]` evaluated under a
///   `prefer_local` intent restricts to local providers.
#[must_use]
pub fn policy_for_profile(profile: crate::types::FreeRoutingProfile) -> SelectionPolicy {
    use crate::types::FreeRoutingProfile as P;
    match profile {
        // Free / Local: free selectors only. (Local additionally relies on the
        // caller's `prefer_local` intent, which `select_free` honors.)
        P::Free | P::Local => SelectionPolicy {
            steps: vec![SelectionStep::PreferFree],
        },
        // Mixed: prefer free, then fall through to the pre-existing cascade
        // (paid path). PreferFree yielding nothing falls through automatically,
        // so a single PreferFree step + chain fall-through gives free-then-paid.
        P::Mixed => SelectionPolicy {
            steps: vec![SelectionStep::PreferFree],
        },
        // Performance: empty → pre-existing paid cascade unchanged.
        P::Performance => SelectionPolicy::default(),
    }
}

/// Runtime signals the resolver consults when evaluating
/// [`FallbackCondition`]s. Defaults read deterministic env gates so tests and
/// the daemon can drive them explicitly.
#[derive(Debug, Clone, Copy, Default)]
pub struct PolicyContext {
    /// True when the token/spend budget is exhausted (tier-cascade signal).
    pub budget_exhausted: bool,
    /// True when a required paid provider is unavailable.
    pub provider_unavailable: bool,
}

impl PolicyContext {
    /// Build a context from deterministic env gates. Mirrors the
    /// `VOX_EXPLORATION_BUDGET_EXHAUSTED` gate already honored in
    /// [`super::select`].
    #[must_use]
    pub fn from_env() -> Self {
        let truthy = |k: &str| {
            std::env::var(k)
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        };
        Self {
            budget_exhausted: truthy("VOX_EXPLORATION_BUDGET_EXHAUSTED"),
            provider_unavailable: truthy("VOX_PROVIDER_UNAVAILABLE"),
        }
    }
}

/// Walk `policy.steps` in order and return the first step that yields a model.
///
/// Returns `None` when the policy is empty (caller should fall through to the
/// pre-existing cascade) or when no step yields an eligible model.
#[must_use]
pub fn resolve_policy(
    policy: &SelectionPolicy,
    intent: &SelectionIntent,
    registry: &ModelRegistry,
    ctx: &PolicyContext,
) -> Option<SelectionOutcome> {
    if policy.is_empty() {
        return None;
    }

    // Accumulated axis emphasis. Starts from the caller's intent axes so a chain
    // that only pins/free-prefers still respects the caller's base shape.
    let mut axes = intent.axes;
    let mut emphasis_seen = 0u32;
    // Track whether the *immediately preceding* step produced a candidate, for
    // `FallbackCondition::NoCandidate`.
    let mut prior_yielded = true;

    for step in &policy.steps {
        let outcome = eval_step(
            step,
            intent,
            registry,
            ctx,
            &mut axes,
            &mut emphasis_seen,
            prior_yielded,
        );
        match outcome {
            StepResult::Selected(o) => return Some(o),
            StepResult::Yielded(o) => return Some(o),
            StepResult::EmphasisAccumulated => {
                // Emphasis steps don't select on their own unless they're the
                // last meaningful step; we run the scorer eagerly so the FIRST
                // emphasis step already produces a concrete model, matching
                // "emphasize intelligence first" semantics.
                if let Some(o) = run_scorer_with_axes(intent, registry, axes) {
                    return Some(o);
                }
                prior_yielded = false;
            }
            StepResult::Nothing => {
                prior_yielded = false;
            }
        }
    }
    None
}

enum StepResult {
    /// A pin / fallback produced a concrete model.
    Selected(SelectionOutcome),
    /// A free / scorer step produced a concrete model.
    Yielded(SelectionOutcome),
    /// An emphasis step folded its weight into the accumulator.
    EmphasisAccumulated,
    /// The step produced no model; continue the chain.
    Nothing,
}

#[allow(clippy::too_many_arguments)]
fn eval_step(
    step: &SelectionStep,
    intent: &SelectionIntent,
    registry: &ModelRegistry,
    ctx: &PolicyContext,
    axes: &mut SelectionAxes,
    emphasis_seen: &mut u32,
    prior_yielded: bool,
) -> StepResult {
    match step {
        SelectionStep::EmphasizeAxis { axis, weight } => {
            apply_emphasis(axes, *axis, *weight, *emphasis_seen);
            *emphasis_seen += 1;
            StepResult::EmphasisAccumulated
        }
        SelectionStep::PinModel(id) => match select_pinned(id, intent, registry) {
            Some(o) => StepResult::Selected(o),
            None => StepResult::Nothing,
        },
        SelectionStep::PreferFree => match select_free(intent, registry) {
            Some(o) => StepResult::Yielded(o),
            None => StepResult::Nothing,
        },
        SelectionStep::FallbackWhen { condition, then } => {
            if condition_holds(condition, intent, registry, ctx, prior_yielded) {
                eval_step(
                    then,
                    intent,
                    registry,
                    ctx,
                    axes,
                    emphasis_seen,
                    prior_yielded,
                )
            } else {
                StepResult::Nothing
            }
        }
    }
}

/// Fold an axis emphasis into the running [`SelectionAxes`]. Earlier emphasis
/// steps (lower `seen`) keep more of their weight; later ones are damped so
/// ordering is priority-preserving.
fn apply_emphasis(axes: &mut SelectionAxes, axis: SelectionAxisKind, weight: u8, seen: u32) {
    // Priority damping: first emphasis full weight, each later one halved.
    let damp = 1u32 << seen.min(7); // 1, 2, 4, ...
    let effective = (u32::from(weight) / damp) as u8;
    let val = effective.max(if seen == 0 { weight } else { 1 });
    match axis {
        SelectionAxisKind::Intelligence => axes.intelligence = val,
        SelectionAxisKind::Efficiency => axes.cost = val,
        SelectionAxisKind::Responsiveness => axes.responsiveness = val,
    }
}

/// Run the existing scorer path with an overridden [`SelectionAxes`].
fn run_scorer_with_axes(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
    axes: SelectionAxes,
) -> Option<SelectionOutcome> {
    let mut shaped = intent.clone();
    shaped.axes = axes;
    super::select::select_via_scorer_public(&shaped, registry)
}

/// Return the pinned model iff present + eligible (intent constraints + routing
/// confidence). Falls through (None) otherwise.
fn select_pinned(
    id: &str,
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let model = registry.get(id)?;
    if !super::select::supports_intent_constraints_public(&model, intent) {
        return None;
    }
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        effective_axes: intent.axes.to_routing_priority(intent.prefer_local),
        model_spec: model,
        reason: SelectionReason::EnvOverride {
            env_var: "selection_policy:pin",
        },
    })
}

/// Select a free model via the LIVE registry free selectors. Mirrors the
/// Economy-path ordering: best-for-task → cheapest-free.
fn select_free(intent: &SelectionIntent, registry: &ModelRegistry) -> Option<SelectionOutcome> {
    let want_local = intent.prefer_local;
    let pred = move |m: &crate::models::ModelSpec| {
        if want_local {
            matches!(
                m.provider_type,
                ProviderType::Ollama | ProviderType::VoxLocal | ProviderType::PopuliMesh
            )
        } else {
            true
        }
    };
    let model = registry
        .best_free_for_with_filter(intent.task, pred)
        .or_else(|| registry.cheapest_free_with_filter(pred))?;
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        effective_axes: intent.axes.to_routing_priority(intent.prefer_local),
        model_spec: model,
        reason: SelectionReason::Scored,
    })
}

fn condition_holds(
    condition: &FallbackCondition,
    intent: &SelectionIntent,
    registry: &ModelRegistry,
    ctx: &PolicyContext,
    prior_yielded: bool,
) -> bool {
    match condition {
        FallbackCondition::OutOfTokens => ctx.budget_exhausted,
        FallbackCondition::ProviderUnavailable => ctx.provider_unavailable,
        FallbackCondition::NoCandidate => !prior_yielded,
        FallbackCondition::CostAboveUsdPerCall(ceiling) => {
            // Holds when the scorer's pick under the current intent would exceed
            // the ceiling (i.e. there is no eligible model under it). Reuse the
            // existing max-cost filter via a shaped intent.
            let mut shaped = intent.clone();
            shaped.max_cost_usd_per_call = Some(*ceiling);
            super::select::select_via_scorer_public(&shaped, registry).is_none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::generated::StrengthTag;
    use crate::models::spec::PricingSource;
    use crate::models::{ModelRegistry, ModelSpec, TaskCategory};

    /// Premium paid model: high quality, expensive, not free.
    fn premium() -> ModelSpec {
        ModelSpec {
            id: "premium-paid".into(),
            canonical_slug: "premium-paid".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 200_000,
            cost_per_1k: 0.015,
            cost_per_1k_input: 0.003,
            cost_per_1k_output: 0.015,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![StrengthTag::Generalist, StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::UserConfig,
            supported_parameters: vec![],
        }
    }

    /// Cheap free model.
    fn free_cheap() -> ModelSpec {
        ModelSpec {
            id: "free-cheap".into(),
            canonical_slug: "free-cheap".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 32_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![StrengthTag::Generalist, StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::UserConfig,
            supported_parameters: vec![],
        }
    }

    fn fixture() -> ModelRegistry {
        let mut r = ModelRegistry::default();
        r.register(premium());
        r.register(free_cheap());
        r
    }

    fn intent() -> SelectionIntent {
        SelectionIntent::for_task(TaskCategory::CodeGen)
    }

    fn ctx() -> PolicyContext {
        PolicyContext::default()
    }

    #[test]
    fn pin_model_selects_that_model() {
        let r = fixture();
        let policy = SelectionPolicy {
            steps: vec![SelectionStep::PinModel("premium-paid".into())],
        };
        let o = resolve_policy(&policy, &intent(), &r, &ctx()).expect("pin selects");
        assert_eq!(o.model_id, "premium-paid");
    }

    #[test]
    fn pin_missing_model_yields_nothing() {
        let r = fixture();
        let policy = SelectionPolicy {
            steps: vec![SelectionStep::PinModel("does-not-exist".into())],
        };
        assert!(resolve_policy(&policy, &intent(), &r, &ctx()).is_none());
    }

    #[test]
    fn prefer_free_selects_free_over_premium() {
        let r = fixture();
        let policy = SelectionPolicy {
            steps: vec![SelectionStep::PreferFree],
        };
        let o = resolve_policy(&policy, &intent(), &r, &ctx()).expect("free selected");
        assert_eq!(o.model_id, "free-cheap");
    }

    #[test]
    fn emphasize_intelligence_vs_efficiency_differ() {
        let r = fixture();
        let intel = SelectionPolicy {
            steps: vec![SelectionStep::EmphasizeAxis {
                axis: SelectionAxisKind::Intelligence,
                weight: 90,
            }],
        };
        let eff = SelectionPolicy {
            steps: vec![SelectionStep::EmphasizeAxis {
                axis: SelectionAxisKind::Efficiency,
                weight: 90,
            }],
        };
        let a = resolve_policy(&intel, &intent(), &r, &ctx()).expect("intel selects");
        let b = resolve_policy(&eff, &intent(), &r, &ctx()).expect("eff selects");
        // Intelligence-first should prefer the premium model; efficiency-first
        // should prefer the free/cheap one. They must differ.
        assert_ne!(
            a.model_id, b.model_id,
            "intelligence vs efficiency emphasis selected the same model: {}",
            a.model_id
        );
        assert_eq!(a.model_id, "premium-paid");
        assert_eq!(b.model_id, "free-cheap");
    }

    #[test]
    fn fallback_out_of_tokens_chooses_fallback_only_when_exhausted() {
        let r = fixture();
        // primary pins premium; fallback (on OutOfTokens) pins the free model.
        let policy = SelectionPolicy {
            steps: vec![
                SelectionStep::FallbackWhen {
                    condition: FallbackCondition::OutOfTokens,
                    then: Box::new(SelectionStep::PinModel("free-cheap".into())),
                },
                SelectionStep::PinModel("premium-paid".into()),
            ],
        };
        // Not exhausted: the fallback's condition is false → falls through to
        // the premium pin.
        let normal = resolve_policy(&policy, &intent(), &r, &ctx()).expect("primary");
        assert_eq!(normal.model_id, "premium-paid");

        // Exhausted: the fallback fires first and pins the free model.
        let exhausted_ctx = PolicyContext {
            budget_exhausted: true,
            provider_unavailable: false,
        };
        let fb = resolve_policy(&policy, &intent(), &r, &exhausted_ctx).expect("fallback");
        assert_eq!(fb.model_id, "free-cheap");
    }

    #[test]
    fn empty_policy_yields_none_so_caller_uses_existing_cascade() {
        let r = fixture();
        let policy = SelectionPolicy::default();
        assert!(policy.is_empty());
        assert!(resolve_policy(&policy, &intent(), &r, &ctx()).is_none());
    }

    #[test]
    fn default_policy_select_with_policy_matches_pre_existing_select() {
        let r = fixture();
        let policy = SelectionPolicy::default();
        // select_with_policy with an empty policy must reproduce plain select().
        let baseline = super::super::select::select(&intent(), &r);
        let via_policy =
            super::super::select::select_with_policy(&intent(), &r, &policy, &ctx());
        assert_eq!(
            baseline.map(|o| o.model_id),
            via_policy.map(|o| o.model_id),
            "empty policy regressed the pre-existing decide()/select() result"
        );
    }

    #[test]
    fn policy_json_round_trips() {
        let policy = SelectionPolicy {
            steps: vec![
                SelectionStep::EmphasizeAxis {
                    axis: SelectionAxisKind::Intelligence,
                    weight: 90,
                },
                SelectionStep::PinModel("x".into()),
                SelectionStep::PreferFree,
                SelectionStep::FallbackWhen {
                    condition: FallbackCondition::OutOfTokens,
                    then: Box::new(SelectionStep::PreferFree),
                },
            ],
        };
        let json = policy.to_json().unwrap();
        let back = SelectionPolicy::from_json(&json).unwrap();
        assert_eq!(policy, back);
    }
}
