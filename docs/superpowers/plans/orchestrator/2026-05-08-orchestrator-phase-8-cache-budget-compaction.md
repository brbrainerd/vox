# Orchestrator Phase 8: Cache + Budget + Compaction (D7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three autonomous context-management decision makers that work together: (1) a cache-hit predictor that prefers providers with warm prefix caches, (2) a budget gate that halts or downgrades when token budgets are exhausted, and (3) a compaction trigger that autonomously selects compaction strategy based on context utilization.

**Architecture:** Three pure modules — `cache_predictor.rs`, `budget_gate.rs`, and `compaction_trigger.rs` — plus integration wiring into the existing `CompactionConfig` and `ModelRegistry`. The existing `CompactionStrategy` (`compaction.rs`) is reused directly; the trigger only selects which strategy to apply. Budget signals feed into `CompositeSignal::budget_exhausted` (declared in P4).

**Tech Stack:** Rust, existing `CompactionStrategy`, `CompactionConfig`, `context_utilization_pct` from vox-db schema, `METRIC_TYPE_CACHE_HIT_PREDICTION`, `METRIC_TYPE_BUDGET_DECISION`, feature flags `vox.orchestrator.cache_aware_routing.enabled`, `vox.orchestrator.tenant_budget.enabled`, `vox.orchestrator.compaction_5layer.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/cache_predictor.rs` |
| Create | `crates/vox-orchestrator/src/budget_gate.rs` |
| Create | `crates/vox-orchestrator/src/compaction_trigger.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — three pub mod lines |
| Create | `crates/vox-orchestrator/tests/cache_budget_compaction_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/cache/hit_prediction.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/cache/miss_prediction.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/budget/exhausted.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/compaction/aggressive_trigger.json` |
| Modify | `docs/src/architecture/where-things-live.md` — add three rows |

---

### Task 1: Cache predictor

**Files:**
- Create: `crates/vox-orchestrator/src/cache_predictor.rs`

- [ ] **Step 1.1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_overlap_predicts_cache_hit() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 800,
            total_context_tokens: 1000,
            provider_has_prefix_cache: true,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Hit);
    }

    #[test]
    fn no_overlap_predicts_miss() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 0,
            total_context_tokens: 1000,
            provider_has_prefix_cache: true,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Miss);
    }

    #[test]
    fn no_provider_cache_always_miss() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 900,
            total_context_tokens: 1000,
            provider_has_prefix_cache: false,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Miss);
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator cache_predictor 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 1.3: Write the module**

Create `crates/vox-orchestrator/src/cache_predictor.rs`:

```rust
//! Prompt cache hit predictor for provider routing (D7).
//!
//! Predicts whether the current context prefix overlaps enough with a
//! provider's cached prefix to be cost-effective.

use serde::{Deserialize, Serialize};

/// Input signals for cache prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSignal {
    /// Number of tokens in the current prompt that match the provider's cached prefix.
    pub prefix_overlap_tokens: u64,
    /// Total tokens in the current context window.
    pub total_context_tokens: u64,
    /// Whether this provider supports prefix caching at all.
    pub provider_has_prefix_cache: bool,
}

/// Cache prediction outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePrediction {
    Hit,
    Miss,
}

/// Configuration thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePredictorConfig {
    /// Fraction of context tokens that must overlap to predict a hit.
    pub hit_threshold_fraction: f64,
}

impl Default for CachePredictorConfig {
    fn default() -> Self {
        Self {
            hit_threshold_fraction: 0.70,
        }
    }
}

/// Pure cache predictor.
pub struct CachePredictor {
    config: CachePredictorConfig,
}

impl CachePredictor {
    pub fn new(config: CachePredictorConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn predict(&self, signal: &CacheSignal) -> CachePrediction {
        if !signal.provider_has_prefix_cache {
            return CachePrediction::Miss;
        }
        if signal.total_context_tokens == 0 {
            return CachePrediction::Miss;
        }
        let overlap = signal.prefix_overlap_tokens as f64 / signal.total_context_tokens as f64;
        if overlap >= self.config.hit_threshold_fraction {
            CachePrediction::Hit
        } else {
            CachePrediction::Miss
        }
    }
}

/// Metric payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePredictionEvent {
    pub metric_type: &'static str,
    pub prediction: CachePrediction,
    pub overlap_fraction: f64,
}

impl CachePredictionEvent {
    pub fn new(prediction: CachePrediction, signal: &CacheSignal) -> Self {
        let frac = if signal.total_context_tokens == 0 {
            0.0
        } else {
            signal.prefix_overlap_tokens as f64 / signal.total_context_tokens as f64
        };
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CACHE_HIT_PREDICTION,
            prediction,
            overlap_fraction: frac,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_overlap_predicts_hit() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 800,
            total_context_tokens: 1000,
            provider_has_prefix_cache: true,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Hit);
    }

    #[test]
    fn no_overlap_predicts_miss() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 0,
            total_context_tokens: 1000,
            provider_has_prefix_cache: true,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Miss);
    }

    #[test]
    fn no_provider_cache_always_miss() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 900,
            total_context_tokens: 1000,
            provider_has_prefix_cache: false,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Miss);
    }

    #[test]
    fn zero_context_is_miss() {
        let predictor = CachePredictor::new(CachePredictorConfig::default());
        let signal = CacheSignal {
            prefix_overlap_tokens: 0,
            total_context_tokens: 0,
            provider_has_prefix_cache: true,
        };
        assert_eq!(predictor.predict(&signal), CachePrediction::Miss);
    }

    #[test]
    fn cache_prediction_event_metric_type() {
        let signal = CacheSignal {
            prefix_overlap_tokens: 800,
            total_context_tokens: 1000,
            provider_has_prefix_cache: true,
        };
        let event = CachePredictionEvent::new(CachePrediction::Hit, &signal);
        assert_eq!(event.metric_type, "orch.cache.hit_prediction");
    }
}
```

- [ ] **Step 1.4: Register and run**

Add `pub mod cache_predictor;` to `lib.rs`.

Run: `cargo test -p vox-orchestrator cache_predictor`
Expected: All 5 tests pass.

- [ ] **Step 1.5: Commit**

```bash
git add crates/vox-orchestrator/src/cache_predictor.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add CachePredictor for prefix-cache routing (D7)"
```

---

### Task 2: Budget gate

**Files:**
- Create: `crates/vox-orchestrator/src/budget_gate.rs`

- [ ] **Step 2.1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_budget_allows() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 50_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.50,
            cost_limit_usd: 5.00,
        };
        assert_eq!(gate.evaluate(&status), BudgetDecision::Proceed);
    }

    #[test]
    fn token_exhausted_triggers_halt() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 96_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.10,
            cost_limit_usd: 5.00,
        };
        // 96% > 95% hard stop threshold
        assert_eq!(gate.evaluate(&status), BudgetDecision::Halt);
    }

    #[test]
    fn near_budget_triggers_downgrade() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 85_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.10,
            cost_limit_usd: 5.00,
        };
        // 85% is in [80%, 95%) → Downgrade
        assert_eq!(gate.evaluate(&status), BudgetDecision::Downgrade);
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator budget_gate 2>&1 | head -10`

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/budget_gate.rs`:

```rust
//! Token and cost budget gate for orchestrator context management (D7).

use serde::{Deserialize, Serialize};

/// Current budget consumption snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub tokens_used: u64,
    pub tokens_limit: u64,
    pub cost_used_usd: f64,
    pub cost_limit_usd: f64,
}

impl BudgetStatus {
    pub fn token_fraction(&self) -> f64 {
        if self.tokens_limit == 0 {
            return 1.0;
        }
        self.tokens_used as f64 / self.tokens_limit as f64
    }

    pub fn cost_fraction(&self) -> f64 {
        if self.cost_limit_usd <= 0.0 {
            return 1.0;
        }
        self.cost_used_usd / self.cost_limit_usd
    }
}

/// Budget decision for the current iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetDecision {
    /// Continue normally.
    Proceed,
    /// Downgrade to Economy tier; warn user.
    Downgrade,
    /// Halt execution; budget exhausted.
    Halt,
}

/// Thresholds for budget gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetGateConfig {
    /// Fraction above which downgrade to economy tier is triggered.
    pub downgrade_fraction: f64,
    /// Fraction above which execution halts.
    pub halt_fraction: f64,
}

impl Default for BudgetGateConfig {
    fn default() -> Self {
        Self {
            downgrade_fraction: 0.80,
            halt_fraction: 0.95,
        }
    }
}

pub struct BudgetGate {
    config: BudgetGateConfig,
}

impl BudgetGate {
    pub fn new(config: BudgetGateConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn evaluate(&self, status: &BudgetStatus) -> BudgetDecision {
        let fraction = status.token_fraction().max(status.cost_fraction());
        if fraction >= self.config.halt_fraction {
            BudgetDecision::Halt
        } else if fraction >= self.config.downgrade_fraction {
            BudgetDecision::Downgrade
        } else {
            BudgetDecision::Proceed
        }
    }

    /// Returns true if budget is exhausted (maps to CompositeSignal::budget_exhausted).
    #[must_use]
    #[inline]
    pub fn is_exhausted(&self, status: &BudgetStatus) -> bool {
        self.evaluate(status) == BudgetDecision::Halt
    }
}

/// Metric payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDecisionEvent {
    pub metric_type: &'static str,
    pub decision: BudgetDecision,
    pub token_fraction: f64,
}

impl BudgetDecisionEvent {
    pub fn new(decision: BudgetDecision, status: &BudgetStatus) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_BUDGET_DECISION,
            decision,
            token_fraction: status.token_fraction(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_budget_allows() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 50_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.50,
            cost_limit_usd: 5.00,
        };
        assert_eq!(gate.evaluate(&status), BudgetDecision::Proceed);
    }

    #[test]
    fn token_exhausted_triggers_halt() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 96_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.10,
            cost_limit_usd: 5.00,
        };
        assert_eq!(gate.evaluate(&status), BudgetDecision::Halt);
    }

    #[test]
    fn near_budget_triggers_downgrade() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 85_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.10,
            cost_limit_usd: 5.00,
        };
        assert_eq!(gate.evaluate(&status), BudgetDecision::Downgrade);
    }

    #[test]
    fn cost_fraction_drives_halt_when_higher() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 10_000,
            tokens_limit: 100_000,  // 10% token use
            cost_used_usd: 4.90,
            cost_limit_usd: 5.00,   // 98% cost use → Halt
        };
        assert_eq!(gate.evaluate(&status), BudgetDecision::Halt);
    }

    #[test]
    fn is_exhausted_true_at_halt() {
        let gate = BudgetGate::new(BudgetGateConfig::default());
        let status = BudgetStatus {
            tokens_used: 96_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.0,
            cost_limit_usd: 5.0,
        };
        assert!(gate.is_exhausted(&status));
    }

    #[test]
    fn budget_decision_event_metric_type() {
        let status = BudgetStatus {
            tokens_used: 96_000,
            tokens_limit: 100_000,
            cost_used_usd: 0.0,
            cost_limit_usd: 5.0,
        };
        let event = BudgetDecisionEvent::new(BudgetDecision::Halt, &status);
        assert_eq!(event.metric_type, "orch.budget.decision");
    }
}
```

- [ ] **Step 2.4: Register and run**

Add `pub mod budget_gate;` to `lib.rs`.

Run: `cargo test -p vox-orchestrator budget_gate`
Expected: All 6 tests pass.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-orchestrator/src/budget_gate.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add BudgetGate for D7 token and cost budget control"
```

---

### Task 3: Compaction trigger

**Files:**
- Create: `crates/vox-orchestrator/src/compaction_trigger.rs`

- [ ] **Step 3.1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::compaction::CompactionStrategy;

    #[test]
    fn low_utilization_suggests_conservative() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(
            trigger.suggest_strategy(0.40),
            CompactionStrategy::Conservative
        );
    }

    #[test]
    fn medium_utilization_suggests_balanced() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(
            trigger.suggest_strategy(0.70),
            CompactionStrategy::Balanced
        );
    }

    #[test]
    fn high_utilization_suggests_aggressive() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(
            trigger.suggest_strategy(0.92),
            CompactionStrategy::Aggressive
        );
    }
}
```

- [ ] **Step 3.2: Write the module**

Create `crates/vox-orchestrator/src/compaction_trigger.rs`:

```rust
//! Autonomous compaction strategy selector (D7).
//!
//! Selects CompactionStrategy from context utilization percentage.
//! Maps into existing CompactionConfig::strategy field.

use serde::{Deserialize, Serialize};
use crate::compaction::CompactionStrategy;

/// Thresholds for compaction strategy selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionTriggerConfig {
    /// Below this utilization → Conservative.
    pub conservative_below: f64,
    /// Below this utilization (and >= conservative) → Balanced.
    pub balanced_below: f64,
    // At or above balanced_below → Aggressive
}

impl Default for CompactionTriggerConfig {
    fn default() -> Self {
        Self {
            conservative_below: 0.60,
            balanced_below: 0.85,
        }
    }
}

pub struct CompactionTrigger {
    config: CompactionTriggerConfig,
}

impl CompactionTrigger {
    pub fn new(config: CompactionTriggerConfig) -> Self {
        Self { config }
    }

    /// Suggest a compaction strategy for the given context utilization fraction (0.0–1.0).
    #[must_use]
    pub fn suggest_strategy(&self, utilization: f64) -> CompactionStrategy {
        if utilization < self.config.conservative_below {
            CompactionStrategy::Conservative
        } else if utilization < self.config.balanced_below {
            CompactionStrategy::Balanced
        } else {
            CompactionStrategy::Aggressive
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_utilization_suggests_conservative() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(trigger.suggest_strategy(0.40), CompactionStrategy::Conservative);
    }

    #[test]
    fn medium_utilization_suggests_balanced() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(trigger.suggest_strategy(0.70), CompactionStrategy::Balanced);
    }

    #[test]
    fn high_utilization_suggests_aggressive() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(trigger.suggest_strategy(0.92), CompactionStrategy::Aggressive);
    }

    #[test]
    fn exactly_at_conservative_boundary_is_balanced() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(trigger.suggest_strategy(0.60), CompactionStrategy::Balanced);
    }

    #[test]
    fn exactly_at_balanced_boundary_is_aggressive() {
        let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
        assert_eq!(trigger.suggest_strategy(0.85), CompactionStrategy::Aggressive);
    }
}
```

- [ ] **Step 3.3: Register and run**

Add `pub mod compaction_trigger;` to `lib.rs`.

Run: `cargo test -p vox-orchestrator compaction_trigger`
Expected: All 5 tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator/src/compaction_trigger.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add CompactionTrigger for autonomous strategy selection (D7)"
```

---

### Task 4: Golden fixtures and integration tests

**Files:**
- Create: fixtures and integration test file

- [ ] **Step 4.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/cache/hit_prediction.json`:
```json
{
  "signal": {
    "prefix_overlap_tokens": 800,
    "total_context_tokens": 1000,
    "provider_has_prefix_cache": true
  },
  "expected_prediction": "Hit"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/cache/miss_prediction.json`:
```json
{
  "signal": {
    "prefix_overlap_tokens": 0,
    "total_context_tokens": 1000,
    "provider_has_prefix_cache": true
  },
  "expected_prediction": "Miss"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/budget/exhausted.json`:
```json
{
  "status": {
    "tokens_used": 96000,
    "tokens_limit": 100000,
    "cost_used_usd": 0.10,
    "cost_limit_usd": 5.00
  },
  "expected_decision": "Halt",
  "expected_exhausted": true
}
```

`crates/vox-orchestrator-test-helpers/fixtures/compaction/aggressive_trigger.json`:
```json
{
  "utilization": 0.92,
  "expected_strategy": "Aggressive"
}
```

- [ ] **Step 4.2: Write integration tests**

Create `crates/vox-orchestrator/tests/cache_budget_compaction_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::cache_predictor::{CachePredictor, CachePredictorConfig, CachePrediction, CacheSignal};
use vox_orchestrator::budget_gate::{BudgetDecision, BudgetGate, BudgetGateConfig, BudgetStatus};
use vox_orchestrator::compaction::CompactionStrategy;
use vox_orchestrator::compaction_trigger::{CompactionTrigger, CompactionTriggerConfig};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct CacheFixture {
    signal: CacheSignal,
    expected_prediction: CachePrediction,
}

#[derive(Deserialize)]
struct BudgetFixture {
    status: BudgetStatus,
    expected_decision: BudgetDecision,
    expected_exhausted: bool,
}

#[derive(Deserialize)]
struct CompactionFixture {
    utilization: f64,
    expected_strategy: CompactionStrategy,
}

#[test]
fn golden_cache_hit() {
    let f: CacheFixture = load_golden_fixture("cache/hit_prediction.json").unwrap();
    let predictor = CachePredictor::new(CachePredictorConfig::default());
    assert_eq!(predictor.predict(&f.signal), f.expected_prediction);
}

#[test]
fn golden_cache_miss() {
    let f: CacheFixture = load_golden_fixture("cache/miss_prediction.json").unwrap();
    let predictor = CachePredictor::new(CachePredictorConfig::default());
    assert_eq!(predictor.predict(&f.signal), f.expected_prediction);
}

#[test]
fn golden_budget_exhausted() {
    let f: BudgetFixture = load_golden_fixture("budget/exhausted.json").unwrap();
    let gate = BudgetGate::new(BudgetGateConfig::default());
    assert_eq!(gate.evaluate(&f.status), f.expected_decision);
    assert_eq!(gate.is_exhausted(&f.status), f.expected_exhausted);
}

#[test]
fn golden_compaction_aggressive() {
    let f: CompactionFixture = load_golden_fixture("compaction/aggressive_trigger.json").unwrap();
    let trigger = CompactionTrigger::new(CompactionTriggerConfig::default());
    assert_eq!(trigger.suggest_strategy(f.utilization), f.expected_strategy);
}
```

- [ ] **Step 4.3: Run all integration tests**

Run: `cargo test --test cache_budget_compaction_integration`
Expected: 4 golden tests pass.

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/cache/ \
        crates/vox-orchestrator-test-helpers/fixtures/budget/ \
        crates/vox-orchestrator-test-helpers/fixtures/compaction/ \
        crates/vox-orchestrator/tests/cache_budget_compaction_integration.rs
git commit -m "test(orchestrator): golden fixtures for cache/budget/compaction D7 modules"
```

---

### Task 5: Update where-things-live.md and arch-check

- [ ] **Step 5.1: Add three rows**

```
| `cache_predictor` | Prefix cache hit predictor for provider routing (D7) | `crates/vox-orchestrator/src/cache_predictor.rs` |
| `budget_gate` | Token and cost budget gate (D7) | `crates/vox-orchestrator/src/budget_gate.rs` |
| `compaction_trigger` | Autonomous compaction strategy selector (D7) | `crates/vox-orchestrator/src/compaction_trigger.rs` |
```

- [ ] **Step 5.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`

- [ ] **Step 5.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register cache_predictor, budget_gate, compaction_trigger"
```

---

### Task 6: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** All three metric types tested (cache, budget, compaction uses CompactionStrategy derive)
- [ ] **G3** No perf bench required (pure arithmetic, <100ns each)
- [ ] **G4** `budget_exhausted=true` → `CompositeSignal` triggers Economy tier in P4
- [ ] **G5** Aggressive compaction triggered at ≥85% utilization

---

**Phase 8 sign-off:** 6 tasks complete, 16+ unit tests + 4 golden fixtures, `cargo build` clean.
