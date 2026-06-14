//! Scaling policy loaded from the repo SSOT [`contracts/scaling/policy.yaml`](../../contracts/scaling/policy.yaml).
//!
//! Use [`ScalingPolicy::embedded`] for stable defaults without I/O. For local overrides in tests,
//! use [`ScalingPolicy::from_yaml_str`].

use serde::Deserialize;

pub mod cost_defense;
mod policy_types;

pub use cost_defense::{
    CostCircuitBreaker, CostDefenseConfig, CostDefenseRejection, CostDefenseState,
};
pub use policy_types::{PathLiterals, PerCrateOverride, Thresholds};

/// Parse, edit, and pretty-print mesh `donations.vox` policy files (formerly `vox-mesh-policy`).
pub mod donations_vox;
/// Alias for [`donations_vox`] (former `vox-mesh-policy` crate surface).
pub mod mesh_policy {
    pub use super::donations_vox::*;
}
pub use donations_vox::{ParseError as DonationsVoxParseError, load_policy, pretty_print};

const EMBEDDED_YAML: &str = include_str!("../../../contracts/scaling/policy.yaml");

/// Repo-root-relative path to the scaling policy YAML SSOT (for docs, CLI messages, and tooling).
pub const SCALING_POLICY_YAML_REPO_PATH: &str = "contracts/scaling/policy.yaml";

/// Full policy document from SSOT YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct ScalingPolicy {
    #[serde(default)]
    pub schema_version: u32,
    /// Human-readable baseline id (e.g. git tag or date).
    #[serde(default)]
    pub baseline_id: String,
    #[serde(default)]
    pub thresholds: Thresholds,
    #[serde(default)]
    pub path_literals: PathLiterals,
    #[serde(default)]
    pub magic_numeric_hints: Vec<u64>,
    #[serde(default)]
    pub per_crate_overrides: Vec<PerCrateOverride>,
}

impl ScalingPolicy {
    /// Policy embedded at build time from `contracts/scaling/policy.yaml`.
    pub fn embedded() -> Self {
        Self::from_yaml_str(EMBEDDED_YAML).expect("embedded scaling policy YAML must parse")
    }

    /// Parse policy from YAML (for tests or tooling).
    pub fn from_yaml_str(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }
}

// ---------------------------------------------------------------------------
// Stable path constants (SSOT-backed; keep literals out of call sites)
// ---------------------------------------------------------------------------

/// Canonical default for Mens/Populi run artifact root (repo-relative).
pub const DEFAULT_MENS_RUNS_ROOT: &str = "mens/runs";

/// Latest symlink / pointer directory under [`DEFAULT_MENS_RUNS_ROOT`].
pub const DEFAULT_MENS_RUNS_LATEST: &str = "mens/runs/latest";

/// Default training run layout revision under [`DEFAULT_MENS_RUNS_ROOT`].
pub const DEFAULT_MENS_RUNS_V1: &str = "mens/runs/v1";

/// Default UV / tooling output subdirectory under runs.
pub const DEFAULT_MENS_RUNS_UV_OUTPUT: &str = "mens/runs/uv_output";

/// Default QLoRA run directory basename (repo-relative path prefix).
pub const DEFAULT_MENS_RUNS_QWEN_QLORA: &str = "mens/runs/qwen35_qlora";

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: ScalingPolicy::from_yaml_str returning Ok on completely invalid YAML,
    // silently filling in all defaults and hiding that the input was garbage.
    #[test]
    fn from_yaml_str_rejects_invalid_yaml() {
        let bad = "{{{{not valid yaml at all}}}}";
        let result = ScalingPolicy::from_yaml_str(bad);
        assert!(result.is_err(), "invalid YAML must return Err, not silently default");
    }

    // Catches: schema_version field being silently zeroed out by serde when the
    // key is present in YAML, losing version tracking.
    #[test]
    fn from_yaml_str_preserves_schema_version() {
        let yaml = "schema_version: 42\nbaseline_id: test";
        let p = ScalingPolicy::from_yaml_str(yaml).expect("valid YAML must parse");
        assert_eq!(p.schema_version, 42, "schema_version must be preserved");
    }

    // Catches: CostCircuitBreaker::record_task_completion not incrementing retry
    // count, allowing an infinite retry loop to pass Layer 2 checks indefinitely.
    #[test]
    fn retry_count_monotonically_increases_after_completions() {
        let mut cb = CostCircuitBreaker::new(CostDefenseConfig::default());
        for i in 1..=5u32 {
            cb.record_task_completion("task-loop", "tenant-a", 0.01);
            let count = cb.state.task_retry_counts.get("task-loop").copied().unwrap_or(0);
            assert_eq!(count, i, "retry count must equal number of completions at step {i}");
        }
    }

    // Catches: reset_daily accidentally clearing monthly_spent_usd, making the
    // monthly pacing layer blind to prior spend after each daily reset.
    #[test]
    fn reset_daily_does_not_touch_monthly_spend() {
        let mut cb = CostCircuitBreaker::new(CostDefenseConfig::default());
        cb.state.monthly_spent_usd = 100.0;
        cb.state.daily_spent_usd = 10.0;
        cb.state.reset_daily();
        assert!(
            (cb.state.monthly_spent_usd - 100.0).abs() < f64::EPSILON,
            "reset_daily must not alter monthly_spent_usd"
        );
        assert!(
            cb.state.daily_spent_usd.abs() < f64::EPSILON,
            "reset_daily must clear daily_spent_usd"
        );
    }

    // Catches: check_before_task incorrectly rejecting a zero-cost task when the
    // daily budget is exactly at its limit (boundary off-by-one with strict `>`).
    #[test]
    fn daily_budget_exactly_at_limit_with_zero_cost_not_rejected() {
        let mut cb = CostCircuitBreaker::new(CostDefenseConfig::default());
        cb.state.daily_spent_usd = cb.config.daily_budget_usd;
        let r = cb.check_before_task(60, "t-zero", "tenant-a", "local", 0.0);
        let budget_rejection = r.iter().any(|x| {
            matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. })
        });
        assert!(
            !budget_rejection,
            "zero-cost task at exact budget boundary must not trigger DailyBudgetExhausted"
        );
    }

    // Catches: model pinning check using case-sensitive comparison so "LOCAL" is
    // rejected even though "local" is in allowed_model_tiers.
    #[test]
    fn model_pinning_case_insensitive_match() {
        let cb = CostCircuitBreaker::new(CostDefenseConfig::default());
        for tier in &["LOCAL", "Mid", "FRONTIER"] {
            let r = cb.check_before_task(60, "t1", "tenant-a", tier, 0.01);
            let pinned = r.iter().any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. }));
            assert!(!pinned, "model tier {tier:?} must match case-insensitively");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_parses() {
        let p = ScalingPolicy::embedded();
        assert!(p.schema_version >= 1);
        assert!(!p.path_literals.mens_runs_variants.is_empty());
    }
}
