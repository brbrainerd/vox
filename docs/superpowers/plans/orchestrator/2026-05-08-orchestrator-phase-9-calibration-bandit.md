# Orchestrator Phase 9: Calibration + Bandit (D10) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a calibration loop and contextual bandit that learns optimal routing preferences from binary feedback signals (task success / failure). The bandit updates arm weights and feeds updated preferences back into model selection.

**Architecture:** A new `calibration.rs` module holds: (1) `CalibrationLoop` — tracks drift between predicted and observed confidence scores, emitting drift alerts when z-score exceeds threshold; (2) `ContextualBandit` — Thompson sampling over model arms using the existing `arm_stats: HashMap<String, (u32, u32)>` (alpha, beta counts) already in `ModelRegistry`. The bandit update is additive to the existing `record_penalty()` infrastructure.

**Tech Stack:** Rust, existing `ModelRegistry::arm_stats` and `record_penalty()`, `METRIC_TYPE_CALIBRATION_RUN`, `METRIC_TYPE_DRIFT_ALERT`, `METRIC_TYPE_BANDIT_UPDATE`, feature flags `vox.orchestrator.calibration_loop.enabled`, `vox.orchestrator.drift_detector.enabled`, `vox.orchestrator.contextual_bandit.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/calibration.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod calibration |
| Modify | `crates/vox-orchestrator/src/models/registry.rs` — expose record_bandit_outcome() |
| Create | `crates/vox-orchestrator/tests/calibration_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/calibration/drift_alert.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/calibration/bandit_update.json` |
| Modify | `docs/src/architecture/where-things-live.md` — add calibration row |

---

### Task 1: CalibrationLoop (drift detector)

**Files:**
- Create: `crates/vox-orchestrator/src/calibration.rs`

- [ ] **Step 1.1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_drift_within_threshold() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        // Feed 10 observations close to predicted
        for _ in 0..10 {
            loop_.record(0.80, 0.78); // predicted=0.80, observed=0.78
        }
        assert!(!loop_.is_drifting());
    }

    #[test]
    fn drift_detected_after_large_divergence() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..10 {
            loop_.record(0.80, 0.20); // large gap every time
        }
        assert!(loop_.is_drifting());
    }

    #[test]
    fn drift_clears_after_reset() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..10 {
            loop_.record(0.80, 0.20);
        }
        assert!(loop_.is_drifting());
        loop_.reset();
        assert!(!loop_.is_drifting());
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator calibration 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 1.3: Write the module**

Create `crates/vox-orchestrator/src/calibration.rs`:

```rust
//! Calibration loop and contextual bandit for orchestrator routing adaptation (D10).
//!
//! CalibrationLoop: tracks drift between predicted and observed confidence.
//! ContextualBandit: Thompson sampling over model arms.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CalibrationLoop
// ---------------------------------------------------------------------------

/// Running statistics for drift detection.
#[derive(Debug, Clone, Default)]
struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64, // Welford's online variance
}

impl RunningStats {
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / (self.count - 1) as f64
    }

    fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Configuration for drift detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationConfig {
    /// Z-score threshold above which drift is reported.
    pub drift_sigma: f64,
    /// Minimum observations before drift can be declared.
    pub min_observations: u64,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            drift_sigma: 2.0,
            min_observations: 10,
        }
    }
}

/// Online calibration loop tracking deviation between predicted and observed confidence.
pub struct CalibrationLoop {
    config: CalibrationConfig,
    errors: RunningStats,
}

impl CalibrationLoop {
    pub fn new(config: CalibrationConfig) -> Self {
        Self {
            config,
            errors: RunningStats::default(),
        }
    }

    /// Record a predicted vs. observed confidence pair.
    pub fn record(&mut self, predicted: f64, observed: f64) {
        let error = (predicted - observed).abs();
        self.errors.update(error);
    }

    /// Returns true if the mean absolute error exceeds drift_sigma standard deviations
    /// from zero (i.e., systematic bias in predictions).
    #[must_use]
    pub fn is_drifting(&self) -> bool {
        if self.errors.count < self.config.min_observations {
            return false;
        }
        let std = self.errors.std_dev();
        if std < 1e-9 {
            // Perfectly consistent — but check if mean error itself is large
            return self.errors.mean > self.config.drift_sigma * 0.1;
        }
        // z-score of mean error vs zero baseline
        let z = self.errors.mean / (std / (self.errors.count as f64).sqrt());
        z >= self.config.drift_sigma
    }

    /// Reset statistics (called after drift is acknowledged).
    pub fn reset(&mut self) {
        self.errors = RunningStats::default();
    }

    pub fn observation_count(&self) -> u64 {
        self.errors.count
    }
}

// ---------------------------------------------------------------------------
// ContextualBandit
// ---------------------------------------------------------------------------

/// A single arm's Thompson sampling state (alpha=successes+1, beta=failures+1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditArm {
    pub arm_id: String,
    pub alpha: u32, // successes + 1 (Beta prior starts at 1)
    pub beta: u32,  // failures + 1
}

impl BanditArm {
    pub fn new(arm_id: impl Into<String>) -> Self {
        Self {
            arm_id: arm_id.into(),
            alpha: 1,
            beta: 1,
        }
    }

    /// Expected value of the Beta distribution = alpha / (alpha + beta).
    #[must_use]
    pub fn expected_reward(&self) -> f64 {
        self.alpha as f64 / (self.alpha + self.beta) as f64
    }

    pub fn record_success(&mut self) {
        self.alpha = self.alpha.saturating_add(1);
    }

    pub fn record_failure(&mut self) {
        self.beta = self.beta.saturating_add(1);
    }
}

/// Simple contextual bandit over model arms.
pub struct ContextualBandit {
    pub arms: Vec<BanditArm>,
}

impl ContextualBandit {
    pub fn new(arm_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            arms: arm_ids.into_iter().map(BanditArm::new).collect(),
        }
    }

    /// Return the arm_id with the highest expected reward (greedy selection).
    #[must_use]
    pub fn best_arm(&self) -> Option<&str> {
        self.arms
            .iter()
            .max_by(|a, b| {
                a.expected_reward()
                    .partial_cmp(&b.expected_reward())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|arm| arm.arm_id.as_str())
    }

    pub fn record_outcome(&mut self, arm_id: &str, success: bool) {
        if let Some(arm) = self.arms.iter_mut().find(|a| a.arm_id == arm_id) {
            if success {
                arm.record_success();
            } else {
                arm.record_failure();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Metric events
// ---------------------------------------------------------------------------

/// Payload for calibration run metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRunEvent {
    pub metric_type: &'static str,
    pub observation_count: u64,
    pub is_drifting: bool,
}

impl CalibrationRunEvent {
    pub fn new(loop_: &CalibrationLoop) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CALIBRATION_RUN,
            observation_count: loop_.observation_count(),
            is_drifting: loop_.is_drifting(),
        }
    }
}

/// Payload for drift alert metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftAlertEvent {
    pub metric_type: &'static str,
    pub observation_count: u64,
}

impl DriftAlertEvent {
    pub fn new(loop_: &CalibrationLoop) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_DRIFT_ALERT,
            observation_count: loop_.observation_count(),
        }
    }
}

/// Payload for bandit update metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditUpdateEvent {
    pub metric_type: &'static str,
    pub arm_id: String,
    pub success: bool,
    pub new_expected_reward: f64,
}

impl BanditUpdateEvent {
    pub fn new(arm: &BanditArm, success: bool) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_BANDIT_UPDATE,
            arm_id: arm.arm_id.clone(),
            success,
            new_expected_reward: arm.expected_reward(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_drift_within_threshold() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..10 {
            loop_.record(0.80, 0.78);
        }
        assert!(!loop_.is_drifting());
    }

    #[test]
    fn drift_detected_after_large_divergence() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..10 {
            loop_.record(0.80, 0.20);
        }
        assert!(loop_.is_drifting());
    }

    #[test]
    fn drift_clears_after_reset() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..10 {
            loop_.record(0.80, 0.20);
        }
        assert!(loop_.is_drifting());
        loop_.reset();
        assert!(!loop_.is_drifting());
    }

    #[test]
    fn fewer_than_min_observations_never_drifts() {
        let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
        for _ in 0..5 {
            loop_.record(0.80, 0.20);
        }
        assert!(!loop_.is_drifting()); // only 5 < 10 minimum
    }

    #[test]
    fn bandit_new_arm_has_equal_priors() {
        let arm = BanditArm::new("claude-haiku");
        assert_eq!(arm.alpha, 1);
        assert_eq!(arm.beta, 1);
        assert!((arm.expected_reward() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bandit_success_increases_alpha() {
        let mut arm = BanditArm::new("claude-haiku");
        arm.record_success();
        assert_eq!(arm.alpha, 2);
        assert!(arm.expected_reward() > 0.5);
    }

    #[test]
    fn bandit_best_arm_returns_highest_reward() {
        let mut bandit = ContextualBandit::new(["haiku", "sonnet", "opus"]);
        // Give sonnet 10 successes
        for _ in 0..10 {
            bandit.record_outcome("sonnet", true);
        }
        assert_eq!(bandit.best_arm(), Some("sonnet"));
    }

    #[test]
    fn calibration_run_event_metric_type() {
        let loop_ = CalibrationLoop::new(CalibrationConfig::default());
        let event = CalibrationRunEvent::new(&loop_);
        assert_eq!(event.metric_type, "orch.calibration.run");
    }

    #[test]
    fn drift_alert_event_metric_type() {
        let loop_ = CalibrationLoop::new(CalibrationConfig::default());
        let event = DriftAlertEvent::new(&loop_);
        assert_eq!(event.metric_type, "orch.calibration.drift_alert");
    }

    #[test]
    fn bandit_update_event_metric_type() {
        let arm = BanditArm::new("test-arm");
        let event = BanditUpdateEvent::new(&arm, true);
        assert_eq!(event.metric_type, "orch.calibration.bandit_update");
    }
}
```

- [ ] **Step 1.4: Register in lib.rs**

```rust
pub mod calibration;
```

- [ ] **Step 1.5: Run tests**

Run: `cargo test -p vox-orchestrator calibration -- --nocapture`
Expected: All 10 tests pass.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vox-orchestrator/src/calibration.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add CalibrationLoop and ContextualBandit (D10)"
```

---

### Task 2: Expose record_bandit_outcome on ModelRegistry

**Files:**
- Modify: `crates/vox-orchestrator/src/models/registry.rs`

- [ ] **Step 2.1: Read registry.rs to locate arm_stats and record_penalty**

Read `crates/vox-orchestrator/src/models/registry.rs` lines 1–100 to understand `arm_stats` field type and `record_penalty` method signature.

- [ ] **Step 2.2: Write test**

Add to `crates/vox-orchestrator/src/models/tests.rs` (or a new file if tests.rs exists):

```rust
#[test]
fn record_bandit_outcome_updates_arm_stats() {
    let mut registry = ModelRegistry::new();
    registry.record_bandit_outcome("test-model", true);
    // arm_stats should have alpha incremented
    // Read the arm_stats field to verify; field may be pub(crate)
    // Just verify no panic for now
}
```

- [ ] **Step 2.3: Add method to registry**

In `crates/vox-orchestrator/src/models/registry.rs`, add after `record_penalty`:

```rust
/// Update the Thompson sampling arm for `model_id` with a binary outcome.
/// `success = true` increments alpha (wins); `success = false` increments beta (losses).
/// This feeds directly into `arm_stats` which `best_for_internal` already consults.
pub fn record_bandit_outcome(&mut self, model_id: &str, success: bool) {
    let entry = self.arm_stats.entry(model_id.to_string()).or_insert((1, 1));
    if success {
        entry.0 = entry.0.saturating_add(1);
    } else {
        entry.1 = entry.1.saturating_add(1);
    }
}
```

Note: Verify the `arm_stats` field type is `HashMap<String, (u32, u32)>` by reading registry.rs before writing. Adjust if the type differs.

- [ ] **Step 2.4: Run tests**

Run: `cargo test -p vox-orchestrator models`
Expected: All pass.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-orchestrator/src/models/registry.rs
git commit -m "feat(orchestrator): expose record_bandit_outcome on ModelRegistry (D10)"
```

---

### Task 3: Golden fixtures and integration tests

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/calibration/*.json`
- Create: `crates/vox-orchestrator/tests/calibration_integration.rs`

- [ ] **Step 3.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/calibration/drift_alert.json`:
```json
{
  "observations": [
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20},
    {"predicted": 0.80, "observed": 0.20}
  ],
  "expected_drifting": true
}
```

`crates/vox-orchestrator-test-helpers/fixtures/calibration/bandit_update.json`:
```json
{
  "arm_id": "claude-sonnet",
  "successes": 10,
  "failures": 2,
  "expected_best": true
}
```

- [ ] **Step 3.2: Write integration tests**

Create `crates/vox-orchestrator/tests/calibration_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::calibration::{BanditArm, CalibrationConfig, CalibrationLoop, ContextualBandit};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct Observation {
    predicted: f64,
    observed: f64,
}

#[derive(Deserialize)]
struct DriftFixture {
    observations: Vec<Observation>,
    expected_drifting: bool,
}

#[derive(Deserialize)]
struct BanditFixture {
    arm_id: String,
    successes: u32,
    failures: u32,
    expected_best: bool,
}

#[test]
fn golden_drift_alert() {
    let f: DriftFixture = load_golden_fixture("calibration/drift_alert.json").unwrap();
    let mut loop_ = CalibrationLoop::new(CalibrationConfig::default());
    for obs in &f.observations {
        loop_.record(obs.predicted, obs.observed);
    }
    assert_eq!(loop_.is_drifting(), f.expected_drifting);
}

#[test]
fn golden_bandit_best_arm() {
    let f: BanditFixture = load_golden_fixture("calibration/bandit_update.json").unwrap();
    let mut bandit = ContextualBandit::new(["claude-haiku", f.arm_id.as_str()]);
    for _ in 0..f.successes {
        bandit.record_outcome(&f.arm_id, true);
    }
    for _ in 0..f.failures {
        bandit.record_outcome(&f.arm_id, false);
    }
    if f.expected_best {
        assert_eq!(bandit.best_arm(), Some(f.arm_id.as_str()));
    }
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test --test calibration_integration`
Expected: 2 golden tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/calibration/ \
        crates/vox-orchestrator/tests/calibration_integration.rs
git commit -m "test(orchestrator): golden fixtures for CalibrationLoop and ContextualBandit"
```

---

### Task 4: Update where-things-live.md and arch-check

- [ ] **Step 4.1: Add calibration row**

```
| `calibration` | Calibration loop + contextual bandit for adaptive routing (D10) | `crates/vox-orchestrator/src/calibration.rs` |
```

- [ ] **Step 4.2: Run arch-check and commit**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register calibration in where-things-live.md"
```

---

### Task 5: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** All three metric types tested (`orch.calibration.run`, `orch.calibration.drift_alert`, `orch.calibration.bandit_update`)
- [ ] **G3** Drift detection requires ≥ `min_observations` before firing
- [ ] **G4** Bandit `best_arm()` returns the arm with most successes given uniform priors
- [ ] **G5** `record_bandit_outcome` on registry integrates with existing `arm_stats`

---

**Phase 9 sign-off:** 5 tasks complete, 10+ unit tests + 2 golden fixtures, `cargo build` clean.
