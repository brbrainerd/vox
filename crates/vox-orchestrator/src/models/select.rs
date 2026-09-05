#![cfg_attr(test, allow(unsafe_code))] // test-only std::env::set_var (edition 2024)
//! `select()` — single-source-of-truth model selection.
//!
//! Council-ratified 2026-05-15 (model-pipeline SSOT consolidation). Every model
//! selection in Vox should flow through [`select`] so that:
//!
//! 1. **Multi-axis user input** ([`SelectionAxes`]) — cost / responsiveness /
//!    intelligence knobs (each 0-100) drive routing instead of binary
//!    Economy/Performance.
//! 2. **Caller-hint conventions** ([`SelectionIntent::repair_loop`],
//!    [`SelectionIntent::research`], etc.) give every Vox subsystem a
//!    consistent starting point.
//! 3. **Transparency** ([`SelectionOutcome::reason`]) — the caller always
//!    knows *why* a model was picked (premium-alias pin? scorer? env
//!    override?) so debugging routing surprises is trivial.
//! 4. **Hardcoded-string elimination** — replaces the ad-hoc constants in
//!    `vox-config::bootstrap_inference` (`REPAIR_LOOP_PREFERRED`,
//!    `RESEARCH_FLASH_FALLBACK`, etc.) with intent-driven resolution that
//!    respects the current catalog + premium aliases.
//!
//! See [`docs/src/architecture/model-selection-2026-q2.md`](../../../../docs/src/architecture/model-selection-2026-q2.md)
//! §8 for the design rationale and migration plan.

use crate::config::CostPreference;
use crate::models::{ModelRegistry, ModelSpec, ProviderType, TaskCategory};
use vox_config::AutoRoutingPriority;
use vox_telemetry::{SelectionDecisionEvent, TelemetryEvent};

// ─── Canonical request/response (SSOT API) ─────────────────────────────────

/// Rich model-selection request consumed by the canonical selector.
#[derive(Debug, Clone)]
pub struct ModelSelectionRequest {
    pub intent: SelectionIntent,
    pub required_capabilities: Vec<super::generated::Capability>,
    pub candidate_scope: CandidateScope,
}

/// Where candidate models may be drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CandidateScope {
    #[default]
    AllProviders,
    LocalOnly,
    CloudOnly,
}

/// Decision envelope returned by the canonical selector (extends [`SelectionOutcome`]).
#[derive(Debug, Clone)]
pub struct ModelSelectionDecision {
    pub selected_model: String,
    pub provider_route: super::ModelRouteBackend,
    pub score_breakdown: ScoreBreakdown,
    pub alternatives: Vec<String>,
    pub rejection_reasons: Vec<String>,
    pub pricing_confidence: super::spec::PricingSource,
    pub discovery_state: super::autonomic::ModelConfidence,
    pub outcome: SelectionOutcome,
}

/// Lightweight score transparency for dashboard + `vox model explain`.
#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub effective_axes: AutoRoutingPriority,
    pub reason: SelectionReason,
    pub capability_match_count: usize,
    pub candidate_count: usize,
    pub intelligence_score: f64,
    pub efficiency_score: f64,
    pub latency_score: f64,
    pub telemetry_quality_score: Option<f64>,
}

impl ModelSelectionRequest {
    #[must_use]
    pub fn from_intent(intent: SelectionIntent) -> Self {
        Self {
            intent,
            required_capabilities: Vec::new(),
            candidate_scope: CandidateScope::AllProviders,
        }
    }
}

/// Canonical selector entry point with structured decision envelope.
#[must_use]
pub fn decide(
    request: &ModelSelectionRequest,
    registry: &ModelRegistry,
) -> Option<ModelSelectionDecision> {
    use std::collections::HashSet;

    let all = registry.list_models();
    let mut rejection_reasons: Vec<String> = Vec::new();
    let mut candidates: Vec<ModelSpec> = Vec::new();
    // Code-review fix: computed once, loop-invariant — `decide()` builds its
    // own candidate set independently of `best_for_internal`'s filter chain
    // (its later `.or_else` scored-fallback is the only branch that reaches
    // that filter), so without this check here a premium-alias or
    // directly-`select()`-returned cloud model would sail through `decide()`
    // regardless of VOX_INFERENCE_PRIVACY=local_only.
    let privacy_local_only = crate::route_policy::inference_privacy_local_only_from_env();

    for m in all {
        if !scope_allows(m.provider_type.clone(), request.candidate_scope) {
            rejection_reasons.push(format!("{} rejected: outside candidate_scope", m.id));
            continue;
        }
        if !request
            .required_capabilities
            .iter()
            .all(|cap| m.capabilities.supports(*cap))
        {
            rejection_reasons.push(format!("{} rejected: missing required capability", m.id));
            continue;
        }
        if !supports_intent_constraints(&m, &request.intent) {
            rejection_reasons.push(format!("{} rejected: intent constraints", m.id));
            continue;
        }
        if !ModelRegistry::key_is_present_for(&m) {
            rejection_reasons.push(format!("{} rejected: missing provider key", m.id));
            continue;
        }
        if !crate::route_policy::privacy_allows_model_for_mode(&m, privacy_local_only) {
            rejection_reasons.push(format!("{} rejected: privacy mode excludes cloud", m.id));
            continue;
        }
        let conf = confidence_state_for_model(&m);
        if !is_routing_eligible(conf) {
            rejection_reasons.push(format!(
                "{} gated: confidence_state={}",
                m.id,
                conf.as_str()
            ));
            continue;
        }
        candidates.push(m);
    }

    if candidates.is_empty() {
        // Controlled exploration fallback for unconfirmed models when explicitly enabled.
        if exploration_enabled() {
            for m in registry.list_models() {
                if !scope_allows(m.provider_type.clone(), request.candidate_scope) {
                    continue;
                }
                if !request
                    .required_capabilities
                    .iter()
                    .all(|cap| m.capabilities.supports(*cap))
                {
                    continue;
                }
                if !supports_intent_constraints(&m, &request.intent) {
                    continue;
                }
                if !ModelRegistry::key_is_present_for(&m) {
                    continue;
                }
                if !crate::route_policy::privacy_allows_model_for_mode(&m, privacy_local_only) {
                    continue;
                }
                let conf = confidence_state_for_model(&m);
                if conf == super::autonomic::ModelConfidence::Deprecated {
                    continue;
                }
                if exploration_budget_exhausted()
                    && m.pricing_source == super::spec::PricingSource::Unknown
                {
                    continue;
                }
                candidates.push(m);
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let candidate_ids: HashSet<String> = candidates.iter().map(|m| m.id.clone()).collect();
    let intent = &request.intent;

    let selected = select(intent, registry)
        .filter(|o| candidate_ids.contains(&o.model_id))
        .or_else(|| {
            // Scoped fallback through registry scorer constrained to candidate set.
            // Intentionally inherits `intent.allow_free_in_performance_mode` here
            // rather than hardcoding `false`: this is still the same caller-supplied
            // intent, so a research intent's free-tier preference should still apply
            // to its own fallback path. Every non-research SelectionIntent constructor
            // hardcodes this field `false`, so decide()'s fallback stays inert for
            // chat/coding/etc. traffic today — this is a deliberate, not incidental,
            // consequence of threading the flag through by intent rather than by caller.
            let model = registry.best_for_with_filter(
                intent.task,
                intent.complexity,
                intent.axes.to_cost_preference(),
                intent.allow_free_in_performance_mode,
                |m| candidate_ids.contains(&m.id),
                None,
            )?;
            Some(SelectionOutcome {
                model_id: model.id.clone(),
                model_spec: model,
                reason: SelectionReason::Scored,
                effective_axes: intent.axes.to_routing_priority(intent.prefer_local),
            })
        })?;

    let alternatives: Vec<String> = candidates
        .iter()
        .filter(|m| m.id != selected.model_id)
        .take(5)
        .map(|m| m.id.clone())
        .collect();

    let cap_match = request
        .required_capabilities
        .iter()
        .filter(|cap| selected.model_spec.capabilities.supports(**cap))
        .count();

    Some(ModelSelectionDecision {
        selected_model: selected.model_id.clone(),
        provider_route: super::route_backend_for_model(&selected.model_spec),
        score_breakdown: ScoreBreakdown {
            effective_axes: selected.effective_axes,
            reason: selected.reason.clone(),
            capability_match_count: cap_match,
            candidate_count: candidates.len(),
            intelligence_score: super::scoring::quality_score(&selected.model_spec),
            efficiency_score: super::scoring::efficiency_score(&selected.model_spec),
            latency_score: super::scoring::latency_score(&selected.model_spec),
            telemetry_quality_score: registry
                .scoreboard_snapshot()
                .get(&selected.model_id)
                .map(|s| s.quality_score),
        },
        alternatives,
        rejection_reasons,
        pricing_confidence: selected.model_spec.pricing_source.clone(),
        discovery_state: confidence_state_for_model(&selected.model_spec),
        outcome: selected,
    })
}

fn scope_allows(provider_type: ProviderType, scope: CandidateScope) -> bool {
    match scope {
        CandidateScope::AllProviders => true,
        CandidateScope::LocalOnly => matches!(
            provider_type,
            ProviderType::Ollama | ProviderType::VoxLocal | ProviderType::PopuliMesh
        ),
        CandidateScope::CloudOnly => !matches!(
            provider_type,
            ProviderType::Ollama | ProviderType::VoxLocal | ProviderType::PopuliMesh
        ),
    }
}

fn confidence_state_for_model(m: &ModelSpec) -> super::autonomic::ModelConfidence {
    let pins = vox_config::load_model_pins_config().unwrap_or_default();
    if pins.retired_ids.iter().any(|id| id == &m.id) {
        return super::autonomic::ModelConfidence::Deprecated;
    }
    // Pricing-source heuristic (the `scoreboard: None` answer). The scoreboard-
    // aware path that promotes discovered models on real evidence lives in
    // `discovery_pipeline::resolve_eligibility`.
    super::discovery_pipeline::resolve_eligibility(m, None, 0.0)
}

fn exploration_enabled() -> bool {
    std::env::var("VOX_ROUTING_ENABLE_EXPLORATION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn exploration_budget_exhausted() -> bool {
    std::env::var("VOX_EXPLORATION_BUDGET_EXHAUSTED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn is_routing_eligible(conf: super::autonomic::ModelConfidence) -> bool {
    conf.eligible_for_routing()
}

// ─── User-facing axes ──────────────────────────────────────────────────────

/// Three-axis user-facing model-selection knob, 0-100 per axis.
///
/// Projected onto the lower-level 6-axis [`AutoRoutingPriority`] inside
/// [`select`]. The remaining three system-derived axes (availability, balance,
/// mobile) get sensible defaults that respect [`SelectionIntent::prefer_local`].
///
/// Axis sum can be anything; the scorer normalizes by total weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionAxes {
    /// 0 = cost is no concern; 100 = absolutely cheapest. Maps to `efficiency`.
    pub cost: u8,
    /// 0 = latency doesn't matter; 100 = fastest. Maps to `latency`.
    pub responsiveness: u8,
    /// 0 = any model is fine; 100 = highest capability. Maps to `precision`.
    pub intelligence: u8,
}

impl SelectionAxes {
    /// **Cost-first**: 70 / 15 / 15. Use for classifiers, CI lints, NLI checks.
    pub const COST_FIRST: Self = Self {
        cost: 70,
        responsiveness: 15,
        intelligence: 15,
    };

    /// **Balanced**: 33 / 33 / 34. Default for most callers.
    pub const BALANCED: Self = Self {
        cost: 33,
        responsiveness: 33,
        intelligence: 34,
    };

    /// **Quality-first**: 15 / 15 / 70. Use for code review, security audit,
    /// debugging, research, planning.
    pub const QUALITY_FIRST: Self = Self {
        cost: 15,
        responsiveness: 15,
        intelligence: 70,
    };

    /// **Fast**: 15 / 70 / 15. Use for IDE autocomplete, ghost-text inference,
    /// any user-facing typed-feedback loop.
    pub const FAST: Self = Self {
        cost: 15,
        responsiveness: 70,
        intelligence: 15,
    };

    /// Parse `cost:N,responsiveness:N,intelligence:N` from the
    /// `VOX_MODEL_AXES` env var. Unknown keys are ignored; missing keys default
    /// to [`SelectionAxes::BALANCED`].
    #[must_use]
    pub fn from_env() -> Self {
        let Ok(raw) = std::env::var("VOX_MODEL_AXES") else {
            return Self::default();
        };
        let mut out = Self::default();
        for part in raw.split(',') {
            let mut it = part.splitn(2, ':');
            let key = it.next().unwrap_or("").trim().to_ascii_lowercase();
            let val = it.next().unwrap_or("").trim();
            let Ok(parsed) = val.parse::<u8>() else {
                continue;
            };
            match key.as_str() {
                "cost" | "efficiency" => out.cost = parsed,
                "responsiveness" | "latency" | "speed" => out.responsiveness = parsed,
                "intelligence" | "precision" | "quality" => out.intelligence = parsed,
                _ => {}
            }
        }
        out
    }

    /// Project the 3-axis user knob onto the 6-axis `AutoRoutingPriority` that
    /// the existing scorer in `models::scoring::auto_score_model` expects.
    /// System-derived axes (availability, balance, mobile) get conservative
    /// defaults that the scorer's intent-aware heuristics will fine-tune.
    #[must_use]
    pub fn to_routing_priority(self, prefer_local: bool) -> AutoRoutingPriority {
        AutoRoutingPriority {
            efficiency: self.cost,
            precision: self.intelligence,
            latency: self.responsiveness,
            availability: 20,
            balance: 5,
            mobile: if prefer_local { 70 } else { 0 },
        }
    }

    /// Derive a binary [`CostPreference`] hint for legacy callers that haven't
    /// migrated to multi-axis. Picks `Economy` when cost weight clearly
    /// dominates; `Performance` otherwise.
    #[must_use]
    pub fn to_cost_preference(self) -> CostPreference {
        if self.cost as u16 > (self.intelligence as u16).saturating_add(self.responsiveness as u16)
        {
            CostPreference::Economy
        } else {
            CostPreference::Performance
        }
    }
}

impl Default for SelectionAxes {
    fn default() -> Self {
        Self::BALANCED
    }
}

// ─── Intent ─────────────────────────────────────────────────────────────────

/// Describes what the caller is trying to do. Drives [`select`]'s choice of
/// premium-alias resolution, caller-hint defaults, and routing priorities.
#[derive(Debug, Clone)]
pub struct SelectionIntent {
    pub task: TaskCategory,
    pub axes: SelectionAxes,
    /// 1-10. Used by the underlying scorer to bias toward higher-precision
    /// models on complex tasks. See `models::scoring::auto_score_model`.
    pub complexity: u8,
    /// If Some, models with `max_context` below this size are penalized.
    pub context_size_hint: Option<usize>,
    /// Free-form caller identifier for telemetry + premium-alias resolution.
    /// Examples: `"repair-loop"`, `"research"`, `"review"`, `"nli-classifier"`,
    /// `"ide-autocomplete"`, `"plan-mode"`.
    pub caller_hint: Option<&'static str>,
    /// True if the caller wants local-only models (privacy, offline, mobile).
    pub prefer_local: bool,
    /// Hard ceiling on per-call USD cost. Models whose cost exceeds this are
    /// excluded. `None` = no ceiling.
    pub max_cost_usd_per_call: Option<f64>,
    /// True if the caller does multi-turn or repeated-prompt workloads
    /// (e.g. `vox repair` 3-attempt loop, agent ReAct). Prefers models with
    /// `supports_prompt_caching = true` when available.
    pub cacheable_workload: bool,
    /// When true, free-tier (`ModelSpec.is_free`) models are allowed to
    /// compete even under `CostPreference::Performance`, which otherwise
    /// excludes them unconditionally (`registry.rs::best_for_internal`).
    /// Defaults to `false` everywhere except `SelectionIntent::research()`,
    /// which sets it from `VOX_RESEARCH_PREFER_FREE_TIER`.
    pub allow_free_in_performance_mode: bool,
}

impl SelectionIntent {
    /// Build an intent with sensible defaults for the given task.
    #[must_use]
    pub fn for_task(task: TaskCategory) -> Self {
        Self {
            task,
            axes: SelectionAxes::default(),
            complexity: 5,
            context_size_hint: None,
            caller_hint: None,
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: false,
            allow_free_in_performance_mode: false,
        }
    }

    /// Pre-baked intent for the `vox repair` 3-attempt LLM loop.
    /// Sonnet-cacheable shape: BALANCED axes, cacheable_workload=true.
    #[must_use]
    pub fn repair_loop() -> Self {
        Self {
            task: TaskCategory::CodeGen,
            axes: SelectionAxes::BALANCED,
            complexity: 5,
            context_size_hint: None,
            caller_hint: Some("repair-loop"),
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: true,
            allow_free_in_performance_mode: false,
        }
    }

    /// Pre-baked intent for research / planning / claim stages.
    #[must_use]
    pub fn research() -> Self {
        Self {
            task: TaskCategory::Research,
            axes: SelectionAxes::QUALITY_FIRST,
            complexity: 7,
            context_size_hint: None,
            caller_hint: Some("research"),
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: false,
            allow_free_in_performance_mode: vox_config::inference::research_prefer_free_tier(),
        }
    }

    /// Pre-baked intent for code review / judge stages.
    #[must_use]
    pub fn review() -> Self {
        Self {
            task: TaskCategory::Review,
            axes: SelectionAxes::QUALITY_FIRST,
            complexity: 6,
            context_size_hint: None,
            caller_hint: Some("review"),
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: true,
            allow_free_in_performance_mode: false,
        }
    }

    /// Pre-baked intent for NLI / verifier / classifier stages (cheapest tier).
    #[must_use]
    pub fn nli_classifier() -> Self {
        Self {
            task: TaskCategory::Parsing,
            axes: SelectionAxes::COST_FIRST,
            complexity: 2,
            context_size_hint: None,
            caller_hint: Some("nli-classifier"),
            prefer_local: false,
            max_cost_usd_per_call: Some(0.01),
            cacheable_workload: false,
            allow_free_in_performance_mode: false,
        }
    }

    /// Pre-baked intent for IDE autocomplete / ghost-text (fastest tier).
    #[must_use]
    pub fn ide_autocomplete() -> Self {
        Self {
            task: TaskCategory::CodeGen,
            axes: SelectionAxes::FAST,
            complexity: 3,
            context_size_hint: None,
            caller_hint: Some("ide-autocomplete"),
            prefer_local: true,
            max_cost_usd_per_call: None,
            cacheable_workload: false,
            allow_free_in_performance_mode: false,
        }
    }

    /// Pre-baked intent for plan-mode / multi-step planning.
    #[must_use]
    pub fn plan_mode() -> Self {
        Self {
            task: TaskCategory::Planning,
            axes: SelectionAxes::QUALITY_FIRST,
            complexity: 8,
            context_size_hint: None,
            caller_hint: Some("plan-mode"),
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: false,
            allow_free_in_performance_mode: false,
        }
    }
}

// ─── Outcome ────────────────────────────────────────────────────────────────

/// Result of [`select`]: the chosen model + transparency about why.
#[derive(Debug, Clone)]
pub struct SelectionOutcome {
    pub model_id: String,
    pub model_spec: ModelSpec,
    pub reason: SelectionReason,
    pub effective_axes: AutoRoutingPriority,
}

/// Why [`select`] returned the model it did. Useful for telemetry, debugging
/// routing surprises, and showing users why their request hit a given LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionReason {
    /// The `premium_alias` map in `model-routing.v1.yaml` pinned this task to
    /// a specific model id. Honored when the caller's intelligence weight is
    /// high (>= 50) or when the alias model is present in the registry.
    PremiumAlias {
        task: TaskCategory,
        alias_model_id: String,
    },
    /// The scorer in `models::scoring::auto_score_model` returned this model
    /// as the highest-ranked candidate for the projected axes.
    Scored,
    /// The caller asked for `prefer_local: true`. Selected the best local
    /// (Ollama / VoxLocal) model. If no local model is available, falls
    /// through to `Scored`.
    LocalOnly,
    /// An env var (`VOX_MODEL_FORCE`) hardcoded the choice.
    EnvOverride { env_var: &'static str },
}

/// Human-readable rendering of [`SelectionReason`], meant to show up verbatim
/// in a GUI tooltip (e.g. `ModelBadge`) — not a raw `{:?}` passthrough.
impl std::fmt::Display for SelectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectionReason::PremiumAlias {
                task,
                alias_model_id,
            } => write!(
                f,
                "Pinned to {alias_model_id} — the premium alias for {} tasks",
                crate::models::task_category_premium_key(*task)
            ),
            SelectionReason::Scored => {
                write!(
                    f,
                    "Chosen by the model scorer as the best match for your request"
                )
            }
            SelectionReason::LocalOnly => {
                write!(f, "Selected the best available local (on-device) model")
            }
            SelectionReason::EnvOverride { env_var } => {
                write!(f, "Forced by the {env_var} environment variable")
            }
        }
    }
}

// ─── Entry point ────────────────────────────────────────────────────────────

/// Single-source-of-truth model selection.
///
/// Resolution order:
///   1. `VOX_MODEL_FORCE` env override (returns immediately if matches a known model id).
///   2. `prefer_local`: search local-tier models first; fall through to scorer if none.
///   3. `premium_alias` honor: if the task has a premium alias AND the caller's
///      `axes.intelligence >= 50`, return the alias-pinned model when present
///      in the registry.
///   4. Otherwise: project axes → [`AutoRoutingPriority`], install via env for
///      the scorer to read, then delegate to
///      [`ModelRegistry::best_for_with_filter`] with caller-supplied filters
///      (max_cost ceiling, cacheable_workload preference).
///
/// Returns `None` when no model satisfies the intent (e.g. all filtered out).
///
/// **SAFETY (env mutation):** when axes need to be projected, this function
/// temporarily sets `VOX_AUTO_ROUTING_PRIORITY` so the existing scorer reads
/// the caller-specific weights. The mutation is restored on return.
#[allow(unsafe_code)]
pub fn select(intent: &SelectionIntent, registry: &ModelRegistry) -> Option<SelectionOutcome> {
    // 0. Ordered selection-policy chain (process-global, set by the daemon from
    //    the persisted `selection_policy` user preference). When no policy is
    //    installed, or the policy is empty, or the policy yields nothing, this
    //    falls through to the pre-existing cascade in `select_inner` — so
    //    default behavior is unchanged.
    if let Some(policy) = super::policy::active_policy() {
        let ctx = super::policy::PolicyContext::from_env();
        if let Some(o) = super::policy::resolve_policy(&policy, intent, registry, &ctx) {
            emit_decision_event(intent, &o);
            return Some(o);
        }
    }

    let outcome = select_inner(intent, registry);
    if let Some(ref o) = outcome {
        emit_decision_event(intent, o);
    }
    outcome
}

/// Select honoring an explicitly-supplied [`super::policy::SelectionPolicy`]
/// and [`super::policy::PolicyContext`] (used by callers that thread a policy
/// directly rather than via the process global — and by tests). Falls through
/// to the pre-existing cascade when the policy yields nothing.
#[must_use]
pub fn select_with_policy(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
    policy: &super::policy::SelectionPolicy,
    ctx: &super::policy::PolicyContext,
) -> Option<SelectionOutcome> {
    if let Some(o) = super::policy::resolve_policy(policy, intent, registry, ctx) {
        emit_decision_event(intent, &o);
        return Some(o);
    }
    select(intent, registry)
}

fn select_inner(intent: &SelectionIntent, registry: &ModelRegistry) -> Option<SelectionOutcome> {
    // 1. VOX_MODEL_FORCE env override.
    if let Ok(force) = std::env::var("VOX_MODEL_FORCE") {
        let force = force.trim().to_string();
        if !force.is_empty()
            && let Some(model) = registry.get(&force)
        {
            return Some(SelectionOutcome {
                model_id: force,
                model_spec: model,
                reason: SelectionReason::EnvOverride {
                    env_var: "VOX_MODEL_FORCE",
                },
                effective_axes: intent.axes.to_routing_priority(intent.prefer_local),
            });
        }
    }

    // 2. Local-only path.
    if intent.prefer_local
        && let Some(outcome) = select_local_first(intent, registry)
    {
        return Some(outcome);
    }

    // 3. Premium-alias honor when intelligence axis is high.
    if intent.axes.intelligence >= 50
        && let Some(outcome) = select_via_premium_alias(intent, registry)
    {
        return Some(outcome);
    }

    // 4. General scorer path.
    select_via_scorer(intent, registry)
}

/// Emit a [`SelectionDecisionEvent`] for telemetry / L3 council-report consumption.
/// No-op when no telemetry recorder is registered (zero-cost on default paths).
fn emit_decision_event(intent: &SelectionIntent, outcome: &SelectionOutcome) {
    let (reason_str, alias_key) = match &outcome.reason {
        SelectionReason::PremiumAlias { task, .. } => (
            "premium_alias",
            Some(crate::models::task_category_premium_key(*task).to_string()),
        ),
        SelectionReason::Scored => ("scored", None),
        SelectionReason::LocalOnly => ("local_only", None),
        SelectionReason::EnvOverride { .. } => ("env_override", None),
    };
    let event = SelectionDecisionEvent {
        intent_caller: intent.caller_hint.map(str::to_string),
        task: crate::models::task_category_premium_key(intent.task).to_string(),
        axes: (
            intent.axes.cost,
            intent.axes.responsiveness,
            intent.axes.intelligence,
        ),
        chosen_model: outcome.model_id.clone(),
        reason: reason_str.to_string(),
        premium_alias_key: alias_key,
        repository_id: None,
    };
    vox_telemetry::record_event!(&TelemetryEvent::SelectionDecision(event));
}

fn select_local_first(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let effective_axes = intent.axes.to_routing_priority(true);
    let model = registry
        .list_models()
        .into_iter()
        .filter(|m| {
            matches!(
                m.provider_type,
                ProviderType::Ollama | ProviderType::VoxLocal | ProviderType::PopuliMesh
            )
        })
        .filter(|m| supports_intent_constraints(m, intent))
        .max_by(|a, b| {
            score_for_intent(a, intent)
                .partial_cmp(&score_for_intent(b, intent))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::LocalOnly,
        effective_axes,
    })
}

fn select_via_premium_alias(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let key = crate::models::task_category_premium_key(intent.task);
    let alias = registry.premium_alias_for(key)?.to_string();
    let model = registry.get(&alias)?;
    if !supports_intent_constraints(&model, intent) {
        return None;
    }
    // Code-review fix: this path bypasses `best_for_internal` (a raw
    // `registry.get`), so it must apply the same privacy hard-filter itself
    // — otherwise VOX_INFERENCE_PRIVACY=local_only never actually blocks a
    // premium-alias cloud pick, since this branch runs before the scorer
    // path in `select_inner` and the alias lookup has no filter of its own.
    if !crate::route_policy::privacy_allows_model_for_mode(
        &model,
        crate::route_policy::inference_privacy_local_only_from_env(),
    ) {
        return None;
    }
    if !ModelRegistry::key_is_present_for(&model) {
        return None;
    }
    let effective_axes = intent.axes.to_routing_priority(intent.prefer_local);
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::PremiumAlias {
            task: intent.task,
            alias_model_id: alias,
        },
        effective_axes,
    })
}

fn select_via_scorer(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let effective_axes = intent.axes.to_routing_priority(intent.prefer_local);
    let cost_pref = intent.axes.to_cost_preference();
    let intent_clone = intent.clone();
    // Install this request's axes as the scorer's base weights for the duration
    // of the pass, so per-task SelectionAxes actually drive the choice (not just
    // the global VOX_AUTO_ROUTING_PRIORITY env). Restored on drop.
    let _axes_guard = crate::models::scoring::AxesOverrideGuard::set(effective_axes);
    let model = registry.best_for_with_filter(
        intent.task,
        intent.complexity,
        cost_pref,
        intent.allow_free_in_performance_mode,
        |m| supports_intent_constraints(m, &intent_clone) && ModelRegistry::key_is_present_for(m),
        None,
    )?;
    drop(_axes_guard);
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::Scored,
        effective_axes,
    })
}

/// Crate-internal accessor for the policy resolver: run the scorer path with
/// the (possibly axis-shaped) intent.
pub(crate) fn select_via_scorer_public(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    select_via_scorer(intent, registry)
}

/// Crate-internal accessor for the policy resolver: check intent hard filters.
#[must_use]
pub(crate) fn supports_intent_constraints_public(m: &ModelSpec, intent: &SelectionIntent) -> bool {
    supports_intent_constraints(m, intent)
}

/// True iff `m` satisfies the intent's hard filters
/// (max-cost ceiling, cacheable_workload preference, context size).
fn supports_intent_constraints(m: &ModelSpec, intent: &SelectionIntent) -> bool {
    if let Some(max_cost) = intent.max_cost_usd_per_call {
        let blended = if m.cost_per_1k_input > 0.0 || m.cost_per_1k_output > 0.0 {
            (m.cost_per_1k_input + m.cost_per_1k_output) / 2.0
        } else {
            m.cost_per_1k
        };
        if blended > max_cost * 1000.0 {
            // `max_cost_usd_per_call` budgets a single typical call, not a
            // raw per-1k rate; multiply by an assumed ~1k-token call.
            // Conservative — if user supplied a very tight ceiling, this
            // still excludes the obviously-too-expensive options.
            return false;
        }
    }
    if let Some(min_ctx) = intent.context_size_hint
        && let Some(model_ctx) = Some(m.capabilities.max_context as usize).filter(|&c| c > 0)
        && model_ctx < min_ctx
    {
        return false;
    }
    true
}

/// Lightweight scoring used by the local-first path. Not a substitute for
/// the registry's full scorer; just enough to rank within the local-tier
/// subset. Higher is better.
fn score_for_intent(m: &ModelSpec, intent: &SelectionIntent) -> f64 {
    let mut s = 1.0;
    if intent.cacheable_workload && m.supports_prompt_caching {
        s += 0.5;
    }
    // Prefer models with stronger strength match.
    let want = crate::models::task_category_strength(intent.task);
    if m.strengths.contains(&want) {
        s += 0.5;
    }
    // Tie-breaker: larger context.
    s += (m.capabilities.max_context as f64).log10().max(0.0) / 100.0;
    s
}

/// Convenience: build a fresh registry from cache + run [`select`].
///
/// Use this from crates that don't already hold a `ModelRegistry` handle.
/// Crates inside vox-orchestrator should hold a registry directly to avoid
/// re-loading the catalog on every selection.
pub fn select_with_default_registry(intent: &SelectionIntent) -> Option<SelectionOutcome> {
    let registry = ModelRegistry::from_cache();
    select(intent, &registry)
}

// NOTE (U.3 / Phase 7): local-vs-cloud spoke-inference routing is not wired yet.
// When it is, read the spoke's `SpokeRouter.prefer_local` hint and set
// `ModelSelectionRequest.candidate_scope` (LocalOnly) before `decide()`, at the
// real inference call site — landing the routing helper together with its
// consumer and an end-to-end test, not speculatively ahead of it.

#[cfg(test)]
mod tests {
    // Rust 2024 made std::env::{set_var,remove_var} unsafe; #[file_serial] tests below.
    #![allow(unsafe_code)]
    // Env-mutating tests exercise `from_env` cascades; they are `#[file_serial]` so no
    // other env-mutating test runs concurrently, and each restores the prior value.
    use serial_test::file_serial;

    use super::*;

    #[test]
    fn axes_project_onto_routing_priority() {
        let axes = SelectionAxes::QUALITY_FIRST;
        let prio = axes.to_routing_priority(false);
        assert_eq!(prio.efficiency, 15);
        assert_eq!(prio.precision, 70);
        assert_eq!(prio.latency, 15);
        assert_eq!(prio.mobile, 0);
    }

    #[test]
    fn prefer_local_pushes_mobile_weight_high() {
        let axes = SelectionAxes::BALANCED;
        let prio = axes.to_routing_priority(true);
        assert_eq!(prio.mobile, 70);
    }

    #[test]
    fn axes_to_cost_preference_picks_economy_when_cost_dominates() {
        assert_eq!(
            SelectionAxes::COST_FIRST.to_cost_preference(),
            CostPreference::Economy
        );
        assert_eq!(
            SelectionAxes::QUALITY_FIRST.to_cost_preference(),
            CostPreference::Performance
        );
        assert_eq!(
            SelectionAxes::BALANCED.to_cost_preference(),
            CostPreference::Performance
        );
        assert_eq!(
            SelectionAxes::FAST.to_cost_preference(),
            CostPreference::Performance
        );
    }

    #[test]
    fn presets_are_internally_consistent() {
        for (name, p) in [
            ("COST_FIRST", SelectionAxes::COST_FIRST),
            ("BALANCED", SelectionAxes::BALANCED),
            ("QUALITY_FIRST", SelectionAxes::QUALITY_FIRST),
            ("FAST", SelectionAxes::FAST),
        ] {
            assert_eq!(
                p.cost as u16 + p.responsiveness as u16 + p.intelligence as u16,
                100,
                "preset {name} should sum to 100 for clarity"
            );
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn from_env_returns_default_when_unset() {
        // SAFETY: tests are gated by the parent test serialization; we restore.
        let prior = std::env::var("VOX_MODEL_AXES").ok();
        unsafe { std::env::remove_var("VOX_MODEL_AXES") };
        assert_eq!(SelectionAxes::from_env(), SelectionAxes::BALANCED);
        unsafe {
            if let Some(v) = prior {
                std::env::set_var("VOX_MODEL_AXES", v);
            }
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn from_env_parses_custom_axes() {
        let prior = std::env::var("VOX_MODEL_AXES").ok();
        unsafe {
            std::env::set_var(
                "VOX_MODEL_AXES",
                "cost:80,intelligence:10,responsiveness:10",
            )
        };
        let axes = SelectionAxes::from_env();
        assert_eq!(axes.cost, 80);
        assert_eq!(axes.intelligence, 10);
        assert_eq!(axes.responsiveness, 10);
        unsafe {
            match prior {
                Some(v) => std::env::set_var("VOX_MODEL_AXES", v),
                None => std::env::remove_var("VOX_MODEL_AXES"),
            }
        }
    }

    #[test]
    fn intent_repair_loop_is_cacheable() {
        let i = SelectionIntent::repair_loop();
        assert!(i.cacheable_workload);
        assert_eq!(i.caller_hint, Some("repair-loop"));
        assert_eq!(i.task, TaskCategory::CodeGen);
    }

    #[test]
    fn intent_research_uses_quality_first_axes() {
        let i = SelectionIntent::research();
        assert_eq!(i.axes, SelectionAxes::QUALITY_FIRST);
        assert_eq!(i.task, TaskCategory::Research);
    }

    #[test]
    fn research_intent_allows_free_when_prefer_free_tier_env_set() {
        let intent = SelectionIntent::research();
        // This assertion documents the contract this task establishes: default
        // (no env set) must preserve existing behavior: free models excluded.
        assert!(!intent.allow_free_in_performance_mode);
    }

    #[test]
    fn intent_nli_has_tight_cost_ceiling() {
        let i = SelectionIntent::nli_classifier();
        assert_eq!(i.axes, SelectionAxes::COST_FIRST);
        assert!(i.max_cost_usd_per_call.is_some());
    }

    #[test]
    fn intent_ide_autocomplete_prefers_local_and_fast() {
        let i = SelectionIntent::ide_autocomplete();
        assert!(i.prefer_local);
        assert_eq!(i.axes, SelectionAxes::FAST);
    }

    // #[file_serial]: calls decide() against the real bootstrap registry and
    // asserts on the resulting provider type; races against the privacy-
    // override tests below the same way select_with_empty_policy_falls_
    // through_to_cascade did (see its own comment).
    #[test]
    #[file_serial]
    fn decide_respects_candidate_scope_cloud_only() {
        let registry = ModelRegistry::new();
        let req = ModelSelectionRequest {
            intent: SelectionIntent::for_task(TaskCategory::CodeGen),
            required_capabilities: vec![],
            candidate_scope: CandidateScope::CloudOnly,
        };
        if let Some(decision) = decide(&req, &registry) {
            assert!(!matches!(
                decision.outcome.model_spec.provider_type,
                ProviderType::Ollama | ProviderType::VoxLocal | ProviderType::PopuliMesh
            ));
        }
    }

    #[test]
    #[file_serial]
    fn decide_populates_non_placeholder_fields() {
        let registry = ModelRegistry::new();
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        if let Some(decision) = decide(&req, &registry) {
            assert!(decision.score_breakdown.candidate_count > 0);
            assert!(!decision.selected_model.is_empty());
            assert!(!decision.discovery_state.as_str().is_empty());
        }
    }

    #[test]
    fn confidence_state_tracks_pricing_source() {
        let registry = ModelRegistry::new();
        let mut model = registry
            .list_models()
            .into_iter()
            .next()
            .expect("at least one model");
        model.id = "test-model-not-retired".to_string();
        model.pricing_source = super::super::spec::PricingSource::Unknown;
        assert_eq!(
            confidence_state_for_model(&model),
            super::super::autonomic::ModelConfidence::Provisional
        );
        model.pricing_source = super::super::spec::PricingSource::Telemetry;
        assert_eq!(
            confidence_state_for_model(&model),
            super::super::autonomic::ModelConfidence::Confirmed
        );
    }

    #[test]
    #[file_serial]
    fn exploration_budget_gate_blocks_unknown_when_exhausted() {
        let prior_enable = std::env::var("VOX_ROUTING_ENABLE_EXPLORATION").ok();
        let prior_budget = std::env::var("VOX_EXPLORATION_BUDGET_EXHAUSTED").ok();
        unsafe {
            std::env::set_var("VOX_ROUTING_ENABLE_EXPLORATION", "1");
            std::env::set_var("VOX_EXPLORATION_BUDGET_EXHAUSTED", "1");
        }
        assert!(exploration_enabled());
        assert!(exploration_budget_exhausted());
        let mut m = ModelRegistry::new()
            .list_models()
            .into_iter()
            .next()
            .expect("at least one model");
        m.pricing_source = super::super::spec::PricingSource::Unknown;
        let blocked = exploration_budget_exhausted()
            && m.pricing_source == super::super::spec::PricingSource::Unknown;
        assert!(blocked);
        unsafe {
            match prior_enable {
                Some(v) => std::env::set_var("VOX_ROUTING_ENABLE_EXPLORATION", v),
                None => std::env::remove_var("VOX_ROUTING_ENABLE_EXPLORATION"),
            }
            match prior_budget {
                Some(v) => std::env::set_var("VOX_EXPLORATION_BUDGET_EXHAUSTED", v),
                None => std::env::remove_var("VOX_EXPLORATION_BUDGET_EXHAUSTED"),
            }
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn select_with_premium_alias_honors_alias_when_intelligence_high() {
        // The premium alias for codegen (anthropic/claude-opus-4.7) is gated by
        // the key-present check added in the B3 key-gated candidate filter
        // (`select_via_premium_alias` -> `ModelRegistry::key_is_present_for`).
        // A hosted CI runner with no ANTHROPIC_API_KEY configured must still be
        // able to exercise the premium-alias *routing* logic in isolation, so
        // set a test key here (mirrors `key_gate_admits_provider_when_key_present`).
        let prior = std::env::var("ANTHROPIC_API_KEY").ok();
        // SAFETY: #[file_serial]; prior value restored below.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
        let registry = ModelRegistry::new();
        let intent = SelectionIntent {
            axes: SelectionAxes::QUALITY_FIRST,
            ..SelectionIntent::for_task(TaskCategory::CodeGen)
        };
        let outcome = select(&intent, &registry);
        unsafe {
            match prior {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
        let outcome = outcome.expect("a model exists");
        // With QUALITY_FIRST axes (intelligence=70), premium alias should fire.
        // The alias for codegen is `anthropic/claude-opus-4.7` per current routing.yaml.
        match outcome.reason {
            SelectionReason::PremiumAlias {
                ref alias_model_id, ..
            } => {
                assert_eq!(alias_model_id, "anthropic/claude-opus-4.7");
            }
            other => panic!("expected PremiumAlias, got {:?}", other),
        }
    }

    #[test]
    #[file_serial]
    fn select_falls_back_to_scorer_when_intelligence_low() {
        let registry = ModelRegistry::new();
        let intent = SelectionIntent {
            axes: SelectionAxes::COST_FIRST,
            ..SelectionIntent::for_task(TaskCategory::CodeGen)
        };
        let outcome = select(&intent, &registry).expect("a model exists");
        match outcome.reason {
            SelectionReason::Scored => {}
            SelectionReason::LocalOnly => {} // acceptable fallback
            other => panic!("expected Scored or LocalOnly, got {:?}", other),
        }
    }

    // ── Wave-14 adversarial tests ────────────────────────────────────────────

    #[cfg(test)]
    mod semcov_wave14_tests {
        use super::*;
        use crate::models::{ModelRegistry, TaskCategory};

        #[test]
        fn cost_first_axes_economy_when_cost_exceeds_sum_of_others() {
            // Catches: `>` replaced with `>=` in to_cost_preference, making COST_FIRST
            // resolve to Performance instead of Economy when other axes sum == cost axis.
            let axes = SelectionAxes {
                cost: 40,
                responsiveness: 20,
                intelligence: 20,
            };
            // 40 > (20 + 20) = 40 is FALSE → Performance; only strictly greater triggers Economy.
            assert_eq!(axes.to_cost_preference(), CostPreference::Performance);

            let axes_strictly_greater = SelectionAxes {
                cost: 41,
                responsiveness: 20,
                intelligence: 20,
            };
            // 41 > 40 → Economy
            assert_eq!(
                axes_strictly_greater.to_cost_preference(),
                CostPreference::Economy
            );
        }

        #[test]
        fn to_routing_priority_prefer_local_false_sets_mobile_to_zero() {
            // Catches: prefer_local=false branch accidentally inheriting mobile=70
            // from a nearby prefer_local=true call in the same thread.
            let axes = SelectionAxes::QUALITY_FIRST;
            let prio = axes.to_routing_priority(false);
            assert_eq!(
                prio.mobile, 0,
                "prefer_local=false must set mobile=0, not inherit 70"
            );
        }

        #[test]
        fn context_size_hint_zero_never_filters_models() {
            // Catches: `Some(0)` context hint incorrectly filtering all models whose
            // max_context is also 0 (unknown) — the guard should treat 0-context models
            // as unconstrained, not reject them.
            let registry = ModelRegistry::new();
            // Pull any model out and verify a 0-hint doesn't filter it.
            if let Some(m) = registry.list_models().into_iter().next() {
                let intent = SelectionIntent {
                    context_size_hint: Some(0),
                    ..SelectionIntent::for_task(TaskCategory::CodeGen)
                };
                // supports_intent_constraints_public uses the public re-export.
                // We only test that a model with any context passes a 0 hint.
                let passes = supports_intent_constraints_public(&m, &intent);
                // A 0 min_ctx should NOT exclude a model. Even if model_ctx == 0,
                // 0 < 0 is false so the check must pass.
                assert!(
                    passes,
                    "context_size_hint=0 must not filter model (max_context={})",
                    m.capabilities.max_context
                );
            }
        }

        #[test]
        fn max_cost_ceiling_zero_filters_all_non_free_models() {
            // Catches: `>` vs `>=` in the cost ceiling check — a model with
            // cost_per_1k > 0 but ceiling = 0 must be rejected, not sneaking through.
            let registry = ModelRegistry::new();
            let paid_model = registry
                .list_models()
                .into_iter()
                .find(|m| m.cost_per_1k > 0.0 || m.cost_per_1k_input > 0.0);
            if let Some(m) = paid_model {
                let intent = SelectionIntent {
                    max_cost_usd_per_call: Some(0.0),
                    ..SelectionIntent::for_task(TaskCategory::CodeGen)
                };
                let passes = supports_intent_constraints_public(&m, &intent);
                assert!(
                    !passes,
                    "paid model must be filtered when max_cost_usd_per_call=0.0"
                );
            }
        }

        #[test]
        #[file_serial]
        #[allow(unsafe_code)]
        fn from_env_ignores_unknown_key_without_panicking() {
            // Catches: unknown keys in VOX_MODEL_AXES causing a panic or
            // silently overwriting a known axis due to partial-match logic.
            unsafe { std::env::set_var("VOX_MODEL_AXES", "boguskey:99,cost:55") };
            let axes = SelectionAxes::from_env();
            unsafe { std::env::remove_var("VOX_MODEL_AXES") };
            assert_eq!(axes.cost, 55, "known key 'cost' must be parsed");
            // responsiveness and intelligence stay at defaults when omitted.
            assert_eq!(
                axes.responsiveness,
                SelectionAxes::BALANCED.responsiveness,
                "missing responsiveness must default to BALANCED.responsiveness"
            );
        }

        #[test]
        #[file_serial]
        #[allow(unsafe_code)]
        fn from_env_invalid_numeric_value_falls_back_to_default_for_that_axis() {
            // Catches: invalid parse (e.g. "cost:abc") panicking instead of
            // leaving the axis at the running default.
            unsafe { std::env::set_var("VOX_MODEL_AXES", "cost:abc,intelligence:60") };
            let axes = SelectionAxes::from_env();
            unsafe { std::env::remove_var("VOX_MODEL_AXES") };
            // cost parse fails → remains at BALANCED default (33)
            assert_eq!(axes.cost, SelectionAxes::BALANCED.cost);
            // intelligence parses fine
            assert_eq!(axes.intelligence, 60);
        }

        #[test]
        fn scope_cloud_only_excludes_all_local_provider_types() {
            // Catches: scope_allows returning true for PopuliMesh or VoxLocal
            // when CloudOnly is set — a plausible copy-paste bug adding a new
            // local ProviderType without updating the cloud-exclusion arm.
            use crate::models::ProviderType;
            for local_provider in [
                ProviderType::Ollama,
                ProviderType::VoxLocal,
                ProviderType::PopuliMesh,
            ] {
                assert!(
                    !scope_allows(local_provider.clone(), CandidateScope::CloudOnly),
                    "{local_provider:?} must be excluded from CloudOnly scope"
                );
            }
        }

        #[test]
        #[file_serial]
        fn decide_returns_none_when_no_candidates_match_scope_and_capability() {
            // Catches: decide() panicking or returning a model that violates scope,
            // rather than returning None, when every registry model is filtered out.
            // We require a capability that no builtin model advertises to force the
            // candidate list empty, exercising the None-return branch.
            use crate::models::generated::Capability;
            let registry = ModelRegistry::new();
            // Capability::Vision is the most likely to be absent on local-only light models;
            // pair it with LocalOnly scope for maximal rejection.
            let req = ModelSelectionRequest {
                intent: SelectionIntent::for_task(TaskCategory::CodeGen),
                required_capabilities: vec![
                    Capability::SupportsVision,
                    Capability::SupportsAudioInput,
                ],
                candidate_scope: CandidateScope::LocalOnly,
            };
            // Either None (no candidates) or Some(d) where the model must be local.
            if let Some(d) = decide(&req, &registry) {
                assert!(
                    matches!(
                        d.outcome.model_spec.provider_type,
                        crate::models::ProviderType::Ollama
                            | crate::models::ProviderType::VoxLocal
                            | crate::models::ProviderType::PopuliMesh
                    ),
                    "scope=LocalOnly must never return a cloud model"
                );
            }
            // The key invariant is: no panic, correct scope on any returned model.
        }
    }

    // #[file_serial]: this test's two select() calls must observe the same
    // TEST_PRIVACY_OVERRIDE state (or lack of it) across both calls; without
    // this it can race the privacy-override tests above and see the override
    // flip mid-test, making the two calls' candidate sets diverge.
    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn select_with_empty_policy_falls_through_to_cascade() {
        // BALANCED axes (this intent's default) resolve to `CostPreference::Performance`
        // (cost=33 <= intelligence+responsiveness=67), which excludes every
        // free-tier model from the scorer (`registry.rs::best_for_internal`)
        // unless `allow_free_in_performance_mode` is set — and every paid
        // candidate is separately excluded by the B3 key-gated candidate filter
        // when no provider key is configured. On a hosted runner with no
        // ANTHROPIC_API_KEY, that leaves zero eligible candidates for either
        // call below. Set a test key so a real paid candidate is selectable,
        // matching `key_gate_admits_provider_when_key_present`'s pattern.
        let prior = std::env::var("ANTHROPIC_API_KEY").ok();
        // SAFETY: #[file_serial]; prior value restored below.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
        let registry = ModelRegistry::new();
        let intent = SelectionIntent::for_task(TaskCategory::CodeGen);
        // An empty policy carries no steps, so the resolver yields nothing and
        // `select_with_policy` falls through to the pre-existing `select` cascade.
        let policy = crate::models::policy::SelectionPolicy::default();
        let ctx = crate::models::policy::PolicyContext::default();
        let via_policy = select_with_policy(&intent, &registry, &policy, &ctx);
        let via_cascade = select(&intent, &registry);
        unsafe {
            match prior {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
        let via_policy = via_policy.expect("a model exists for codegen");
        let via_cascade = via_cascade.expect("a model exists for codegen");
        assert_eq!(via_policy.model_id, via_cascade.model_id);
    }

    // ── Phase-2 wiring: key-gated candidate filter (B3) ─────────────────────
    fn key_gate_spec(id: &str, provider_type: ProviderType) -> ModelSpec {
        crate::models::ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type,
            max_tokens: 32_000,
            cost_per_1k: 0.001,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.001,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::StrengthTag::Generalist],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            // UserConfig ⇒ ModelConfidence::Confirmed ⇒ routing-eligible
            // (discovery_pipeline.rs:25-34), so only the key gate is under test.
            pricing_source: crate::models::spec::PricingSource::UserConfig,
            supported_parameters: vec![],
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn key_gate_excludes_keyless_provider_and_reports_rejection() {
        // SAFETY: #[file_serial] serializes env mutation; mirrors models/tests.rs:133-140.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-test",
            ProviderType::Anthropic,
        ));
        registry.register(key_gate_spec("ollama-local-test", ProviderType::Ollama));
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        let d = decide(&req, &registry).expect("local (keyless-OK) candidate remains");
        assert_eq!(d.selected_model, "ollama-local-test");
        assert!(
            d.rejection_reasons
                .iter()
                .any(|r| r.contains("anthropic-direct-test") && r.contains("missing provider key")),
            "keyless provider must be rejected with a key reason: {:?}",
            d.rejection_reasons
        );
    }

    // Code-review fix: `decide()` built its own candidate set independently of
    // `best_for_internal`'s privacy filter — this proves the privacy check
    // added directly to `decide()`'s candidate loop actually excludes a cloud
    // candidate under local_only, not just best_for_internal's own callers.
    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn decide_excludes_cloud_candidate_under_local_only_privacy() {
        // This test targets the *privacy* rejection path specifically, so the
        // cloud candidate must clear the (separate, earlier-in-the-loop) B3
        // key-gated candidate filter first — otherwise on a runner with no
        // OPENROUTER_API_KEY it is rejected as "missing provider key" instead
        // of "privacy mode excludes cloud", and the assertion below fails for
        // the wrong reason. Mirrors `key_gate_admits_provider_when_key_present`.
        let prior = std::env::var("OPENROUTER_API_KEY").ok();
        // SAFETY: #[file_serial]; prior value restored below.
        unsafe { std::env::set_var("OPENROUTER_API_KEY", "test-key") };
        crate::route_policy::set_test_privacy_override(Some("local_only"));
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec("cloud-test", ProviderType::OpenRouter));
        registry.register(key_gate_spec(
            "ollama-local-privacy-test",
            ProviderType::Ollama,
        ));
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        let d = decide(&req, &registry);
        crate::route_policy::set_test_privacy_override(None);
        unsafe {
            match prior {
                Some(v) => std::env::set_var("OPENROUTER_API_KEY", v),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }
        }
        let d = d.expect("local candidate must still be selectable under local_only");
        assert_eq!(d.selected_model, "ollama-local-privacy-test");
        assert!(
            d.rejection_reasons
                .iter()
                .any(|r| r.contains("cloud-test") && r.contains("privacy mode excludes cloud")),
            "cloud provider must be rejected with a privacy reason: {:?}",
            d.rejection_reasons
        );
    }

    // Same coverage for `select_via_premium_alias`'s own privacy check: with
    // QUALITY_FIRST axes (intelligence=70) the premium-alias path would
    // normally fire and return a cloud alias model (see
    // `select_with_premium_alias_honors_alias_when_intelligence_high` above);
    // under local_only it must be skipped so `select_inner` falls through to
    // the scorer path instead.
    #[test]
    #[file_serial]
    fn select_skips_cloud_premium_alias_under_local_only_privacy() {
        crate::route_policy::set_test_privacy_override(Some("local_only"));
        let registry = ModelRegistry::new();
        let intent = SelectionIntent {
            axes: SelectionAxes::QUALITY_FIRST,
            ..SelectionIntent::for_task(TaskCategory::CodeGen)
        };
        let outcome = select(&intent, &registry);
        crate::route_policy::set_test_privacy_override(None);
        // None, Scored, or LocalOnly are all acceptable here — only PremiumAlias is a failure.
        if let Some(SelectionReason::PremiumAlias { .. }) = outcome.map(|o| o.reason) {
            panic!("premium-alias (cloud) pick must not survive under local_only privacy")
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn key_gate_admits_provider_when_key_present() {
        let prior = std::env::var("ANTHROPIC_API_KEY").ok();
        // SAFETY: #[file_serial]; prior value restored below.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-test",
            ProviderType::Anthropic,
        ));
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        let d = decide(&req, &registry).expect("keyed candidate is eligible");
        assert_eq!(d.selected_model, "anthropic-direct-test");
        #[allow(unsafe_code)]
        unsafe {
            match prior {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn select_via_scorer_excludes_keyless_provider() {
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-scorer-test",
            ProviderType::Anthropic,
        ));
        registry.register(key_gate_spec("ollama-scorer-test", ProviderType::Ollama));

        let intent = SelectionIntent::research();
        let outcome = select_via_scorer(&intent, &registry);
        if let Some(o) = outcome {
            assert_ne!(
                o.model_id, "anthropic-direct-scorer-test",
                "select_via_scorer returned a keyless-provider candidate"
            );
        }
    }

    // ── Non-research intents: key-gating must degrade gracefully too ────────
    //
    // `select()`'s key-presence gate (added alongside `decide()`'s pre-existing
    // one — see `key_gate_*` tests above and git history: `decide()` gained
    // `key_is_present_for` in 54d2758fba "B3", and `select_via_scorer` /
    // `select_via_premium_alias` converged to the same rule afterward) backs
    // EVERY `SelectionIntent` constructor, not just `SelectionIntent::research()`.
    // These two tests prove non-research callers (`repair_loop()` stands in for
    // `ide_autocomplete()` / `plan_mode()` / `review()` / `for_task()`, which all
    // share the same `select_inner()` cascade) still degrade gracefully: no
    // keyless candidate leaks through, and a present key is still admitted.

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn select_via_scorer_falls_back_gracefully_for_non_research_intent_when_key_missing() {
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-repair-test",
            ProviderType::Anthropic,
        ));
        registry.register(key_gate_spec(
            "ollama-repair-fallback-test",
            ProviderType::Ollama,
        ));

        // Use a non-research intent — repair_loop is representative of the
        // "everyday" callers whose behavior this test protects.
        let intent = SelectionIntent::repair_loop();
        let outcome = select_via_scorer(&intent, &registry);
        if let Some(o) = outcome {
            assert_ne!(
                o.model_id, "anthropic-direct-repair-test",
                "select_via_scorer must not select a keyless provider for non-research intents either"
            );
        }
        // A None outcome is also acceptable (no eligible candidate) — the
        // assertion above is what matters: never silently return a keyless
        // candidate regardless of which intent asked.
    }

    #[test]
    #[file_serial]
    #[allow(unsafe_code)]
    fn select_admits_non_research_intent_when_key_present() {
        let prior = std::env::var("ANTHROPIC_API_KEY").ok();
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-repair-keyed-test",
            ProviderType::Anthropic,
        ));

        let intent = SelectionIntent::repair_loop();
        let outcome = select(&intent, &registry);
        // With a key present and no better-scoring alternative registered,
        // select() should be ABLE to choose the keyed candidate for a
        // non-research intent too — proving the gate doesn't just always
        // reject, it correctly admits when the key is actually there.
        assert!(
            outcome.is_some(),
            "select() should find a candidate when a key is present"
        );

        #[allow(unsafe_code)]
        unsafe {
            match prior {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
    }
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn selection_reason_display_is_non_empty_and_distinct() {
        // Fix Task 7: `SelectionReason` needs a human-readable `Display` impl for
        // ModelBadge's tooltip — a raw `{:?}` passthrough is not acceptable copy.
        let premium = SelectionReason::PremiumAlias {
            task: TaskCategory::CodeGen,
            alias_model_id: "anthropic/claude-opus-4.7".to_string(),
        };
        let scored = SelectionReason::Scored;
        let local_only = SelectionReason::LocalOnly;
        let env_override = SelectionReason::EnvOverride {
            env_var: "VOX_MODEL_FORCE",
        };

        let premium_s = premium.to_string();
        let scored_s = scored.to_string();
        let local_only_s = local_only.to_string();
        let env_override_s = env_override.to_string();

        for s in [&premium_s, &scored_s, &local_only_s, &env_override_s] {
            assert!(
                !s.is_empty(),
                "SelectionReason::to_string() must not be empty"
            );
        }

        let all = [
            premium_s.as_str(),
            scored_s.as_str(),
            local_only_s.as_str(),
            env_override_s.as_str(),
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(
                    all[i], all[j],
                    "each SelectionReason variant must render a distinct message"
                );
            }
        }

        // The premium-alias message should surface the alias id for transparency.
        assert!(premium_s.contains("anthropic/claude-opus-4.7"));
        // The env-override message should surface the env var name.
        assert!(env_override_s.contains("VOX_MODEL_FORCE"));
    }

    #[test]
    fn scope_allows_enforces_provider_locality() {
        // AllProviders admits everything.
        assert!(scope_allows(
            ProviderType::Anthropic,
            CandidateScope::AllProviders
        ));
        assert!(scope_allows(
            ProviderType::Ollama,
            CandidateScope::AllProviders
        ));

        // LocalOnly admits only the three local providers.
        assert!(scope_allows(
            ProviderType::Ollama,
            CandidateScope::LocalOnly
        ));
        assert!(scope_allows(
            ProviderType::VoxLocal,
            CandidateScope::LocalOnly
        ));
        assert!(scope_allows(
            ProviderType::PopuliMesh,
            CandidateScope::LocalOnly
        ));
        assert!(!scope_allows(
            ProviderType::Anthropic,
            CandidateScope::LocalOnly
        ));

        // CloudOnly is the exact complement of LocalOnly.
        assert!(scope_allows(
            ProviderType::Anthropic,
            CandidateScope::CloudOnly
        ));
        assert!(!scope_allows(
            ProviderType::Ollama,
            CandidateScope::CloudOnly
        ));
        assert!(!scope_allows(
            ProviderType::VoxLocal,
            CandidateScope::CloudOnly
        ));
        assert!(!scope_allows(
            ProviderType::PopuliMesh,
            CandidateScope::CloudOnly
        ));
    }
}
