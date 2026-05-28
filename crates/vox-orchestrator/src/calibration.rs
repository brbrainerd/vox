//! Calibration loop: Welford drift detection + greedy Beta-posterior bandit (D10).
//!
//! `CalibrationLoop` tracks a running mean/variance over latency or quality scores
//! and alerts when the z-score of a new observation exceeds the drift threshold.
//! `ContextualBandit` picks the arm with the highest expected reward under each
//! arm's Beta(α, β) posterior — i.e. greedy exploitation. Thompson draws live in
//! [`crate::routing::ModelSelectionEngine`] using aligned `(successes, failures)` counts.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Welford online statistics ─────────────────────────────────────────────────

/// Running mean and variance via Welford's online algorithm.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunningStats {
    pub count: u64,
    pub mean: f64,
    pub m2: f64,
}

impl RunningStats {
    /// Update with a new observation.
    #[inline]
    pub fn update(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    /// Sample variance (unbiased). Returns 0.0 when count < 2.
    #[must_use]
    #[inline]
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// Sample standard deviation. Returns 0.0 when count < 2.
    #[must_use]
    #[inline]
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Z-score of a new observation against the current baseline.
    /// Returns 0.0 when std_dev is zero.
    #[must_use]
    #[inline]
    pub fn z_score(&self, x: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-12 {
            0.0
        } else {
            (x - self.mean) / sd
        }
    }
}

// ── Calibration loop ─────────────────────────────────────────────────────────

/// Configuration for the calibration loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationConfig {
    /// Minimum observations before drift alerts are emitted.
    pub min_observations: u64,
    /// |z-score| ≥ this triggers a drift alert.
    pub drift_sigma_threshold: f64,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            min_observations: 10,
            drift_sigma_threshold: 2.0,
        }
    }
}

/// Outcome of a calibration check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationOutcome {
    pub z_score: f64,
    pub drift_detected: bool,
    pub observation_count: u64,
}

/// Online calibration loop wrapping [`RunningStats`].
pub struct CalibrationLoop {
    config: CalibrationConfig,
    stats: RunningStats,
}

impl CalibrationLoop {
    pub fn new(config: CalibrationConfig) -> Self {
        Self {
            config,
            stats: RunningStats::default(),
        }
    }

    /// Record a new observation and check for drift.
    #[must_use]
    pub fn observe(&mut self, value: f64) -> CalibrationOutcome {
        let z = if self.stats.count >= self.config.min_observations {
            self.stats.z_score(value)
        } else {
            0.0
        };
        self.stats.update(value);
        let drift = self.stats.count >= self.config.min_observations
            && z.abs() >= self.config.drift_sigma_threshold;
        CalibrationOutcome {
            z_score: z,
            drift_detected: drift,
            observation_count: self.stats.count,
        }
    }

    pub fn stats(&self) -> &RunningStats {
        &self.stats
    }
}

// ── Contextual bandit ─────────────────────────────────────────────────────────

/// A single bandit arm tracking a Beta posterior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditArm {
    pub model_id: String,
    pub successes: u32,
    pub failures: u32,
}

impl BanditArm {
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            successes: 0,
            failures: 0,
        }
    }

    /// Expected reward under Beta(α=successes+1, β=failures+1): mean = α/(α+β).
    #[must_use]
    #[inline]
    pub fn expected_reward(&self) -> f64 {
        let alpha = self.successes as f64 + 1.0;
        let beta = self.failures as f64 + 1.0;
        alpha / (alpha + beta)
    }

    pub fn record(&mut self, success: bool) {
        if success {
            self.successes = self.successes.saturating_add(1);
        } else {
            self.failures = self.failures.saturating_add(1);
        }
    }
}

/// Greedy contextual bandit over a set of model arms.
/// Picks the arm with the highest expected reward under its Beta posterior.
/// Stochastic exploration is delegated to `routing::engine` (Thompson draws) so this
/// module stays pure and deterministic — no RNG required, no I/O.
pub struct ContextualBandit {
    arms: Vec<BanditArm>,
}

impl ContextualBandit {
    pub fn new(arms: Vec<BanditArm>) -> Self {
        Self { arms }
    }

    /// Select the arm with highest expected reward. Returns `None` if no arms.
    #[must_use]
    pub fn select(&self) -> Option<&BanditArm> {
        self.arms.iter().max_by(|a, b| {
            a.expected_reward()
                .partial_cmp(&b.expected_reward())
                .unwrap()
        })
    }

    /// Record an outcome for the arm identified by `model_id`.
    pub fn record_outcome(&mut self, model_id: &str, success: bool) {
        if let Some(arm) = self.arms.iter_mut().find(|a| a.model_id == model_id) {
            arm.record(success);
        }
    }

    pub fn arms(&self) -> &[BanditArm] {
        &self.arms
    }
}

#[must_use]
pub fn arm_stats_from_bandit(bandit: &ContextualBandit) -> HashMap<String, (u32, u32)> {
    bandit
        .arms()
        .iter()
        .map(|a| (a.model_id.clone(), (a.successes, a.failures)))
        .collect()
}

// ── Metric payloads ───────────────────────────────────────────────────────────

/// Serialize-only — `&'static str` is incompatible with `Deserialize` and round-tripping isn't needed.
#[derive(Debug, Clone, Serialize)]
pub struct CalibrationRunEvent {
    pub metric_type: &'static str,
    pub z_score: f64,
    pub observation_count: u64,
    pub session_id: Option<String>,
}

impl CalibrationRunEvent {
    pub fn new(outcome: &CalibrationOutcome, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CALIBRATION_RUN,
            z_score: outcome.z_score,
            observation_count: outcome.observation_count,
            session_id,
        }
    }
}

/// Serialize-only — see `CalibrationRunEvent` for rationale.
#[derive(Debug, Clone, Serialize)]
pub struct DriftAlertEvent {
    pub metric_type: &'static str,
    pub z_score: f64,
    pub session_id: Option<String>,
}

impl DriftAlertEvent {
    pub fn new(z_score: f64, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_DRIFT_ALERT,
            z_score,
            session_id,
        }
    }
}

/// Serialize-only — see `CalibrationRunEvent` for rationale.
#[derive(Debug, Clone, Serialize)]
pub struct BanditUpdateEvent {
    pub metric_type: &'static str,
    pub model_id: String,
    pub success: bool,
    pub expected_reward: f64,
    pub session_id: Option<String>,
}

impl BanditUpdateEvent {
    pub fn new(arm: &BanditArm, success: bool, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_BANDIT_UPDATE,
            model_id: arm.model_id.clone(),
            success,
            expected_reward: arm.expected_reward(),
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welford_mean_converges() {
        let mut s = RunningStats::default();
        for x in [1.0, 2.0, 3.0, 4.0, 5.0] {
            s.update(x);
        }
        assert!((s.mean - 3.0).abs() < 1e-9);
    }

    #[test]
    fn welford_variance_known_sequence() {
        let mut s = RunningStats::default();
        for x in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            s.update(x);
        }
        // population var = 4.0, sample var = 4.571...
        assert!((s.variance() - (32.0 / 7.0)).abs() < 1e-6);
    }

    #[test]
    fn z_score_zero_before_min_observations() {
        let mut cal = CalibrationLoop::new(CalibrationConfig {
            min_observations: 5,
            ..Default::default()
        });
        for _ in 0..4 {
            let out = cal.observe(1.0);
            assert!(!out.drift_detected);
            assert_eq!(out.z_score, 0.0);
        }
    }

    #[test]
    fn drift_detected_on_outlier() {
        let mut cal = CalibrationLoop::new(CalibrationConfig::default());
        // Baseline with realistic variance so std_dev > 0
        for x in [1.0f64, 2.0, 1.5, 2.5, 1.0, 2.0, 1.5, 2.5, 1.0, 2.0] {
            let _ = cal.observe(x);
        }
        let out = cal.observe(50.0);
        assert!(out.drift_detected, "z_score={}", out.z_score);
        assert!(out.z_score > 2.0);
    }

    #[test]
    fn no_drift_on_in_range_observation() {
        let mut cal = CalibrationLoop::new(CalibrationConfig::default());
        for x in [1.0, 2.0, 3.0, 2.0, 1.0, 2.0, 3.0, 2.0, 1.0, 2.0] {
            let _ = cal.observe(x);
        }
        let out = cal.observe(2.0);
        assert!(!out.drift_detected);
    }

    #[test]
    fn bandit_arm_uninformed_prior_is_half() {
        let arm = BanditArm::new("m1");
        assert!((arm.expected_reward() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bandit_arm_all_successes_approaches_one() {
        let mut arm = BanditArm::new("m1");
        for _ in 0..100 {
            arm.record(true);
        }
        assert!(arm.expected_reward() > 0.99);
    }

    #[test]
    fn bandit_selects_arm_with_highest_reward() {
        let arms = vec![
            BanditArm {
                model_id: "weak".into(),
                successes: 1,
                failures: 9,
            },
            BanditArm {
                model_id: "strong".into(),
                successes: 9,
                failures: 1,
            },
        ];
        let bandit = ContextualBandit::new(arms);
        assert_eq!(bandit.select().unwrap().model_id, "strong");
    }

    #[test]
    fn bandit_record_outcome_updates_arm() {
        let mut bandit = ContextualBandit::new(vec![BanditArm::new("m1")]);
        bandit.record_outcome("m1", true);
        bandit.record_outcome("m1", false);
        let arm = &bandit.arms()[0];
        assert_eq!(arm.successes, 1);
        assert_eq!(arm.failures, 1);
    }

    #[test]
    fn arm_stats_from_bandit_maps_models() {
        let bandit = ContextualBandit::new(vec![BanditArm::new("m-a"), BanditArm::new("m-b")]);
        let m = arm_stats_from_bandit(&bandit);
        assert_eq!(m.len(), 2);
        assert_eq!(m.get("m-a").copied(), Some((0, 0)));
    }

    #[test]
    fn calibration_run_event_has_correct_metric_type() {
        let outcome = CalibrationOutcome {
            z_score: 0.0,
            drift_detected: false,
            observation_count: 1,
        };
        let ev = CalibrationRunEvent::new(&outcome, None);
        assert_eq!(ev.metric_type, "orch.calibration.run");
    }

    #[test]
    fn drift_alert_event_has_correct_metric_type() {
        let ev = DriftAlertEvent::new(3.5, None);
        assert_eq!(ev.metric_type, "orch.calibration.drift_alert");
    }

    #[test]
    fn bandit_update_event_has_correct_metric_type() {
        let arm = BanditArm::new("m1");
        let ev = BanditUpdateEvent::new(&arm, true, None);
        assert_eq!(ev.metric_type, "orch.calibration.bandit_update");
    }
}
