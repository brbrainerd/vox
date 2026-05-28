# Orchestrator Phase 5: Plan-Mode Trigger (D2 — Planning) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an autonomous decision function that determines when the orchestrator should operate in Plan-and-Execute mode (predictable tool graph, structured tasks) vs. ReAct mode (exploratory, single-hop). The trigger uses task complexity, dependency count, tool-hint count, and plan adequacy score as inputs.

**Architecture:** A new `plan_mode_trigger.rs` module holds a pure `PlanModeTrigger` struct. It takes a `PlanModeSignal` and returns `PlanModeDecision::{React, PlanAndExecute}`. The decision is wired into `vox-orchestrator-mcp`'s `plan_loop.rs` before the first LLM call. The existing `RubricScores::weighted_score()` feeds into the trigger when a prior plan adequacy report is available.

**Tech Stack:** Rust, existing `PlanAdequacyTask`, `RubricScores`, `PlanRefinementState`, `METRIC_TYPE_PLAN_MODE_DECISION`, feature flag `vox.orchestrator.plan_mode_trigger.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs` |
| Modify | `crates/vox-orchestrator/src/planning/mod.rs` — pub mod plan_mode_trigger |
| Modify | `crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs` — call trigger pre-loop |
| Create | `crates/vox-orchestrator/tests/plan_mode_trigger_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/plan-mode/react_simple.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/plan-mode/plan_complex.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/plan-mode/plan_many_deps.json` |
| Modify | `contracts/orchestration/plan-mode-trigger.v1.yaml` (if scaffold exists from P1, else create) |
| Modify | `docs/src/architecture/where-things-live.md` — add plan_mode_trigger row |

---

### Task 1: Write the contract

**Files:**
- Create/Modify: `contracts/orchestration/plan-mode-trigger.v1.yaml`

- [ ] **Step 1.1: Write the YAML contract**

```yaml
# contracts/orchestration/plan-mode-trigger.v1.yaml
version: 1
description: "Plan-mode vs. ReAct mode decision trigger (D2)"
thresholds:
  min_complexity_for_plan: 6         # estimated_complexity >= this → plan mode
  min_deps_for_plan: 2               # dependency count >= this → plan mode
  min_tool_hints_for_plan: 3         # tool_hints count >= this → plan mode
  adequacy_score_below_react: 0.50   # if prior plan adequacy < this → stay ReAct
  adequacy_score_plan_threshold: 0.65 # if adequacy >= this AND complexity >= 4 → plan
metrics_key: "orch.plan.mode_decision"
feature_flag: "vox.orchestrator.plan_mode_trigger.enabled"
```

- [ ] **Step 1.2: Commit**

```bash
git add contracts/orchestration/plan-mode-trigger.v1.yaml
git commit -m "feat(contracts): add plan-mode-trigger.v1.yaml schema"
```

---

### Task 2: Core PlanModeTrigger

**Files:**
- Create: `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs`

- [ ] **Step 2.1: Write failing tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_complexity_chooses_react() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 0,
            tool_hint_count: 1,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::React);
    }

    #[test]
    fn high_complexity_chooses_plan() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 8,
            dependency_count: 0,
            tool_hint_count: 0,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn many_deps_triggers_plan_even_with_low_complexity() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 3,
            tool_hint_count: 0,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn many_tool_hints_triggers_plan() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 0,
            tool_hint_count: 4,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn low_prior_adequacy_stays_react() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 5, // borderline
            dependency_count: 1,
            tool_hint_count: 2,
            prior_adequacy_score: Some(0.30), // below react threshold
        };
        // low adequacy means prior plan was bad → stay ReAct this round
        assert_eq!(trigger.decide(&signal), PlanModeDecision::React);
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator plan_mode_trigger 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs`:

```rust
//! Autonomous plan-mode vs. ReAct mode decision trigger (D2).
//!
//! Pure function: no I/O, no allocations on the hot path.
//! Wired into plan_loop.rs before the first LLM call.

use serde::{Deserialize, Serialize};

/// Decision: should the orchestrator use Plan-and-Execute or ReAct?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanModeDecision {
    /// Reactive single-hop mode: evaluate → act → observe → repeat.
    React,
    /// Structured plan-and-execute: build full task DAG first, then execute.
    PlanAndExecute,
}

/// Input signals for the trigger decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeSignal {
    /// Task estimated_complexity (0–10).
    pub complexity: u8,
    /// Number of declared task dependencies.
    pub dependency_count: usize,
    /// Number of tool hints declared upfront.
    pub tool_hint_count: usize,
    /// `RubricScores::weighted_score()` from a prior plan adequacy pass, if available.
    pub prior_adequacy_score: Option<f32>,
}

/// Thresholds loaded from contract YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeTriggerConfig {
    pub min_complexity_for_plan: u8,
    pub min_deps_for_plan: usize,
    pub min_tool_hints_for_plan: usize,
    pub adequacy_score_below_react: f32,
    pub adequacy_score_plan_threshold: f32,
    pub adequacy_plan_min_complexity: u8,
}

impl Default for PlanModeTriggerConfig {
    fn default() -> Self {
        Self {
            min_complexity_for_plan: 6,
            min_deps_for_plan: 2,
            min_tool_hints_for_plan: 3,
            adequacy_score_below_react: 0.50,
            adequacy_score_plan_threshold: 0.65,
            adequacy_plan_min_complexity: 4,
        }
    }
}

/// Pure plan-mode trigger.
pub struct PlanModeTrigger {
    config: PlanModeTriggerConfig,
}

impl PlanModeTrigger {
    pub fn new(config: PlanModeTriggerConfig) -> Self {
        Self { config }
    }

    /// Returns the mode decision for this signal.
    #[must_use]
    pub fn decide(&self, signal: &PlanModeSignal) -> PlanModeDecision {
        let c = &self.config;

        // Prior adequacy score veto: if past plans were weak, stay ReAct
        if let Some(score) = signal.prior_adequacy_score {
            if score < c.adequacy_score_below_react {
                return PlanModeDecision::React;
            }
        }

        // Strong signal: high complexity
        if signal.complexity >= c.min_complexity_for_plan {
            return PlanModeDecision::PlanAndExecute;
        }

        // Strong signal: many declared dependencies
        if signal.dependency_count >= c.min_deps_for_plan {
            return PlanModeDecision::PlanAndExecute;
        }

        // Strong signal: many declared tool hints (predictable tool graph)
        if signal.tool_hint_count >= c.min_tool_hints_for_plan {
            return PlanModeDecision::PlanAndExecute;
        }

        // Prior adequacy upgrade: good prior plan + borderline complexity
        if let Some(score) = signal.prior_adequacy_score {
            if score >= c.adequacy_score_plan_threshold
                && signal.complexity >= c.adequacy_plan_min_complexity
            {
                return PlanModeDecision::PlanAndExecute;
            }
        }

        PlanModeDecision::React
    }
}

/// Metric payload emitted per mode decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeEvent {
    pub metric_type: &'static str,
    pub decision: PlanModeDecision,
    pub complexity: u8,
    pub dependency_count: usize,
    pub tool_hint_count: usize,
    pub session_id: Option<String>,
}

impl PlanModeEvent {
    pub fn new(decision: PlanModeDecision, signal: &PlanModeSignal) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_PLAN_MODE_DECISION,
            decision,
            complexity: signal.complexity,
            dependency_count: signal.dependency_count,
            tool_hint_count: signal.tool_hint_count,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_complexity_chooses_react() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 0,
            tool_hint_count: 1,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::React);
    }

    #[test]
    fn high_complexity_chooses_plan() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 8,
            dependency_count: 0,
            tool_hint_count: 0,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn many_deps_triggers_plan_even_with_low_complexity() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 3,
            tool_hint_count: 0,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn many_tool_hints_triggers_plan() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 3,
            dependency_count: 0,
            tool_hint_count: 4,
            prior_adequacy_score: None,
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn low_prior_adequacy_stays_react() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 5,
            dependency_count: 1,
            tool_hint_count: 2,
            prior_adequacy_score: Some(0.30),
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::React);
    }

    #[test]
    fn high_adequacy_with_moderate_complexity_upgrades() {
        let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
        let signal = PlanModeSignal {
            complexity: 5, // >= adequacy_plan_min_complexity(4)
            dependency_count: 0,
            tool_hint_count: 0,
            prior_adequacy_score: Some(0.80), // >= 0.65
        };
        assert_eq!(trigger.decide(&signal), PlanModeDecision::PlanAndExecute);
    }

    #[test]
    fn plan_mode_event_has_correct_metric_type() {
        let signal = PlanModeSignal {
            complexity: 8,
            dependency_count: 2,
            tool_hint_count: 3,
            prior_adequacy_score: None,
        };
        let event = PlanModeEvent::new(PlanModeDecision::PlanAndExecute, &signal);
        assert_eq!(event.metric_type, "orch.plan.mode_decision");
    }
}
```

- [ ] **Step 2.4: Register in planning/mod.rs**

Read `crates/vox-orchestrator/src/planning/mod.rs` then add:
```rust
pub mod plan_mode_trigger;
```

- [ ] **Step 2.5: Run tests**

Run: `cargo test -p vox-orchestrator plan_mode_trigger -- --nocapture`
Expected: All 7 tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vox-orchestrator/src/planning/plan_mode_trigger.rs \
        crates/vox-orchestrator/src/planning/mod.rs
git commit -m "feat(orchestrator): add PlanModeTrigger for D2 plan-mode vs ReAct decision"
```

---

### Task 3: Golden fixtures

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/plan-mode/*.json`
- Create: `crates/vox-orchestrator/tests/plan_mode_trigger_integration.rs`

- [ ] **Step 3.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/plan-mode/react_simple.json`:
```json
{
  "signal": {
    "complexity": 3,
    "dependency_count": 0,
    "tool_hint_count": 1,
    "prior_adequacy_score": null
  },
  "expected_decision": "React"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/plan-mode/plan_complex.json`:
```json
{
  "signal": {
    "complexity": 8,
    "dependency_count": 0,
    "tool_hint_count": 0,
    "prior_adequacy_score": null
  },
  "expected_decision": "PlanAndExecute"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/plan-mode/plan_many_deps.json`:
```json
{
  "signal": {
    "complexity": 3,
    "dependency_count": 3,
    "tool_hint_count": 0,
    "prior_adequacy_score": null
  },
  "expected_decision": "PlanAndExecute"
}
```

- [ ] **Step 3.2: Write integration test file**

Create `crates/vox-orchestrator/tests/plan_mode_trigger_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::planning::plan_mode_trigger::{
    PlanModeDecision, PlanModeSignal, PlanModeTrigger, PlanModeTriggerConfig,
};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct PlanModeFixture {
    signal: PlanModeSignal,
    expected_decision: PlanModeDecision,
}

#[test]
fn golden_react_simple() {
    let f: PlanModeFixture =
        load_golden_fixture("plan-mode/react_simple.json").unwrap();
    let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
    assert_eq!(trigger.decide(&f.signal), f.expected_decision);
}

#[test]
fn golden_plan_complex() {
    let f: PlanModeFixture =
        load_golden_fixture("plan-mode/plan_complex.json").unwrap();
    let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
    assert_eq!(trigger.decide(&f.signal), f.expected_decision);
}

#[test]
fn golden_plan_many_deps() {
    let f: PlanModeFixture =
        load_golden_fixture("plan-mode/plan_many_deps.json").unwrap();
    let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
    assert_eq!(trigger.decide(&f.signal), f.expected_decision);
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test --test plan_mode_trigger_integration`
Expected: 3 golden tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/plan-mode/ \
        crates/vox-orchestrator/tests/plan_mode_trigger_integration.rs
git commit -m "test(orchestrator): golden fixtures for PlanModeTrigger"
```

---

### Task 4: Wire into plan_loop.rs with feature-flag guard

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs`

- [ ] **Step 4.1: Read plan_loop.rs entry function**

Read `crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs` lines 1–150 to find where `PlanRefinementState` is initialized and where the first LLM call is made.

- [ ] **Step 4.2: Write test for the wired gate**

Add to `plan_mode_trigger_integration.rs`:

```rust
#[test]
fn trigger_does_not_panic_with_empty_signal() {
    let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
    let signal = PlanModeSignal {
        complexity: 0,
        dependency_count: 0,
        tool_hint_count: 0,
        prior_adequacy_score: None,
    };
    // Edge case: complexity 0, everything at minimum → React
    assert_eq!(trigger.decide(&signal), PlanModeDecision::React);
}
```

- [ ] **Step 4.3: Add trigger call at plan_loop entry**

In `plan_loop.rs`, before the first LLM call (the `mcp_infer_completion` call for initial planning), add:

```rust
// Feature-gated plan-mode trigger (D2)
if std::env::var("VOX_ORCHESTRATOR_PLAN_MODE_TRIGGER").as_deref() == Ok("1") {
    use vox_orchestrator::planning::plan_mode_trigger::{
        PlanModeDecision, PlanModeSignal, PlanModeTrigger, PlanModeTriggerConfig,
    };
    let signal = PlanModeSignal {
        complexity: params.tasks.iter()
            .map(|t| t.estimated_complexity)
            .max()
            .unwrap_or(0),
        dependency_count: params.tasks.iter()
            .map(|t| t.depends_on.len())
            .sum(),
        tool_hint_count: params.tasks.iter()
            .flat_map(|t| t.tool_hints.iter())
            .count(),
        prior_adequacy_score: None, // populated by P3 if available
    };
    let trigger = PlanModeTrigger::new(PlanModeTriggerConfig::default());
    let mode = trigger.decide(&signal);
    tracing::debug!(mode = ?mode, "plan-mode trigger decision");
    // TODO(P5): if React, short-circuit to single-hop execution
}
```

Note: `params.tasks` and `t.tool_hints` field names must be verified by reading `PlanParams` and `PlanTask` in `params.rs` before writing. Adjust if field names differ.

- [ ] **Step 4.4: Verify compilation**

Run: `cargo build -p vox-orchestrator-mcp 2>&1 | tail -20`
Expected: Compiles clean.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs \
        crates/vox-orchestrator/tests/plan_mode_trigger_integration.rs
git commit -m "feat(orchestrator): wire PlanModeTrigger into plan_loop entry (feature-gated)"
```

---

### Task 5: Update where-things-live.md and arch-check

- [ ] **Step 5.1: Add plan_mode_trigger row**

```
| `plan_mode_trigger` | Plan-mode vs. ReAct decision trigger (D2) | `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs` |
```

- [ ] **Step 5.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`
Expected: Clean.

- [ ] **Step 5.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register plan_mode_trigger in where-things-live.md"
```

---

### Task 6: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** `PlanModeEvent::metric_type == METRIC_TYPE_PLAN_MODE_DECISION`
- [ ] **G3** No perf bench required (decision is pure, <1µs by construction)
- [ ] **G4** Contract: `plan-mode-trigger.v1.yaml` parses correctly
- [ ] **G5** HITL fallback: low adequacy score (`< 0.50`) always returns `React`

---

**Phase 5 sign-off:** 5 tasks complete, 7+ unit tests + 3 golden fixtures, `cargo build` clean.
