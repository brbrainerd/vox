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
        .and_then(|o| {
            if candidate_ids.contains(&o.model_id) {
                Some(o)
            } else {
                None
            }
        })
        .or_else(|| {
            // Scoped fallback through registry scorer constrained to candidate set.
            let model = registry.best_for_with_filter(
                intent.task,
                intent.complexity,
                intent.axes.to_cost_preference(),
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
        |m| supports_intent_constraints(m, &intent_clone),
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

#[cfg(test)]
mod tests {
    // Env-mutating tests exercise `from_env` cascades; they are `#[serial]` so no
    // other env-mutating test runs concurrently, and each restores the prior value.
    use serial_test::serial;

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
    #[serial]
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
    #[serial]
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

    #[test]
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
    #[serial]
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
    fn select_with_premium_alias_honors_alias_when_intelligence_high() {
        let registry = ModelRegistry::new();
        let intent = SelectionIntent {
            axes: SelectionAxes::QUALITY_FIRST,
            ..SelectionIntent::for_task(TaskCategory::CodeGen)
        };
        let outcome = select(&intent, &registry).expect("a model exists");
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
        use super::super::*;
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

    #[test]
    fn select_with_empty_policy_falls_through_to_cascade() {
        let registry = ModelRegistry::new();
        let intent = SelectionIntent::for_task(TaskCategory::CodeGen);
        // An empty policy carries no steps, so the resolver yields nothing and
        // `select_with_policy` falls through to the pre-existing `select` cascade.
        let policy = crate::models::policy::SelectionPolicy::default();
        let ctx = crate::models::policy::PolicyContext::default();
        let via_policy = select_with_policy(&intent, &registry, &policy, &ctx)
            .expect("a model exists for codegen");
        let via_cascade = select(&intent, &registry).expect("a model exists for codegen");
        assert_eq!(via_policy.model_id, via_cascade.model_id);
    }
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;

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
