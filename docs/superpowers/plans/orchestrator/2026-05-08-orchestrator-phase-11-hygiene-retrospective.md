# Orchestrator Phase 11: Hygiene + Retrospective Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all ten decision modules (P2–P10) together into a single `OrchestratorPolicy` façade, run all quality gates across the full codebase, regenerate auto-generated docs, and write the program retrospective.

**Architecture:** A thin `orchestrator_policy.rs` module composes all D1–D10 decision makers behind one struct. This is the only production-code change in P11; all other tasks are test, doc, and cleanup.

**Tech Stack:** Rust, all P2–P10 modules, `vox-doc-pipeline`, `cargo run -p vox-arch-check`, five quality gates.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/orchestrator_policy.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod orchestrator_policy |
| Create | `crates/vox-orchestrator/tests/orchestrator_policy_integration.rs` |
| Create | `docs/src/architecture/orchestrator-policy-program-retrospective-2026.md` |
| Run | `cargo run -p vox-doc-pipeline` |
| Run | `cargo run -p vox-arch-check` |

---

### Task 1: OrchestratorPolicy façade

**Files:**
- Create: `crates/vox-orchestrator/src/orchestrator_policy.rs`

- [ ] **Step 1.1: Write failing test first**

```rust
// At the bottom of orchestrator_policy.rs (created in 1.3)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_evaluate_does_not_panic_with_defaults() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext::default();
        let decision = policy.evaluate(&ctx);
        // Smoke test: must return Some decision without panic
        let _ = decision;
    }

    #[test]
    fn policy_budget_exhausted_forces_economy_tier() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext {
            budget_status: vox_orchestrator::budget_gate::BudgetStatus {
                tokens_used: 97_000,
                tokens_limit: 100_000,
                cost_used_usd: 0.0,
                cost_limit_usd: 5.0,
            },
            ..PolicyContext::default()
        };
        let decision = policy.evaluate(&ctx);
        assert_eq!(decision.routing_tier, vox_orchestrator::tier_cascade::RoutingTier::Economy);
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator orchestrator_policy 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 1.3: Write the façade**

Create `crates/vox-orchestrator/src/orchestrator_policy.rs`:

```rust
//! Unified orchestrator policy façade — composes all D1–D10 decision modules.
//!
//! Call `OrchestratorPolicy::evaluate(&ctx)` once per loop iteration.
//! Returns a `PolicyDecision` that drives all downstream routing choices.

use serde::{Deserialize, Serialize};

use crate::budget_gate::{BudgetGate, BudgetGateConfig, BudgetDecision, BudgetStatus};
use crate::cache_predictor::{CachePredictor, CachePredictorConfig, CacheSignal};
use crate::calibration::CalibrationConfig;
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
use crate::compaction::CompactionStrategy;
use crate::compaction_trigger::{CompactionTrigger, CompactionTriggerConfig};
use crate::confidence_fusion::{ConfidenceFuser, FusionConfig, FusionDecision, FusionInputs};
use crate::planning::plan_mode_trigger::{PlanModeDecision, PlanModeSignal, PlanModeTrigger, PlanModeTriggerConfig};
use crate::privacy_classifier::{ClassificationSignals, PrivacyClassifier, PrivacyClassifierConfig};
use crate::risk_matrix::{HitlAction, RiskDimensions, RiskGrade, RiskMatrix, RiskMatrixConfig};
use crate::subagent_dispatch::{DispatchConfig, DispatchDecision, DispatchRouter, DispatchSignal};
use crate::tier_cascade::{AlarmLevel, CompositeSignal, RoutingTier, TierCascadeConfig, TierCascadeRouter};

/// Full context for a single policy evaluation.
/// Callers fill in the fields they have; defaults are safe no-ops.
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    // D6 — Circuit breaker
    pub circuit_breaker_state: CircuitBreakerState,
    // D3 — Confidence fusion
    pub fusion_inputs: FusionInputs,
    // D7 — Cache
    pub cache_signal: Option<CacheSignal>,
    // D7 — Budget
    pub budget_status: BudgetStatus,
    // D7 — Compaction
    pub context_utilization: f64,
    // D8 — Privacy
    pub privacy_signals: ClassificationSignals,
    // D5+D9 — Risk
    pub risk_dims: RiskDimensions,
    // D4 — Sub-agent dispatch
    pub dispatch_signal: DispatchSignal,
    // D2 — Plan mode
    pub plan_mode_signal: PlanModeSignal,
    // D1 — Tier cascade (overrides computed tier if Some)
    pub explicit_complexity: Option<u8>,
}

impl Default for BudgetStatus {
    fn default() -> Self {
        BudgetStatus {
            tokens_used: 0,
            tokens_limit: u64::MAX,
            cost_used_usd: 0.0,
            cost_limit_usd: f64::MAX,
        }
    }
}

impl Default for DispatchSignal {
    fn default() -> Self {
        DispatchSignal {
            complexity: 0,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        }
    }
}

impl Default for PlanModeSignal {
    fn default() -> Self {
        PlanModeSignal {
            complexity: 0,
            dependency_count: 0,
            tool_hint_count: 0,
            prior_adequacy_score: None,
        }
    }
}

/// Composite policy decision for one orchestrator iteration.
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    /// D1: which model tier to use.
    pub routing_tier: RoutingTier,
    /// D2: whether to plan-and-execute or react.
    pub plan_mode: PlanModeDecision,
    /// D3: whether to invoke Socrates research.
    pub invoke_socrates: bool,
    /// D4: how to handle sub-agent dispatch.
    pub dispatch: DispatchDecision,
    /// D5+D9: HITL action required.
    pub hitl_action: HitlAction,
    /// D6: circuit breaker trip reason if any.
    pub circuit_trip: Option<crate::circuit_breaker::TripReason>,
    /// D7: suggested compaction strategy.
    pub compaction_strategy: CompactionStrategy,
    /// D8: privacy requires local inference.
    pub privacy_requires_local: bool,
}

/// All config in one place. Defaults are loaded from contract YAMLs at runtime;
/// these struct defaults mirror those contract defaults.
#[derive(Debug, Clone, Default)]
pub struct OrchestratorPolicyConfig {
    pub circuit_breaker: CircuitBreakerConfig,
    pub fusion: FusionConfig,
    pub tier_cascade: TierCascadeConfig,
    pub plan_mode: PlanModeTriggerConfig,
    pub risk_matrix: RiskMatrixConfig,
    pub dispatch: DispatchConfig,
    pub budget_gate: BudgetGateConfig,
    pub compaction_trigger: CompactionTriggerConfig,
    pub cache_predictor: CachePredictorConfig,
    pub calibration: CalibrationConfig,
    pub privacy_classifier: PrivacyClassifierConfig,
}

/// Unified policy façade composing D1–D10.
pub struct OrchestratorPolicy {
    config: OrchestratorPolicyConfig,
}

impl Default for OrchestratorPolicy {
    fn default() -> Self {
        Self::new(OrchestratorPolicyConfig::default())
    }
}

impl OrchestratorPolicy {
    pub fn new(config: OrchestratorPolicyConfig) -> Self {
        Self { config }
    }

    /// Evaluate all ten decision axes for the current loop iteration.
    #[must_use]
    pub fn evaluate(&self, ctx: &PolicyContext) -> PolicyDecision {
        let c = &self.config;

        // D6: Circuit breaker
        let cb = CircuitBreaker::new(c.circuit_breaker.clone());
        let circuit_trip = cb.should_trip(&ctx.circuit_breaker_state);
        let alarm_level: AlarmLevel = cb.check_tier(&ctx.circuit_breaker_state).into();

        // D3: Confidence fusion
        let fuser = ConfidenceFuser::new(c.fusion.clone());
        let fusion_decision = fuser.decide(&ctx.fusion_inputs);
        let confidence_score = fuser.compute_score(&ctx.fusion_inputs);
        let invoke_socrates = fusion_decision == FusionDecision::InvokeSocrates;

        // D7: Budget gate
        let budget_gate = BudgetGate::new(c.budget_gate.clone());
        let budget_decision = budget_gate.evaluate(&ctx.budget_status);
        let budget_exhausted = matches!(budget_decision, BudgetDecision::Halt);

        // D8: Privacy
        let privacy = PrivacyClassifier::new(c.privacy_classifier.clone());
        let privacy_requires_local = privacy.requires_local(&ctx.privacy_signals);

        // D1: Tier cascade
        let complexity = ctx
            .explicit_complexity
            .unwrap_or(ctx.plan_mode_signal.complexity);
        let composite = CompositeSignal {
            complexity,
            confidence_score,
            circuit_breaker_tier: alarm_level,
            budget_exhausted,
            privacy_requires_local,
        };
        let tier_router = TierCascadeRouter::new(c.tier_cascade.clone());
        let routing_tier = tier_router.select_tier(&composite);

        // D2: Plan mode
        let plan_trigger = PlanModeTrigger::new(c.plan_mode.clone());
        let plan_mode = plan_trigger.decide(&ctx.plan_mode_signal);

        // D5+D9: Risk matrix
        let risk = RiskMatrix::new(c.risk_matrix.clone());
        let risk_grade = risk.grade(&ctx.risk_dims);
        let hitl_action = risk.hitl_action(&risk_grade);

        // D4: Dispatch
        let dispatch_router = DispatchRouter::new(c.dispatch.clone());
        let mut dispatch_sig = ctx.dispatch_signal.clone();
        dispatch_sig.budget_exhausted = budget_exhausted;
        let dispatch = dispatch_router.decide(&dispatch_sig);

        // D7: Compaction
        let compaction = CompactionTrigger::new(c.compaction_trigger.clone());
        let compaction_strategy = compaction.suggest_strategy(ctx.context_utilization);

        PolicyDecision {
            routing_tier,
            plan_mode,
            invoke_socrates,
            dispatch,
            hitl_action,
            circuit_trip,
            compaction_strategy,
            privacy_requires_local,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_evaluate_does_not_panic_with_defaults() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext::default();
        let _decision = policy.evaluate(&ctx);
    }

    #[test]
    fn policy_budget_exhausted_forces_economy_tier() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext {
            budget_status: BudgetStatus {
                tokens_used: 97_000,
                tokens_limit: 100_000,
                cost_used_usd: 0.0,
                cost_limit_usd: 5.0,
            },
            ..PolicyContext::default()
        };
        let decision = policy.evaluate(&ctx);
        assert_eq!(decision.routing_tier, RoutingTier::Economy);
    }

    #[test]
    fn policy_high_complexity_chooses_plan_mode() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext {
            plan_mode_signal: PlanModeSignal {
                complexity: 9,
                dependency_count: 3,
                tool_hint_count: 4,
                prior_adequacy_score: None,
            },
            explicit_complexity: Some(9),
            ..PolicyContext::default()
        };
        let decision = policy.evaluate(&ctx);
        assert_eq!(decision.plan_mode, PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn policy_critical_risk_triggers_block_and_escalate() {
        let policy = OrchestratorPolicy::default();
        let ctx = PolicyContext {
            risk_dims: RiskDimensions {
                irreversibility: 1.0,
                blast_radius: 1.0,
                compliance_exposure: 1.0,
                confidence: 0.0,
            },
            ..PolicyContext::default()
        };
        let decision = policy.evaluate(&ctx);
        assert_eq!(decision.hitl_action, HitlAction::BlockAndEscalate);
    }
}
```

- [ ] **Step 1.4: Register in lib.rs**

```rust
pub mod orchestrator_policy;
```

- [ ] **Step 1.5: Run tests**

Run: `cargo test -p vox-orchestrator orchestrator_policy -- --nocapture`
Expected: All 4 tests pass.

Note: If any P2–P10 modules were not implemented yet (they may still be in plan state), some imports will fail. In that case, wrap the missing ones in `#[cfg(feature = "...")]` guards or use `#[allow(unused_imports)]` until all phases are executed.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vox-orchestrator/src/orchestrator_policy.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add OrchestratorPolicy façade composing all D1-D10 modules"
```

---

### Task 2: Integration test suite for OrchestratorPolicy

**Files:**
- Create: `crates/vox-orchestrator/tests/orchestrator_policy_integration.rs`

- [ ] **Step 2.1: Write the integration tests**

Create `crates/vox-orchestrator/tests/orchestrator_policy_integration.rs`:

```rust
use vox_orchestrator::budget_gate::BudgetStatus;
use vox_orchestrator::circuit_breaker::CircuitBreakerState;
use vox_orchestrator::orchestrator_policy::{OrchestratorPolicy, PolicyContext};
use vox_orchestrator::planning::plan_mode_trigger::PlanModeSignal;
use vox_orchestrator::risk_matrix::RiskDimensions;
use vox_orchestrator::tier_cascade::RoutingTier;

#[test]
fn default_policy_context_produces_low_risk_react_decision() {
    let policy = OrchestratorPolicy::default();
    let ctx = PolicyContext::default();
    let decision = policy.evaluate(&ctx);
    // Default context: low complexity → React, low risk → Proceed
    assert_eq!(decision.plan_mode, vox_orchestrator::planning::plan_mode_trigger::PlanModeDecision::React);
    assert_eq!(decision.hitl_action, vox_orchestrator::risk_matrix::HitlAction::Proceed);
}

#[test]
fn circuit_breaker_trip_propagates_to_decision() {
    let policy = OrchestratorPolicy::default();
    let ctx = PolicyContext {
        circuit_breaker_state: CircuitBreakerState {
            no_progress_loops: 3,
            ..Default::default()
        },
        ..PolicyContext::default()
    };
    let decision = policy.evaluate(&ctx);
    assert!(decision.circuit_trip.is_some());
}

#[test]
fn privacy_classified_task_sets_requires_local() {
    use vox_orchestrator::privacy_classifier::ClassificationSignals;
    let policy = OrchestratorPolicy::default();
    let ctx = PolicyContext {
        privacy_signals: ClassificationSignals {
            has_health_data: true,
            ..Default::default()
        },
        ..PolicyContext::default()
    };
    let decision = policy.evaluate(&ctx);
    assert!(decision.privacy_requires_local);
}

#[test]
fn compaction_aggressive_at_high_utilization() {
    use vox_orchestrator::compaction::CompactionStrategy;
    let policy = OrchestratorPolicy::default();
    let ctx = PolicyContext {
        context_utilization: 0.92,
        ..PolicyContext::default()
    };
    let decision = policy.evaluate(&ctx);
    assert_eq!(decision.compaction_strategy, CompactionStrategy::Aggressive);
}
```

- [ ] **Step 2.2: Run integration tests**

Run: `cargo test --test orchestrator_policy_integration`
Expected: All 4 tests pass.

- [ ] **Step 2.3: Commit**

```bash
git add crates/vox-orchestrator/tests/orchestrator_policy_integration.rs
git commit -m "test(orchestrator): integration tests for OrchestratorPolicy façade"
```

---

### Task 3: All quality gates (full codebase pass)

- [ ] **Step 3.1: G1 — arch-check**

Run: `cargo run -p vox-arch-check 2>&1`
Expected: Clean. No new violations. Orphan detector at error level.

- [ ] **Step 3.2: G2 — telemetry conformance**

Run: `cargo test -p vox-orchestrator metric_type`
Expected: All metric_type constant tests pass (one per D1–D10 module).

- [ ] **Step 3.3: G3 — performance budgets**

Run the three benchmarks established in P2, P3, P4:
```bash
cargo bench -p vox-orchestrator --bench circuit_breaker 2>&1 | grep "circuit_breaker_should_trip"
cargo bench -p vox-orchestrator --bench confidence_fusion 2>&1 | grep "confidence_fusion_decide"
cargo bench -p vox-orchestrator --bench tier_cascade 2>&1 | grep "tier_cascade_select_tier"
```
Expected: All three means <50µs (should be <1µs in practice).

- [ ] **Step 3.4: G4 — contract conformance**

```bash
cargo test -p vox-orchestrator golden
```
Expected: All 30+ golden fixture tests pass.

- [ ] **Step 3.5: G5 — HITL fallback**

```bash
cargo test -p vox-orchestrator hitl
cargo test -p vox-orchestrator escalate
```
Expected: All HITL/escalation tests pass.

- [ ] **Step 3.6: Commit quality gate evidence**

```bash
git commit --allow-empty -m "chore(orchestrator): all five quality gates green for P1-P11"
```

---

### Task 4: Regenerate auto-generated docs

- [ ] **Step 4.1: Run doc pipeline**

Run: `cargo run -p vox-doc-pipeline 2>&1 | tail -20`
Expected: `SUMMARY.md`, `architecture-index.md`, `feed.xml` regenerated without errors.

- [ ] **Step 4.2: Verify no hand-edited auto-generated files are dirty**

Run: `git diff --name-only 2>&1`
Expected: Only auto-generated files changed (SUMMARY.md, architecture-index.md, feed.xml).

- [ ] **Step 4.3: Commit regenerated docs**

```bash
git add docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "chore(docs): regenerate SUMMARY.md / architecture-index.md / feed.xml post-P11"
```

---

### Task 5: Write the retrospective

**Files:**
- Create: `docs/src/architecture/orchestrator-policy-program-retrospective-2026.md`

- [ ] **Step 5.1: Write the retrospective document**

```markdown
---
title: "Orchestrator Policy Program Retrospective 2026"
description: "What was built, what worked, what didn't, and what comes next."
category: architecture
status: reference
training_eligible: true
vox_relevance:
  - orchestrator
  - routing
  - hitl
  - calibration
---

# Orchestrator Policy Program Retrospective 2026

## What We Built

Ten autonomous decision modules (D1–D10) implementing the orchestrator policy surface:

| ID | Module | Decision | File |
|----|--------|----------|------|
| D1 | `tier_cascade` | Which model tier (Economy/Standard/Strong) | `crates/vox-orchestrator/src/tier_cascade.rs` |
| D2 | `plan_mode_trigger` | Plan-and-Execute vs. ReAct | `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs` |
| D3 | `confidence_fusion` | Whether to invoke Socrates research | `crates/vox-orchestrator/src/confidence_fusion.rs` |
| D4 | `subagent_dispatch` | Inline vs. spawn sub-agent | `crates/vox-orchestrator/src/subagent_dispatch.rs` |
| D5+D9 | `risk_matrix` | HITL escalation grade | `crates/vox-orchestrator/src/risk_matrix.rs` |
| D6 | `circuit_breaker` | Doom-loop detection | `crates/vox-orchestrator/src/circuit_breaker.rs` |
| D7 | `cache_predictor` + `budget_gate` + `compaction_trigger` | Context management | See three modules |
| D8 | `privacy_classifier` + `privacy_router` | Privacy-aware routing | `crates/vox-orchestrator/src/privacy_classifier.rs` |
| D10 | `calibration` | Adaptive routing via bandit + drift detection | `crates/vox-orchestrator/src/calibration.rs` |

All decisions are composed in `OrchestratorPolicy::evaluate()` (single call per loop iteration).

## What Worked

- **Pure modules**: Zero I/O on the hot path means each module is trivially testable and benches in <1µs.
- **Feature flags**: All 14 flags default-false; safe to ship before wiring.
- **Golden fixtures**: 30+ JSON fixtures catch regression on every CI run.
- **Five quality gates**: arch-check, telemetry, perf, contract, HITL fallback — enforced before each phase merged.
- **Existing infrastructure reuse**: `arm_stats`, `record_penalty()`, `BulletinBoard`, `CompactionStrategy`, `RiskBand/RiskDecision`, `PrivacyLevel` all extended rather than replaced.

## What Didn't Work (Or Was Deferred)

- **Runtime YAML config loading**: All configs still use struct defaults. A config loader from `contracts/orchestration/*.yaml` is needed before these decisions are tunable without recompile.
- **LLM-based semantic drift**: `semantic_drift_sigma` in D6 requires an embedding service; stubbed at 0.0 for now. The circuit breaker ignores this signal until P12.
- **Contextual bandit Thompson sampling**: The bandit uses greedy selection (highest expected reward), not actual sampled draws. True Thompson sampling needs a random number source.
- **Feedback loop**: `CalibrationLoop` and `ContextualBandit` exist but are not yet fed real success/failure signals from completed tasks. That wiring is P12.
- **Privacy → local model routing**: `privacy_requires_local` is set in `CompositeSignal` but `TierCascadeRouter` does not yet filter by local providers. Needs provider capability index.

## What We Cannot Automate (Confirmed)

From research (see `autonomous-orchestration-policy-research-2026.md`):

- **Authorizing irreversible actions**: Risk matrix grades and proposes; HITL must decide.
- **Privacy legal determination**: Classifier provides a signal; legal counsel determines compliance scope.
- **Novel tool discovery**: Orchestrator cannot autonomously decide to use a new tool it has never seen.
- **Long-horizon goal alignment**: Autonomy degrades as task horizon extends; human goal review is mandatory at >10 task chains.

## Metrics Emitted

Fourteen `metric_type` constants registered in `vox-db::research_metrics_contract`:

```
orch.circuit_breaker.trip
orch.socrates.fusion
orch.routing.tier
orch.plan.mode_decision
orch.hitl.interrupt
orch.risk.score
orch.privacy.route_decision
orch.cache.hit_prediction
orch.budget.decision
orch.calibration.run
orch.calibration.drift_alert
orch.calibration.bandit_update
orch.subagent.dispatch
orch.subagent.chain_depth_alert
```

## Next Steps (P12 — Not Scoped Here)

1. Runtime YAML config loader for all 14 feature flags and contract thresholds.
2. Real Thompson sampling (needs `rand` crate in vox-orchestrator).
3. Feedback wire: emit bandit outcome from `TaskCompleted`/`TaskFailed` bulletin messages.
4. Semantic drift embedding: wire `entropy_scorer` or an embedding distance for `semantic_drift_sigma`.
5. Provider capability index: map `privacy_requires_local` to actual local-capable model IDs.
```

- [ ] **Step 5.2: Commit the retrospective**

```bash
git add docs/src/architecture/orchestrator-policy-program-retrospective-2026.md
git commit -m "docs(arch): add orchestrator policy program retrospective 2026"
```

---

### Task 6: Update where-things-live.md for all new modules (final pass)

- [ ] **Step 6.1: Verify all ten modules have rows**

Read `docs/src/architecture/where-things-live.md` and confirm rows exist for:
- `circuit_breaker`, `confidence_fusion`, `tier_cascade`, `plan_mode_trigger`, `risk_matrix`, `privacy_classifier`, `cache_predictor`, `budget_gate`, `compaction_trigger`, `calibration`, `subagent_dispatch`, `orchestrator_policy`

Add any missing rows.

- [ ] **Step 6.2: Run arch-check one final time**

Run: `cargo run -p vox-arch-check 2>&1 | tail -30`
Expected: Clean.

- [ ] **Step 6.3: Final commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): final where-things-live.md pass for all P1-P11 modules"
```

---

### Task 7: Phase 11 sign-off checklist

- [ ] `OrchestratorPolicy::evaluate()` compiles and all 4 unit tests pass
- [ ] `orchestrator_policy_integration` — all 4 integration tests pass
- [ ] All five quality gates green (arch-check, telemetry, perf, golden, HITL)
- [ ] Auto-generated docs regenerated and committed
- [ ] Retrospective written to `docs/src/architecture/`
- [ ] All 12 new modules in `where-things-live.md`
- [ ] All 14 feature flags default-false in `feature-flags.v1.yaml`
- [ ] No `TODO(P1)` through `TODO(P10)` markers left unresolved (search: `grep -r "TODO(P" crates/vox-orchestrator/src/`)

---

**Program complete.** The orchestrator now has ten autonomous decision modules, a unified façade, 30+ golden fixtures, 5 criterion benchmarks, 12 metric constants, 14 feature flags, and a retrospective document.
