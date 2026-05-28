# Orchestrator Phase 10: Sub-Agent Dispatch (D4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add autonomous sub-agent dispatch decision logic: when should the orchestrator spawn a sub-agent vs. execute inline? Cap chain depth to prevent runaway delegation. Emit dispatch and chain-depth telemetry.

**Architecture:** A new `subagent_dispatch.rs` module holds a pure `DispatchRouter` that decides `DispatchDecision::{Inline, Spawn, Reject}` based on task complexity, current chain depth, and resource budget. An `EscalationRequired` variant is added to `AgentMessage` (for P6 integration) and a `SubAgentDispatched` variant is added for D4 telemetry. Chain depth is capped at a configurable limit; exceeding it publishes a chain-depth alert.

**Tech Stack:** Rust, `AgentMessage` enum in `types/messages.rs`, `BulletinBoard`, `METRIC_TYPE_SUBAGENT_DISPATCH`, `METRIC_TYPE_CHAIN_DEPTH_ALERT`, feature flags `vox.orchestrator.subagent_dispatch.enabled`, `vox.orchestrator.chain_length_cap.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/subagent_dispatch.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod subagent_dispatch |
| Modify | `crates/vox-orchestrator/src/types/messages.rs` — add new variants |
| Create | `crates/vox-orchestrator/tests/subagent_dispatch_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/dispatch/inline_simple.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/dispatch/spawn_complex.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/dispatch/reject_chain_too_deep.json` |
| Modify | `docs/src/architecture/where-things-live.md` — add subagent_dispatch row |

---

### Task 1: Add new AgentMessage variants

**Files:**
- Modify: `crates/vox-orchestrator/src/types/messages.rs`

- [ ] **Step 1.1: Read messages.rs to understand the full existing variant list**

Read `crates/vox-orchestrator/src/types/messages.rs` to find the end of `AgentMessage` enum and understand existing fields for IDs (AgentId, TaskId, CorrelationId).

- [ ] **Step 1.2: Write test for new variants**

Add to a new `crates/vox-orchestrator/tests/subagent_dispatch_integration.rs`:

```rust
use vox_orchestrator::types::AgentMessage;

#[test]
fn agent_message_escalation_required_variant_exists() {
    let msg = AgentMessage::EscalationRequired {
        session_id: "test-session".to_string(),
        grade: "high".to_string(),
        action_description: "delete production database".to_string(),
    };
    // Verify it serializes without panic
    let _ = serde_json::to_string(&msg).unwrap();
}

#[test]
fn agent_message_subagent_dispatched_variant_exists() {
    let msg = AgentMessage::SubAgentDispatched {
        parent_agent_id: "parent".to_string(),
        child_task_description: "sub task".to_string(),
        chain_depth: 2,
    };
    let _ = serde_json::to_string(&msg).unwrap();
}

#[test]
fn agent_message_chain_depth_alert_variant_exists() {
    let msg = AgentMessage::ChainDepthAlert {
        current_depth: 5,
        max_depth: 5,
    };
    let _ = serde_json::to_string(&msg).unwrap();
}
```

- [ ] **Step 1.3: Run test to verify it fails**

Run: `cargo test --test subagent_dispatch_integration agent_message 2>&1 | head -20`
Expected: variant not found errors.

- [ ] **Step 1.4: Add variants to AgentMessage**

In `crates/vox-orchestrator/src/types/messages.rs`, at the end of the `AgentMessage` enum (before the closing `}`), add:

```rust
    /// Risk matrix triggered a HITL escalation requirement (D5+D9).
    EscalationRequired {
        /// Session needing HITL review.
        session_id: String,
        /// Risk grade: "high" | "critical"
        grade: String,
        /// Human-readable description of the action requiring approval.
        action_description: String,
    },
    /// A sub-agent was dispatched to handle a delegated task (D4).
    SubAgentDispatched {
        /// Parent agent that spawned the sub-agent.
        parent_agent_id: String,
        /// Short description of the delegated task.
        child_task_description: String,
        /// Current delegation chain depth (root = 0).
        chain_depth: u32,
    },
    /// Sub-agent chain depth exceeded the configured cap (D4).
    ChainDepthAlert {
        /// Current depth at which the alert fired.
        current_depth: u32,
        /// Configured maximum allowed depth.
        max_depth: u32,
    },
```

- [ ] **Step 1.5: Run compilation check**

Run: `cargo build -p vox-orchestrator 2>&1 | tail -10`
Expected: Compiles clean (non_exhaustive means no match arm issues in other crates).

- [ ] **Step 1.6: Commit**

```bash
git add crates/vox-orchestrator/src/types/messages.rs \
        crates/vox-orchestrator/tests/subagent_dispatch_integration.rs
git commit -m "feat(orchestrator): add EscalationRequired, SubAgentDispatched, ChainDepthAlert to AgentMessage"
```

---

### Task 2: DispatchRouter

**Files:**
- Create: `crates/vox-orchestrator/src/subagent_dispatch.rs`

- [ ] **Step 2.1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_task_dispatches_inline() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 3,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Inline);
    }

    #[test]
    fn complex_task_spawns_subagent() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 8,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Spawn);
    }

    #[test]
    fn chain_too_deep_rejects() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 8,
            chain_depth: 5, // at max_chain_depth
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Reject);
    }

    #[test]
    fn budget_exhausted_forces_inline() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 9,
            chain_depth: 0,
            budget_exhausted: true,
            parent_has_exclusive_lock: false,
        };
        // Budget exhausted: cannot afford to spawn; go inline or reject
        assert_eq!(router.decide(&signal), DispatchDecision::Inline);
    }
}
```

- [ ] **Step 2.2: Run to verify failure**

Run: `cargo test -p vox-orchestrator subagent_dispatch 2>&1 | head -10`

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/subagent_dispatch.rs`:

```rust
//! Sub-agent dispatch decision logic (D4).
//!
//! Decides when the orchestrator should spawn a sub-agent vs. execute inline.
//! Caps chain depth to prevent runaway delegation.

use serde::{Deserialize, Serialize};

/// Decision for a dispatch request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchDecision {
    /// Execute the task inline in the current agent context.
    Inline,
    /// Spawn a sub-agent to handle the task.
    Spawn,
    /// Reject delegation; chain depth exceeded or resources exhausted.
    Reject,
}

/// Signals for dispatch decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchSignal {
    /// Task complexity 0–10.
    pub complexity: u8,
    /// Current sub-agent chain depth (root = 0).
    pub chain_depth: u32,
    /// Whether the session budget is exhausted.
    pub budget_exhausted: bool,
    /// Whether the parent agent holds an exclusive file lock that the child would need.
    pub parent_has_exclusive_lock: bool,
}

/// Thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchConfig {
    /// Complexity at or above which spawning a sub-agent is considered.
    pub min_complexity_for_spawn: u8,
    /// Maximum allowed chain depth before Reject is returned.
    pub max_chain_depth: u32,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            min_complexity_for_spawn: 6,
            max_chain_depth: 5,
        }
    }
}

/// Pure dispatch router.
pub struct DispatchRouter {
    config: DispatchConfig,
}

impl DispatchRouter {
    pub fn new(config: DispatchConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn decide(&self, signal: &DispatchSignal) -> DispatchDecision {
        // Chain depth cap: always Reject regardless of other signals
        if signal.chain_depth >= self.config.max_chain_depth {
            return DispatchDecision::Reject;
        }
        // Budget exhausted: cannot afford to spawn overhead
        if signal.budget_exhausted {
            return DispatchDecision::Inline;
        }
        // Parent lock held: spawning would cause deadlock
        if signal.parent_has_exclusive_lock {
            return DispatchDecision::Inline;
        }
        // Complexity threshold
        if signal.complexity >= self.config.min_complexity_for_spawn {
            DispatchDecision::Spawn
        } else {
            DispatchDecision::Inline
        }
    }

    /// Returns true if a chain-depth alert should be published.
    #[must_use]
    #[inline]
    pub fn should_alert_chain_depth(&self, chain_depth: u32) -> bool {
        chain_depth >= self.config.max_chain_depth
    }
}

/// Metric payload for dispatch events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentDispatchEvent {
    pub metric_type: &'static str,
    pub decision: DispatchDecision,
    pub complexity: u8,
    pub chain_depth: u32,
}

impl SubAgentDispatchEvent {
    pub fn new(decision: DispatchDecision, signal: &DispatchSignal) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_SUBAGENT_DISPATCH,
            decision,
            complexity: signal.complexity,
            chain_depth: signal.chain_depth,
        }
    }
}

/// Metric payload for chain depth alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainDepthAlertEvent {
    pub metric_type: &'static str,
    pub current_depth: u32,
    pub max_depth: u32,
}

impl ChainDepthAlertEvent {
    pub fn new(current_depth: u32, max_depth: u32) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CHAIN_DEPTH_ALERT,
            current_depth,
            max_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_task_dispatches_inline() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 3,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Inline);
    }

    #[test]
    fn complex_task_spawns_subagent() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 8,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Spawn);
    }

    #[test]
    fn chain_too_deep_rejects() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 8,
            chain_depth: 5,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Reject);
    }

    #[test]
    fn budget_exhausted_forces_inline() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 9,
            chain_depth: 0,
            budget_exhausted: true,
            parent_has_exclusive_lock: false,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Inline);
    }

    #[test]
    fn lock_held_forces_inline() {
        let router = DispatchRouter::new(DispatchConfig::default());
        let signal = DispatchSignal {
            complexity: 9,
            chain_depth: 0,
            budget_exhausted: false,
            parent_has_exclusive_lock: true,
        };
        assert_eq!(router.decide(&signal), DispatchDecision::Inline);
    }

    #[test]
    fn chain_depth_alert_fires_at_max() {
        let router = DispatchRouter::new(DispatchConfig::default());
        assert!(router.should_alert_chain_depth(5));
        assert!(!router.should_alert_chain_depth(4));
    }

    #[test]
    fn dispatch_event_metric_type() {
        let signal = DispatchSignal {
            complexity: 7,
            chain_depth: 1,
            budget_exhausted: false,
            parent_has_exclusive_lock: false,
        };
        let event = SubAgentDispatchEvent::new(DispatchDecision::Spawn, &signal);
        assert_eq!(event.metric_type, "orch.subagent.dispatch");
    }

    #[test]
    fn chain_depth_alert_event_metric_type() {
        let event = ChainDepthAlertEvent::new(5, 5);
        assert_eq!(event.metric_type, "orch.subagent.chain_depth_alert");
    }
}
```

- [ ] **Step 2.4: Register and run**

Add `pub mod subagent_dispatch;` to `lib.rs`.

Run: `cargo test -p vox-orchestrator subagent_dispatch -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-orchestrator/src/subagent_dispatch.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add DispatchRouter for sub-agent dispatch decisions (D4)"
```

---

### Task 3: Golden fixtures and integration tests

**Files:**
- Create: fixtures and integration tests

- [ ] **Step 3.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/dispatch/inline_simple.json`:
```json
{
  "signal": {
    "complexity": 3,
    "chain_depth": 0,
    "budget_exhausted": false,
    "parent_has_exclusive_lock": false
  },
  "expected_decision": "Inline"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/dispatch/spawn_complex.json`:
```json
{
  "signal": {
    "complexity": 8,
    "chain_depth": 0,
    "budget_exhausted": false,
    "parent_has_exclusive_lock": false
  },
  "expected_decision": "Spawn"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/dispatch/reject_chain_too_deep.json`:
```json
{
  "signal": {
    "complexity": 8,
    "chain_depth": 5,
    "budget_exhausted": false,
    "parent_has_exclusive_lock": false
  },
  "expected_decision": "Reject"
}
```

- [ ] **Step 3.2: Add golden tests to integration file**

Add to `crates/vox-orchestrator/tests/subagent_dispatch_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::subagent_dispatch::{
    DispatchConfig, DispatchDecision, DispatchRouter, DispatchSignal,
};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct DispatchFixture {
    signal: DispatchSignal,
    expected_decision: DispatchDecision,
}

#[test]
fn golden_inline_simple() {
    let f: DispatchFixture = load_golden_fixture("dispatch/inline_simple.json").unwrap();
    let router = DispatchRouter::new(DispatchConfig::default());
    assert_eq!(router.decide(&f.signal), f.expected_decision);
}

#[test]
fn golden_spawn_complex() {
    let f: DispatchFixture = load_golden_fixture("dispatch/spawn_complex.json").unwrap();
    let router = DispatchRouter::new(DispatchConfig::default());
    assert_eq!(router.decide(&f.signal), f.expected_decision);
}

#[test]
fn golden_reject_chain_too_deep() {
    let f: DispatchFixture = load_golden_fixture("dispatch/reject_chain_too_deep.json").unwrap();
    let router = DispatchRouter::new(DispatchConfig::default());
    assert_eq!(router.decide(&f.signal), f.expected_decision);
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test --test subagent_dispatch_integration`
Expected: All 6 tests pass (3 from Task 1, 3 golden).

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/dispatch/ \
        crates/vox-orchestrator/tests/subagent_dispatch_integration.rs
git commit -m "test(orchestrator): golden fixtures for DispatchRouter"
```

---

### Task 4: Update where-things-live.md and arch-check

- [ ] **Step 4.1: Add subagent_dispatch row**

```
| `subagent_dispatch` | Sub-agent dispatch decision and chain-depth cap (D4) | `crates/vox-orchestrator/src/subagent_dispatch.rs` |
```

- [ ] **Step 4.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`

- [ ] **Step 4.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register subagent_dispatch in where-things-live.md"
```

---

### Task 5: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** Both metric types tested (`orch.subagent.dispatch`, `orch.subagent.chain_depth_alert`)
- [ ] **G3** No perf bench required (<100ns pure)
- [ ] **G4** `chain_depth >= max_chain_depth` always → Reject
- [ ] **G5** `EscalationRequired` variant compiles and serializes in all existing `match AgentMessage` arms (non_exhaustive prevents missed arms)

---

**Phase 10 sign-off:** 5 tasks complete, 8+ unit tests + 3 golden fixtures, `cargo build` clean.
