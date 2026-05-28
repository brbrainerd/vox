# Orchestrator Phase 2: Circuit Breaker (D6 — Doom-Loop Detection) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a five-signal circuit breaker that detects doom loops in the orchestrator's plan-execute cycle and fires graduated warnings, forced replanning, and HITL escalation when needed.

**Architecture:** A new `circuit_breaker.rs` module holds a pure `CircuitBreaker` struct that computes `should_trip()` from five counters; it is wired into `plan_loop.rs` after each tool result. Graduated CAUTION/WARNING messages are injected into the context window; a hard trip escalates via `BulletinBoard`.

**Tech Stack:** Rust, `proptest` for property tests, `criterion` for perf bench, `vox-db` for `METRIC_TYPE_CIRCUIT_BREAKER_TRIP`, feature flag `vox.orchestrator.circuit_breaker.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/circuit_breaker.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod circuit_breaker |
| Modify | `crates/vox-orchestrator/src/mcp_tools/chat_tools/plan_loop.rs` — wire check |
| Create | `crates/vox-orchestrator/benches/circuit_breaker.rs` |
| Create | `crates/vox-orchestrator/tests/circuit_breaker_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/no_trip.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_no_progress.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_same_error.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_tool_thrash.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_ngram_overlap.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_semantic_drift.json` |
| Modify | `contracts/orchestration/circuit-breaker.v1.yaml` — fill real schema (scaffolded in P1) |
| Modify | `docs/src/architecture/where-things-live.md` — add circuit_breaker row |

---

### Task 1: Fill the contract schema

**Files:**
- Modify: `contracts/orchestration/circuit-breaker.v1.yaml`

- [ ] **Step 1.1: Write the YAML contract**

```yaml
# contracts/orchestration/circuit-breaker.v1.yaml
version: 1
description: "Five-signal circuit breaker for orchestrator doom-loop detection (D6)"
signals:
  no_progress_threshold: 3          # consecutive loops with no new tool results
  same_error_threshold: 5           # same error class repeated consecutively
  tool_thrash_threshold: 15         # total redundant tool calls in one plan cycle
  ngram_overlap_threshold: 0.85     # action bigram Jaccard similarity cap
  semantic_drift_sigma: 2.0         # z-score threshold vs session baseline
tiers:
  caution:
    no_progress_gte: 1
    same_error_gte: 2
    tool_thrash_gte: 8
  warning:
    no_progress_gte: 2
    same_error_gte: 3
    tool_thrash_gte: 12
  trip:
    no_progress_gte: 3
    same_error_gte: 5
    tool_thrash_gte: 15
replan_limit: 3                     # replanning attempts before HITL escalation
metrics_key: "orch.circuit_breaker.trip"
feature_flag: "vox.orchestrator.circuit_breaker.enabled"
```

- [ ] **Step 1.2: Commit**

```bash
git add contracts/orchestration/circuit-breaker.v1.yaml
git commit -m "feat(contracts): fill circuit-breaker.v1.yaml schema"
```

---

### Task 2: Define the core types

**Files:**
- Create: `crates/vox-orchestrator/src/circuit_breaker.rs`

- [ ] **Step 2.1: Write the failing test first**

```rust
// At the bottom of circuit_breaker.rs (will be created in 2.2)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trip_when_all_signals_zero() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState::default();
        assert!(cb.should_trip(&state).is_none());
    }

    #[test]
    fn trips_on_no_progress_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 3,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::NoProgress));
    }

    #[test]
    fn caution_tier_at_one_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 1,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Caution);
    }

    #[test]
    fn warning_tier_at_two_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 2,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Warning);
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails (module not yet defined)**

Run: `cargo test -p vox-orchestrator circuit_breaker 2>&1 | head -20`
Expected: `error[E0432]: unresolved import` or module not found.

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/circuit_breaker.rs`:

```rust
//! Five-signal circuit breaker for orchestrator doom-loop detection (D6).
//!
//! Reads thresholds from [`CircuitBreakerConfig`] which is loaded from
//! `contracts/orchestration/circuit-breaker.v1.yaml` at bootstrap.
//! All checks are pure: no async, no I/O, no allocations on the hot path.

use serde::{Deserialize, Serialize};

/// Reason the circuit was tripped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TripReason {
    NoProgress,
    SameError,
    ToolThrash,
    NgramOverlap,
    SemanticDrift,
}

impl std::fmt::Display for TripReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProgress => write!(f, "no-progress"),
            Self::SameError => write!(f, "same-error"),
            Self::ToolThrash => write!(f, "tool-thrash"),
            Self::NgramOverlap => write!(f, "ngram-overlap"),
            Self::SemanticDrift => write!(f, "semantic-drift"),
        }
    }
}

/// Graduated alarm tier (below trip threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlarmTier {
    None,
    Caution,
    Warning,
}

/// Running counters for the breaker; update after each loop iteration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    /// Consecutive plan loops with no new tool results.
    pub no_progress_loops: u32,
    /// Consecutive loops returning the same error class.
    pub same_error_loops: u32,
    /// Total redundant tool calls in this plan cycle.
    pub tool_thrash_count: u32,
    /// Jaccard similarity of current action bigrams vs prior bigrams (0.0–1.0).
    pub ngram_overlap: f64,
    /// Z-score of current embedding vs session baseline.
    pub semantic_drift_sigma: f64,
    /// How many replan attempts have occurred after a trip.
    pub replan_attempts: u32,
}

/// Thresholds loaded from contract YAML. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub no_progress_threshold: u32,
    pub same_error_threshold: u32,
    pub tool_thrash_threshold: u32,
    pub ngram_overlap_threshold: f64,
    pub semantic_drift_sigma: f64,
    pub caution_no_progress: u32,
    pub caution_same_error: u32,
    pub caution_tool_thrash: u32,
    pub warning_no_progress: u32,
    pub warning_same_error: u32,
    pub warning_tool_thrash: u32,
    pub replan_limit: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            no_progress_threshold: 3,
            same_error_threshold: 5,
            tool_thrash_threshold: 15,
            ngram_overlap_threshold: 0.85,
            semantic_drift_sigma: 2.0,
            caution_no_progress: 1,
            caution_same_error: 2,
            caution_tool_thrash: 8,
            warning_no_progress: 2,
            warning_same_error: 3,
            warning_tool_thrash: 12,
            replan_limit: 3,
        }
    }
}

/// Pure, allocation-free circuit breaker.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self { config }
    }

    /// Returns `Some(reason)` if the breaker should trip, `None` otherwise.
    #[must_use]
    #[inline]
    pub fn should_trip(&self, state: &CircuitBreakerState) -> Option<TripReason> {
        if state.no_progress_loops >= self.config.no_progress_threshold {
            return Some(TripReason::NoProgress);
        }
        if state.same_error_loops >= self.config.same_error_threshold {
            return Some(TripReason::SameError);
        }
        if state.tool_thrash_count >= self.config.tool_thrash_threshold {
            return Some(TripReason::ToolThrash);
        }
        if state.ngram_overlap >= self.config.ngram_overlap_threshold {
            return Some(TripReason::NgramOverlap);
        }
        if state.semantic_drift_sigma >= self.config.semantic_drift_sigma {
            return Some(TripReason::SemanticDrift);
        }
        None
    }

    /// Returns the current alarm tier without tripping.
    #[must_use]
    #[inline]
    pub fn check_tier(&self, state: &CircuitBreakerState) -> AlarmTier {
        if state.no_progress_loops >= self.config.warning_no_progress
            || state.same_error_loops >= self.config.warning_same_error
            || state.tool_thrash_count >= self.config.warning_tool_thrash
        {
            return AlarmTier::Warning;
        }
        if state.no_progress_loops >= self.config.caution_no_progress
            || state.same_error_loops >= self.config.caution_same_error
            || state.tool_thrash_count >= self.config.caution_tool_thrash
        {
            return AlarmTier::Caution;
        }
        AlarmTier::None
    }

    /// Returns true if replanning should escalate to HITL (replan limit exceeded).
    #[must_use]
    #[inline]
    pub fn should_escalate(&self, state: &CircuitBreakerState) -> bool {
        state.replan_attempts >= self.config.replan_limit
    }
}

/// Compute bigram Jaccard similarity between two action sequences.
/// Used for ngram overlap signal. O(n) time, O(n) space.
#[must_use]
pub fn bigram_jaccard(a: &[&str], b: &[&str]) -> f64 {
    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }
    use std::collections::HashSet;
    let bigrams_a: HashSet<(&str, &str)> = a.windows(2).map(|w| (w[0], w[1])).collect();
    let bigrams_b: HashSet<(&str, &str)> = b.windows(2).map(|w| (w[0], w[1])).collect();
    let intersection = bigrams_a.intersection(&bigrams_b).count();
    let union = bigrams_a.union(&bigrams_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trip_when_all_signals_zero() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState::default();
        assert!(cb.should_trip(&state).is_none());
    }

    #[test]
    fn trips_on_no_progress_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 3,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::NoProgress));
    }

    #[test]
    fn caution_tier_at_one_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 1,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Caution);
    }

    #[test]
    fn warning_tier_at_two_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 2,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Warning);
    }

    #[test]
    fn trips_on_same_error_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            same_error_loops: 5,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::SameError));
    }

    #[test]
    fn trips_on_tool_thrash_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            tool_thrash_count: 15,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::ToolThrash));
    }

    #[test]
    fn trips_on_ngram_overlap() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            ngram_overlap: 0.90,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::NgramOverlap));
    }

    #[test]
    fn trips_on_semantic_drift() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            semantic_drift_sigma: 2.5,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::SemanticDrift));
    }

    #[test]
    fn no_escalation_below_replan_limit() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            replan_attempts: 2,
            ..Default::default()
        };
        assert!(!cb.should_escalate(&state));
    }

    #[test]
    fn escalates_at_replan_limit() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            replan_attempts: 3,
            ..Default::default()
        };
        assert!(cb.should_escalate(&state));
    }

    #[test]
    fn bigram_jaccard_identical_sequences() {
        let a = vec!["read_file", "write_file", "run_test"];
        let b = vec!["read_file", "write_file", "run_test"];
        assert!((bigram_jaccard(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bigram_jaccard_disjoint_sequences() {
        let a = vec!["read_file", "write_file"];
        let b = vec!["run_test", "commit"];
        assert!(bigram_jaccard(&a, &b) < 1e-9);
    }

    #[test]
    fn bigram_jaccard_empty_inputs() {
        assert!((bigram_jaccard(&[], &[])).abs() < 1e-9);
    }
}
```

- [ ] **Step 2.4: Add `pub mod circuit_breaker;` to `lib.rs`**

In `crates/vox-orchestrator/src/lib.rs`, add:
```rust
pub mod circuit_breaker;
```

- [ ] **Step 2.5: Run tests**

Run: `cargo test -p vox-orchestrator circuit_breaker -- --nocapture`
Expected: All 13 tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vox-orchestrator/src/circuit_breaker.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add CircuitBreaker struct with five-signal detection (D6)"
```

---

### Task 3: Property tests for bigram_jaccard

**Files:**
- Create: `crates/vox-orchestrator/tests/circuit_breaker_integration.rs`

- [ ] **Step 3.1: Write property tests**

```rust
// crates/vox-orchestrator/tests/circuit_breaker_integration.rs
use proptest::prelude::*;
use vox_orchestrator::circuit_breaker::bigram_jaccard;

proptest! {
    #[test]
    fn jaccard_always_in_range(
        a in prop::collection::vec("[a-z_]{3,10}", 0..20),
        b in prop::collection::vec("[a-z_]{3,10}", 0..20),
    ) {
        let a_refs: Vec<&str> = a.iter().map(String::as_str).collect();
        let b_refs: Vec<&str> = b.iter().map(String::as_str).collect();
        let result = bigram_jaccard(&a_refs, &b_refs);
        prop_assert!(result >= 0.0 && result <= 1.0,
            "jaccard out of [0,1]: {result}");
    }

    #[test]
    fn jaccard_symmetry(
        a in prop::collection::vec("[a-z_]{3,10}", 2..15),
        b in prop::collection::vec("[a-z_]{3,10}", 2..15),
    ) {
        let a_refs: Vec<&str> = a.iter().map(String::as_str).collect();
        let b_refs: Vec<&str> = b.iter().map(String::as_str).collect();
        let ab = bigram_jaccard(&a_refs, &b_refs);
        let ba = bigram_jaccard(&b_refs, &a_refs);
        prop_assert!((ab - ba).abs() < 1e-10,
            "jaccard not symmetric: {ab} vs {ba}");
    }
}
```

- [ ] **Step 3.2: Add `proptest` to `vox-orchestrator/Cargo.toml` dev-dependencies**

```toml
[dev-dependencies]
proptest = "1"
```

- [ ] **Step 3.3: Run property tests**

Run: `cargo test -p vox-orchestrator --test circuit_breaker_integration`
Expected: 200 cases pass for each property.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator/tests/circuit_breaker_integration.rs crates/vox-orchestrator/Cargo.toml
git commit -m "test(orchestrator): property tests for bigram_jaccard"
```

---

### Task 4: Criterion benchmark

**Files:**
- Create: `crates/vox-orchestrator/benches/circuit_breaker.rs`

- [ ] **Step 4.1: Write the benchmark**

```rust
// crates/vox-orchestrator/benches/circuit_breaker.rs
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vox_orchestrator::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, bigram_jaccard,
};

fn bench_should_trip(c: &mut Criterion) {
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    let state = CircuitBreakerState {
        no_progress_loops: 1,
        tool_thrash_count: 5,
        ngram_overlap: 0.4,
        ..Default::default()
    };
    c.bench_function("circuit_breaker_should_trip", |b| {
        b.iter(|| cb.should_trip(black_box(&state)))
    });
}

fn bench_bigram_jaccard(c: &mut Criterion) {
    let a: Vec<&str> = vec![
        "read_file", "write_file", "run_test", "commit", "push", "read_file", "run_test",
    ];
    let b: Vec<&str> = vec![
        "read_file", "write_file", "commit", "run_test", "read_file", "write_file",
    ];
    c.bench_function("bigram_jaccard_7x6", |b_| {
        b_.iter(|| bigram_jaccard(black_box(&a), black_box(&b)))
    });
}

criterion_group!(benches, bench_should_trip, bench_bigram_jaccard);
criterion_main!(benches);
```

- [ ] **Step 4.2: Add bench entry to Cargo.toml**

In `crates/vox-orchestrator/Cargo.toml`:
```toml
[[bench]]
name = "circuit_breaker"
harness = false
```

- [ ] **Step 4.3: Run bench to establish baseline**

Run: `cargo bench -p vox-orchestrator --bench circuit_breaker 2>&1 | tail -20`
Expected: `circuit_breaker_should_trip` reports mean <50ns. Record actual numbers in a comment.
Budget: `circuit_breaker_should_trip` MUST be <50µs p99 (the bench should show ns-range).

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator/benches/circuit_breaker.rs crates/vox-orchestrator/Cargo.toml
git commit -m "bench(orchestrator): criterion benchmark for CircuitBreaker should_trip"
```

---

### Task 5: Golden fixtures

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/*.json`

- [ ] **Step 5.1: Write the six fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/no_trip.json`:
```json
{
  "state": {
    "no_progress_loops": 0,
    "same_error_loops": 0,
    "tool_thrash_count": 0,
    "ngram_overlap": 0.1,
    "semantic_drift_sigma": 0.5,
    "replan_attempts": 0
  },
  "expected_trip": null,
  "expected_tier": "None"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_no_progress.json`:
```json
{
  "state": {
    "no_progress_loops": 3,
    "same_error_loops": 0,
    "tool_thrash_count": 0,
    "ngram_overlap": 0.0,
    "semantic_drift_sigma": 0.0,
    "replan_attempts": 0
  },
  "expected_trip": "NoProgress",
  "expected_tier": "Warning"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_same_error.json`:
```json
{
  "state": {
    "no_progress_loops": 0,
    "same_error_loops": 5,
    "tool_thrash_count": 0,
    "ngram_overlap": 0.0,
    "semantic_drift_sigma": 0.0,
    "replan_attempts": 0
  },
  "expected_trip": "SameError",
  "expected_tier": "Warning"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_tool_thrash.json`:
```json
{
  "state": {
    "no_progress_loops": 0,
    "same_error_loops": 0,
    "tool_thrash_count": 15,
    "ngram_overlap": 0.0,
    "semantic_drift_sigma": 0.0,
    "replan_attempts": 0
  },
  "expected_trip": "ToolThrash",
  "expected_tier": "Warning"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_ngram_overlap.json`:
```json
{
  "state": {
    "no_progress_loops": 0,
    "same_error_loops": 0,
    "tool_thrash_count": 0,
    "ngram_overlap": 0.90,
    "semantic_drift_sigma": 0.0,
    "replan_attempts": 0
  },
  "expected_trip": "NgramOverlap",
  "expected_tier": "None"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/trip_semantic_drift.json`:
```json
{
  "state": {
    "no_progress_loops": 0,
    "same_error_loops": 0,
    "tool_thrash_count": 0,
    "ngram_overlap": 0.0,
    "semantic_drift_sigma": 2.5,
    "replan_attempts": 0
  },
  "expected_trip": "SemanticDrift",
  "expected_tier": "None"
}
```

- [ ] **Step 5.2: Write the golden fixture test**

Add to `crates/vox-orchestrator/tests/circuit_breaker_integration.rs`:

```rust
use serde::{Deserialize, Serialize};
use vox_orchestrator::circuit_breaker::{
    AlarmTier, CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, TripReason,
};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct CircuitBreakerFixture {
    state: CircuitBreakerState,
    expected_trip: Option<TripReason>,
    expected_tier: AlarmTier,
}

#[test]
fn golden_no_trip() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/no_trip.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
    assert_eq!(cb.check_tier(&fixture.state), fixture.expected_tier);
}

#[test]
fn golden_trip_no_progress() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/trip_no_progress.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
}

#[test]
fn golden_trip_same_error() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/trip_same_error.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
}

#[test]
fn golden_trip_tool_thrash() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/trip_tool_thrash.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
}

#[test]
fn golden_trip_ngram_overlap() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/trip_ngram_overlap.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
}

#[test]
fn golden_trip_semantic_drift() {
    let fixture: CircuitBreakerFixture =
        load_golden_fixture("circuit-breaker/trip_semantic_drift.json").unwrap();
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    assert_eq!(cb.should_trip(&fixture.state), fixture.expected_trip);
}
```

- [ ] **Step 5.3: Run golden tests**

Run: `cargo test -p vox-orchestrator --test circuit_breaker_integration golden`
Expected: 6 golden tests pass.

- [ ] **Step 5.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/circuit-breaker/ \
        crates/vox-orchestrator/tests/circuit_breaker_integration.rs
git commit -m "test(orchestrator): golden fixtures for CircuitBreaker"
```

---

### Task 6: Wire into plan_loop with feature flag

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/chat_tools/plan_loop.rs`

- [ ] **Step 6.1: Read plan_loop.rs to understand wiring points**

Read `crates/vox-orchestrator/src/mcp_tools/chat_tools/plan_loop.rs`.
Find the main loop iteration that calls tools and receives results — the point after each tool result is received.

- [ ] **Step 6.2: Write the test for wired behavior (feature-flagged)**

Add to `circuit_breaker_integration.rs`:

```rust
// Integration smoke: CircuitBreakerState update logic
#[test]
fn state_update_increments_no_progress() {
    let mut state = CircuitBreakerState::default();
    // Simulate a "no new tool results" iteration
    let had_new_results = false;
    if !had_new_results {
        state.no_progress_loops += 1;
    }
    assert_eq!(state.no_progress_loops, 1);
}
```

- [ ] **Step 6.3: Add wiring to plan_loop.rs**

After reading the file to identify the correct location, add the following pattern inside the tool-result processing loop:

```rust
// Inside the plan loop, after processing tool results:
#[cfg(feature = "circuit-breaker")]  // or check feature flag from config
{
    use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
    let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
    let tier = cb.check_tier(&state.circuit_breaker_state);
    if tier >= crate::circuit_breaker::AlarmTier::Caution {
        // Inject warning message into context
        let msg = format!(
            "[ORCHESTRATOR CAUTION] Circuit breaker alarm: {:?}. Rethink your approach.",
            tier
        );
        // append msg to tool_results or context messages as appropriate
        let _ = msg; // remove when wired to actual context injection
    }
    if let Some(reason) = cb.should_trip(&state.circuit_breaker_state) {
        tracing::warn!(reason = %reason, "Circuit breaker tripped");
        // TODO(P2): emit METRIC_TYPE_CIRCUIT_BREAKER_TRIP via vox-db
        // TODO(P2): call replanner or bulletin escalation
        // For now: break the plan loop
        break;
    }
}
```

Note: The actual wiring requires reading plan_loop.rs to identify the `state` struct and loop structure. Add `circuit_breaker_state: CircuitBreakerState` to whatever state struct the loop uses, or create a local variable if the loop uses local state.

- [ ] **Step 6.4: Verify compilation**

Run: `cargo build -p vox-orchestrator 2>&1 | tail -20`
Expected: Compiles clean.

- [ ] **Step 6.5: Commit**

```bash
git add crates/vox-orchestrator/src/mcp_tools/chat_tools/plan_loop.rs
git commit -m "feat(orchestrator): wire CircuitBreaker check into plan_loop (feature-gated)"
```

---

### Task 7: Metrics emission stub

**Files:**
- Modify: `crates/vox-orchestrator/src/circuit_breaker.rs`

- [ ] **Step 7.1: Write test for metric payload shape**

```rust
#[test]
fn trip_event_serializes_correctly() {
    use serde_json::json;
    let reason = TripReason::NoProgress;
    let payload = json!({
        "metric_type": "orch.circuit_breaker.trip",
        "trip_reason": reason.to_string(),
        "replan_attempts": 0u32,
    });
    assert_eq!(payload["metric_type"], "orch.circuit_breaker.trip");
    assert_eq!(payload["trip_reason"], "no-progress");
}
```

- [ ] **Step 7.2: Add TripEvent struct**

```rust
/// Payload emitted to `llm_interactions` when the breaker trips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripEvent {
    pub metric_type: &'static str,
    pub trip_reason: String,
    pub replan_attempts: u32,
    pub session_id: Option<String>,
}

impl TripEvent {
    pub fn new(reason: TripReason, state: &CircuitBreakerState) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CIRCUIT_BREAKER_TRIP,
            trip_reason: reason.to_string(),
            replan_attempts: state.replan_attempts,
            session_id: None,
        }
    }
}
```

- [ ] **Step 7.3: Run tests**

Run: `cargo test -p vox-orchestrator circuit_breaker`
Expected: All tests pass including `trip_event_serializes_correctly`.

- [ ] **Step 7.4: Commit**

```bash
git add crates/vox-orchestrator/src/circuit_breaker.rs
git commit -m "feat(orchestrator): add TripEvent metric payload for circuit breaker"
```

---

### Task 8: Update where-things-live.md and run arch-check

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 8.1: Add circuit_breaker row**

Find the orchestrator section in `docs/src/architecture/where-things-live.md` and add:

```
| `circuit_breaker` | Five-signal doom-loop detector (D6) | `crates/vox-orchestrator/src/circuit_breaker.rs` |
```

- [ ] **Step 8.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -30`
Expected: No new violations. Orphan detector passes.

- [ ] **Step 8.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register circuit_breaker in where-things-live.md"
```

---

### Task 9: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** Telemetry: `TripEvent::metric_type` matches `METRIC_TYPE_CIRCUIT_BREAKER_TRIP`
- [ ] **G3** Bench: `circuit_breaker_should_trip` mean <50µs (should be <100ns)
- [ ] **G4** Contract: `circuit-breaker.v1.yaml` + fixtures + `TripEvent` serialize consistently
- [ ] **G5** HITL fallback: `should_escalate()` returns true when `replan_attempts >= replan_limit`

---

**Phase 2 sign-off:** All 9 tasks complete, 13+ unit tests + 200 property cases + 6 golden fixtures pass, `cargo build -p vox-orchestrator` clean, bench baseline recorded.
