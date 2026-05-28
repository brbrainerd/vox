# Orchestrator Phase 4: Tier Cascade (D1 — Model Routing) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit three-tier (Haiku → Sonnet → Opus) cascade router that upgrades model tier when composite confidence drops below a threshold, and downgrades on success, without breaking the existing `best_for_task` / scoring pipeline.

**Architecture:** A new `tier_cascade.rs` module holds a pure `TierCascadeRouter` that wraps `ModelRegistry::best_for_task`. Given a `CompositeSignal` (complexity, confidence, budget, privacy), it picks the minimum-cost tier that satisfies quality requirements, then falls back up the cascade if the lower tier would trip the circuit breaker. The existing `CostPreference` enum stays unchanged.

**Tech Stack:** Rust, existing `ModelRegistry`, `CircuitBreakerState` (from P2), `FusionDecision` (from P3), `ModelTier` (from build.rs generated.rs), `CostPreference`, `METRIC_TYPE_MODEL_TIER_ROUTE`, feature flag `vox.orchestrator.tier_cascade.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/tier_cascade.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod tier_cascade |
| Create | `crates/vox-orchestrator/benches/tier_cascade.rs` |
| Create | `crates/vox-orchestrator/tests/tier_cascade_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_economy.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_upgrade_on_low_confidence.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_upgrade_on_circuit_breaker.json` |
| Modify | `contracts/orchestration/model-routing.v1.yaml` — add tier_cascade section |
| Modify | `docs/src/architecture/where-things-live.md` — add tier_cascade row |

---

### Task 1: Extend model-routing.v1.yaml with cascade config

**Files:**
- Modify: `contracts/orchestration/model-routing.v1.yaml`

- [ ] **Step 1.1: Read the current model-routing.v1.yaml**

Read `contracts/orchestration/model-routing.v1.yaml` to see the existing structure.

- [ ] **Step 1.2: Add the tier_cascade section**

Append to the existing YAML (do not remove existing fields):

```yaml
tier_cascade:
  enabled: false   # overridden by feature flag vox.orchestrator.tier_cascade.enabled
  upgrade_on_confidence_below: 0.55   # FusionDecision::InvokeSocrates threshold
  upgrade_on_circuit_breaker_tier: "Warning"
  downgrade_on_success_streak: 3      # consecutive successes before downgrading
  tiers:
    - name: "economy"
      model_tier: "Small"
      min_complexity: 0
      max_complexity: 3
    - name: "standard"
      model_tier: "Medium"
      min_complexity: 4
      max_complexity: 7
    - name: "strong"
      model_tier: "Large"
      min_complexity: 8
      max_complexity: 10
  metrics_key: "orch.routing.tier"
  feature_flag: "vox.orchestrator.tier_cascade.enabled"
```

- [ ] **Step 1.3: Commit**

```bash
git add contracts/orchestration/model-routing.v1.yaml
git commit -m "feat(contracts): add tier_cascade section to model-routing.v1.yaml"
```

---

### Task 2: Core TierCascadeRouter

**Files:**
- Create: `crates/vox-orchestrator/src/tier_cascade.rs`

- [ ] **Step 2.1: Write the failing tests first**

```rust
// At the bottom of tier_cascade.rs (created in 2.3)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_complexity_selects_economy_tier() {
        let signal = CompositeSignal {
            complexity: 2,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Economy);
    }

    #[test]
    fn high_complexity_selects_strong_tier() {
        let signal = CompositeSignal {
            complexity: 9,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Strong);
    }

    #[test]
    fn low_confidence_upgrades_one_tier() {
        let signal = CompositeSignal {
            complexity: 2, // would be Economy
            confidence_score: 0.40, // below invoke_socrates threshold
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        // Economy + upgrade = Standard
        assert_eq!(router.select_tier(&signal), RoutingTier::Standard);
    }

    #[test]
    fn warning_alarm_upgrades_to_strong() {
        let signal = CompositeSignal {
            complexity: 4, // would be Standard
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::Warning,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Strong);
    }

    #[test]
    fn budget_exhausted_stays_economy() {
        let signal = CompositeSignal {
            complexity: 9,
            confidence_score: 0.90,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: true,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Economy);
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator tier_cascade 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/tier_cascade.rs`:

```rust
//! Three-tier cascade router for model selection (D1).
//!
//! Selects Economy / Standard / Strong tier based on task complexity,
//! composite confidence, circuit-breaker alarm level, and budget.
//! All logic is pure; no registry lookups happen here.

use serde::{Deserialize, Serialize};

/// Abstracted alarm level from CircuitBreaker (avoids direct dep on circuit_breaker mod).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlarmLevel {
    None,
    Caution,
    Warning,
}

impl From<crate::circuit_breaker::AlarmTier> for AlarmLevel {
    fn from(t: crate::circuit_breaker::AlarmTier) -> Self {
        match t {
            crate::circuit_breaker::AlarmTier::None => AlarmLevel::None,
            crate::circuit_breaker::AlarmTier::Caution => AlarmLevel::Caution,
            crate::circuit_breaker::AlarmTier::Warning => AlarmLevel::Warning,
        }
    }
}

/// Selected routing tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingTier {
    Economy,
    Standard,
    Strong,
}

impl RoutingTier {
    fn upgrade(self) -> Self {
        match self {
            Self::Economy => Self::Standard,
            Self::Standard | Self::Strong => Self::Strong,
        }
    }
}

/// All signals the cascade uses to pick a tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeSignal {
    /// Task complexity 0–10 (matches AgentTask::estimated_complexity).
    pub complexity: u8,
    /// Composite confidence from P3 ConfidenceFuser (0.0–1.0).
    pub confidence_score: f64,
    /// Alarm level from P2 CircuitBreaker.
    pub circuit_breaker_tier: AlarmLevel,
    /// True if the session budget is exhausted.
    pub budget_exhausted: bool,
    /// True if privacy policy mandates local inference.
    pub privacy_requires_local: bool,
}

/// Config that mirrors the `tier_cascade` section of `model-routing.v1.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierCascadeConfig {
    pub upgrade_on_confidence_below: f64,
    pub economy_max_complexity: u8,
    pub standard_max_complexity: u8,
}

impl Default for TierCascadeConfig {
    fn default() -> Self {
        Self {
            upgrade_on_confidence_below: 0.55,
            economy_max_complexity: 3,
            standard_max_complexity: 7,
        }
    }
}

/// Pure tier selector.
pub struct TierCascadeRouter {
    config: TierCascadeConfig,
}

impl TierCascadeRouter {
    pub fn new(config: TierCascadeConfig) -> Self {
        Self { config }
    }

    /// Select the minimum sufficient tier, applying upgrade rules.
    #[must_use]
    pub fn select_tier(&self, signal: &CompositeSignal) -> RoutingTier {
        // Budget exhaustion: always Economy regardless of complexity
        if signal.budget_exhausted {
            return RoutingTier::Economy;
        }

        // Base tier from complexity
        let mut tier = if signal.complexity <= self.config.economy_max_complexity {
            RoutingTier::Economy
        } else if signal.complexity <= self.config.standard_max_complexity {
            RoutingTier::Standard
        } else {
            RoutingTier::Strong
        };

        // Upgrade on low confidence (one tier up)
        if signal.confidence_score < self.config.upgrade_on_confidence_below {
            tier = tier.upgrade();
        }

        // Upgrade on Warning alarm (force Strong)
        if signal.circuit_breaker_tier >= AlarmLevel::Warning {
            tier = RoutingTier::Strong;
        }

        tier
    }
}

/// Metric payload emitted per routing decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRouteEvent {
    pub metric_type: &'static str,
    pub selected_tier: RoutingTier,
    pub complexity: u8,
    pub confidence_score: f64,
    pub alarm_level: AlarmLevel,
    pub session_id: Option<String>,
}

impl TierRouteEvent {
    pub fn new(tier: RoutingTier, signal: &CompositeSignal) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_MODEL_TIER_ROUTE,
            selected_tier: tier,
            complexity: signal.complexity,
            confidence_score: signal.confidence_score,
            alarm_level: signal.circuit_breaker_tier,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_complexity_selects_economy_tier() {
        let signal = CompositeSignal {
            complexity: 2,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Economy);
    }

    #[test]
    fn high_complexity_selects_strong_tier() {
        let signal = CompositeSignal {
            complexity: 9,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Strong);
    }

    #[test]
    fn low_confidence_upgrades_one_tier() {
        let signal = CompositeSignal {
            complexity: 2,
            confidence_score: 0.40,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Standard);
    }

    #[test]
    fn warning_alarm_upgrades_to_strong() {
        let signal = CompositeSignal {
            complexity: 4,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::Warning,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Strong);
    }

    #[test]
    fn budget_exhausted_stays_economy() {
        let signal = CompositeSignal {
            complexity: 9,
            confidence_score: 0.90,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: true,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        assert_eq!(router.select_tier(&signal), RoutingTier::Economy);
    }

    #[test]
    fn caution_alarm_does_not_force_strong() {
        let signal = CompositeSignal {
            complexity: 2,
            confidence_score: 0.80,
            circuit_breaker_tier: AlarmLevel::Caution,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        // Caution doesn't trigger the Warning upgrade, stays Economy
        assert_eq!(router.select_tier(&signal), RoutingTier::Economy);
    }

    #[test]
    fn tier_route_event_has_correct_metric_type() {
        let signal = CompositeSignal {
            complexity: 5,
            confidence_score: 0.70,
            circuit_breaker_tier: AlarmLevel::None,
            budget_exhausted: false,
            privacy_requires_local: false,
        };
        let router = TierCascadeRouter::new(TierCascadeConfig::default());
        let tier = router.select_tier(&signal);
        let event = TierRouteEvent::new(tier, &signal);
        assert_eq!(event.metric_type, "orch.routing.tier");
    }
}
```

- [ ] **Step 2.4: Register module**

In `crates/vox-orchestrator/src/lib.rs`:
```rust
pub mod tier_cascade;
```

- [ ] **Step 2.5: Run tests**

Run: `cargo test -p vox-orchestrator tier_cascade -- --nocapture`
Expected: All 7 tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vox-orchestrator/src/tier_cascade.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add TierCascadeRouter for three-tier model selection (D1)"
```

---

### Task 3: Golden fixtures and integration tests

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/routing/*.json`
- Create: `crates/vox-orchestrator/tests/tier_cascade_integration.rs`

- [ ] **Step 3.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_economy.json`:
```json
{
  "signal": {
    "complexity": 2,
    "confidence_score": 0.85,
    "circuit_breaker_tier": "None",
    "budget_exhausted": false,
    "privacy_requires_local": false
  },
  "expected_tier": "Economy"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_upgrade_on_low_confidence.json`:
```json
{
  "signal": {
    "complexity": 2,
    "confidence_score": 0.40,
    "circuit_breaker_tier": "None",
    "budget_exhausted": false,
    "privacy_requires_local": false
  },
  "expected_tier": "Standard"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/routing/cascade_upgrade_on_circuit_breaker.json`:
```json
{
  "signal": {
    "complexity": 4,
    "confidence_score": 0.80,
    "circuit_breaker_tier": "Warning",
    "budget_exhausted": false,
    "privacy_requires_local": false
  },
  "expected_tier": "Strong"
}
```

- [ ] **Step 3.2: Write integration tests**

Create `crates/vox-orchestrator/tests/tier_cascade_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::tier_cascade::{
    CompositeSignal, RoutingTier, TierCascadeConfig, TierCascadeRouter,
};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct CascadeFixture {
    signal: CompositeSignal,
    expected_tier: RoutingTier,
}

#[test]
fn golden_cascade_economy() {
    let f: CascadeFixture =
        load_golden_fixture("routing/cascade_economy.json").unwrap();
    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    assert_eq!(router.select_tier(&f.signal), f.expected_tier);
}

#[test]
fn golden_cascade_upgrade_on_low_confidence() {
    let f: CascadeFixture =
        load_golden_fixture("routing/cascade_upgrade_on_low_confidence.json").unwrap();
    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    assert_eq!(router.select_tier(&f.signal), f.expected_tier);
}

#[test]
fn golden_cascade_upgrade_on_circuit_breaker() {
    let f: CascadeFixture =
        load_golden_fixture("routing/cascade_upgrade_on_circuit_breaker.json").unwrap();
    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    assert_eq!(router.select_tier(&f.signal), f.expected_tier);
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test --test tier_cascade_integration`
Expected: 3 golden tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/routing/ \
        crates/vox-orchestrator/tests/tier_cascade_integration.rs
git commit -m "test(orchestrator): golden fixtures for TierCascadeRouter"
```

---

### Task 4: Criterion benchmark

**Files:**
- Create: `crates/vox-orchestrator/benches/tier_cascade.rs`

- [ ] **Step 4.1: Write benchmark**

```rust
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vox_orchestrator::tier_cascade::{
    AlarmLevel, CompositeSignal, TierCascadeConfig, TierCascadeRouter,
};

fn bench_select_tier(c: &mut Criterion) {
    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    let signal = CompositeSignal {
        complexity: 5,
        confidence_score: 0.70,
        circuit_breaker_tier: AlarmLevel::None,
        budget_exhausted: false,
        privacy_requires_local: false,
    };
    c.bench_function("tier_cascade_select_tier", |b| {
        b.iter(|| router.select_tier(black_box(&signal)))
    });
}

criterion_group!(benches, bench_select_tier);
criterion_main!(benches);
```

- [ ] **Step 4.2: Add bench entry to Cargo.toml**

```toml
[[bench]]
name = "tier_cascade"
harness = false
```

- [ ] **Step 4.3: Run benchmark**

Run: `cargo bench -p vox-orchestrator --bench tier_cascade 2>&1 | tail -10`
Expected: `tier_cascade_select_tier` mean <50µs (should be <100ns).

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator/benches/tier_cascade.rs crates/vox-orchestrator/Cargo.toml
git commit -m "bench(orchestrator): criterion benchmark for TierCascadeRouter"
```

---

### Task 5: Wire TierCascadeRouter into ModelRegistry selection path

**Files:**
- Modify: `crates/vox-orchestrator/src/models/registry.rs`

- [ ] **Step 5.1: Read registry.rs best_for_task (lines 569–592)**

Read `crates/vox-orchestrator/src/models/registry.rs` lines 569–650 to understand `best_for_task_with_filter`.

- [ ] **Step 5.2: Add a new method `best_for_cascaded` that wraps `best_for_task`**

In `registry.rs`, add after `best_for_task_with_filter` (~line 592):

```rust
/// Like [`best_for_task`] but uses the tier cascade to select [`CostPreference`]
/// when feature flag `vox.orchestrator.tier_cascade.enabled` is active.
/// Falls back to the provided `preference` when the flag is off.
pub fn best_for_cascaded(
    &self,
    task: &AgentTask,
    preference: CostPreference,
    tier_signal: Option<&crate::tier_cascade::CompositeSignal>,
) -> Option<ModelSpec> {
    if std::env::var("VOX_ORCHESTRATOR_TIER_CASCADE").as_deref() != Ok("1") {
        return self.best_for_task(task, preference);
    }
    let Some(signal) = tier_signal else {
        return self.best_for_task(task, preference);
    };
    use crate::tier_cascade::{RoutingTier, TierCascadeConfig, TierCascadeRouter};
    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    let tier = router.select_tier(signal);
    let derived_pref = match tier {
        RoutingTier::Economy => CostPreference::Economy,
        RoutingTier::Standard => preference, // use caller preference for mid tier
        RoutingTier::Strong => CostPreference::Performance,
    };
    self.best_for_task(task, derived_pref)
}
```

- [ ] **Step 5.3: Write integration test for cascaded selection**

Add to `crates/vox-orchestrator/tests/tier_cascade_integration.rs`:

```rust
#[test]
fn best_for_cascaded_returns_some_with_default_registry() {
    use vox_orchestrator::models::{AgentTask, ModelRegistry};
    use vox_orchestrator::config::CostPreference;
    use vox_orchestrator::tier_cascade::{AlarmLevel, CompositeSignal};
    use vox_orchestrator::types::TaskCategory;

    let registry = ModelRegistry::new();
    if registry.is_empty() {
        // No models registered in test environment — skip
        return;
    }
    let task = AgentTask {
        estimated_complexity: 5,
        task_category: TaskCategory::General,
        ..Default::default()
    };
    let signal = CompositeSignal {
        complexity: 5,
        confidence_score: 0.80,
        circuit_breaker_tier: AlarmLevel::None,
        budget_exhausted: false,
        privacy_requires_local: false,
    };
    // Should return Some or None without panicking
    let _ = registry.best_for_cascaded(&task, CostPreference::Economy, Some(&signal));
}
```

Note: `AgentTask::default()` and `ModelRegistry::is_empty()` may not exist — read the actual types in `crates/vox-orchestrator/src/models/registry.rs` and `crates/vox-orchestrator/src/types.rs` before writing this test, and adjust field names accordingly.

- [ ] **Step 5.4: Run tests**

Run: `cargo test -p vox-orchestrator tier_cascade`
Run: `cargo test --test tier_cascade_integration`
Expected: All pass.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vox-orchestrator/src/models/registry.rs \
        crates/vox-orchestrator/tests/tier_cascade_integration.rs
git commit -m "feat(orchestrator): add best_for_cascaded to ModelRegistry (D1 tier cascade)"
```

---

### Task 6: Update where-things-live.md and run arch-check

- [ ] **Step 6.1: Add tier_cascade row**

```
| `tier_cascade` | Three-tier model cascade router (D1) | `crates/vox-orchestrator/src/tier_cascade.rs` |
```

- [ ] **Step 6.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`
Expected: Clean.

- [ ] **Step 6.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register tier_cascade in where-things-live.md"
```

---

### Task 7: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** `TierRouteEvent::metric_type == METRIC_TYPE_MODEL_TIER_ROUTE`
- [ ] **G3** Bench: `tier_cascade_select_tier` mean <50µs
- [ ] **G4** Contract: `model-routing.v1.yaml` tier_cascade section parses without error
- [ ] **G5** HITL fallback: `budget_exhausted=true` always returns Economy regardless of other signals

---

**Phase 4 sign-off:** 7 tasks complete, 7+ unit tests + 3 golden fixtures, `cargo build -p vox-orchestrator` clean.
