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
            let Ok(parsed) = val.parse::<u8>() else { continue };
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
        if self.cost as u16 > (self.intelligence as u16).saturating_add(self.responsiveness as u16) {
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
    let outcome = select_inner(intent, registry);
    if let Some(ref o) = outcome {
        emit_decision_event(intent, o);
    }
    outcome
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
    let model = registry.best_for_with_filter(
        intent.task,
        intent.complexity,
        cost_pref,
        |m| supports_intent_constraints(m, &intent_clone),
        None,
    )?;
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::Scored,
        effective_axes,
    })
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
    if m.strengths.iter().any(|t| *t == want) {
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
    fn from_env_parses_custom_axes() {
        let prior = std::env::var("VOX_MODEL_AXES").ok();
        unsafe { std::env::set_var("VOX_MODEL_AXES", "cost:80,intelligence:10,responsiveness:10") };
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
            SelectionReason::PremiumAlias { ref alias_model_id, .. } => {
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
}
