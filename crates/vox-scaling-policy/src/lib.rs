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
        assert!(
            result.is_err(),
            "invalid YAML must return Err, not silently default"
        );
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
            let count = cb
                .state
                .task_retry_counts
                .get("task-loop")
                .copied()
                .unwrap_or(0);
            assert_eq!(
                count, i,
                "retry count must equal number of completions at step {i}"
            );
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
        let budget_rejection = r
            .iter()
            .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. }));
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
            let pinned = r
                .iter()
                .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. }));
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

#[cfg(test)]
mod semcov_wave42_tests {
    use super::*;
    use cost_defense::{
        CostCircuitBreaker, CostDefenseConfig, CostDefenseRejection, CostDefenseState,
    };
    use std::collections::HashMap;

    fn breaker_with(cfg: CostDefenseConfig) -> CostCircuitBreaker {
        CostCircuitBreaker::new(cfg)
    }

    fn clean_breaker() -> CostCircuitBreaker {
        CostCircuitBreaker::new(CostDefenseConfig::default())
    }

    // Catches: Layer 1 using `>=` instead of `>`, causing tasks that match the
    // timeout exactly to be rejected instead of allowed.
    #[test]
    fn layer1_exactly_at_timeout_limit_is_allowed() {
        let cb = clean_breaker();
        let limit = cb.config.per_task_timeout_secs;
        let r = cb.check_before_task(limit, "t-exact", "tenant-a", "local", 0.01);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::TaskTimeout { .. })),
            "task at exactly the timeout limit must not be rejected"
        );
    }

    // Catches: Layer 2 trigger firing on first call (0 retries) because the
    // counter is initialised to 1 rather than 0, blocking novel tasks.
    #[test]
    fn layer2_fresh_task_id_never_blocked() {
        let cb = clean_breaker();
        let r = cb.check_before_task(60, "brand-new-task", "tenant-a", "local", 0.01);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::RetryLimitExceeded { .. })),
            "a task with no prior retries must never hit RetryLimitExceeded"
        );
    }

    // Catches: Layer 2 allowing one extra attempt because the comparison is `>`
    // instead of `>=`, letting tasks exceed max_retries_per_task_day by one.
    #[test]
    fn layer2_fires_at_exact_retry_limit_not_one_over() {
        let mut cb = clean_breaker();
        let limit = cb.config.max_retries_per_task_day;
        for _ in 0..limit {
            cb.state.record_retry("t-exact-retry");
        }
        let r = cb.check_before_task(60, "t-exact-retry", "tenant-a", "local", 0.01);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::RetryLimitExceeded { .. })),
            "retry count equal to limit must trigger RetryLimitExceeded"
        );
    }

    // Catches: Layer 3 adding estimated_cost to projected but comparing against
    // daily_spent (not the limit), making the budget check vacuously fail.
    #[test]
    fn layer3_projected_cost_compared_against_limit_not_spent() {
        let mut cb = clean_breaker();
        cb.state.daily_spent_usd = 0.0;
        // An enormous single-task cost that exceeds the daily limit from zero.
        let r = cb.check_before_task(60, "big-task", "tenant-a", "local", 9999.0);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. })),
            "a task whose estimated cost alone exceeds the daily limit must be rejected"
        );
    }

    // Catches: Layer 3 using the tenant spent instead of the global daily spent
    // when there is no tenant cap set, allowing overspend on the global budget.
    #[test]
    fn layer3_global_daily_budget_enforced_regardless_of_tenant_cap() {
        let mut cfg = CostDefenseConfig::default();
        // No tenant cap configured — only the global daily budget should apply.
        cfg.tenant_daily_caps.clear();
        let mut cb = breaker_with(cfg);
        cb.state.daily_spent_usd = 24.99;
        let r = cb.check_before_task(60, "t1", "any-tenant", "local", 0.02);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. })),
            "global daily budget must fire even when no tenant cap is set"
        );
    }

    // Catches: Layer 4 skipping the pinning check when allowed_model_tiers is
    // empty, silently permitting every model tier instead of blocking all.
    #[test]
    fn layer4_empty_allowed_tiers_blocks_every_model() {
        let mut cfg = CostDefenseConfig::default();
        cfg.model_pinning_enabled = true;
        cfg.allowed_model_tiers.clear();
        let cb = breaker_with(cfg);
        let r = cb.check_before_task(60, "t1", "tenant-a", "local", 0.01);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. })),
            "with no allowed tiers, every model must be rejected"
        );
    }

    // Catches: Layer 4 being skipped entirely when model_pinning_enabled is
    // false, but the code checking the flag using `!` or inverted logic.
    #[test]
    fn layer4_disabled_pinning_allows_unknown_tier() {
        let mut cfg = CostDefenseConfig::default();
        cfg.model_pinning_enabled = false;
        let cb = breaker_with(cfg);
        let r = cb.check_before_task(60, "t1", "tenant-a", "super-secret-model", 0.01);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. })),
            "with pinning disabled, any model tier must pass"
        );
    }

    // Catches: Layer 5 warn threshold computed as `pct * budget` but the
    // comparison using `>=` instead of `>`, triggering the warning one cent early.
    #[test]
    fn layer5_monthly_pacing_not_triggered_below_threshold() {
        let mut cfg = CostDefenseConfig::default();
        cfg.monthly_budget_usd = 100.0;
        cfg.monthly_pacing_warn_pct = 0.80; // warn at 80 USD
        let mut cb = breaker_with(cfg);
        // Spend exactly at threshold — projected = 79.99 + 0.0 = 79.99 < 80.0
        cb.state.monthly_spent_usd = 79.99;
        let r = cb.check_before_task(60, "t1", "tenant-a", "local", 0.0);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::MonthlyPacingWarning { .. })),
            "spend just below the monthly pacing threshold must not emit a warning"
        );
    }

    // Catches: Layer 5 treating the pacing warning as a hard block in
    // has_hard_block(), preventing tasks from being dispatched on monthly warnings.
    #[test]
    fn layer5_pacing_warning_is_soft_and_does_not_hard_block() {
        let mut cfg = CostDefenseConfig::default();
        cfg.monthly_budget_usd = 100.0;
        cfg.monthly_pacing_warn_pct = 0.80;
        let mut cb = breaker_with(cfg);
        cb.state.monthly_spent_usd = 95.0;
        let r = cb.check_before_task(60, "t1", "tenant-a", "local", 0.01);
        // Warning must appear…
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::MonthlyPacingWarning { .. })),
            "expected monthly pacing warning"
        );
        // …but must not be classified as a hard block.
        assert!(
            !CostCircuitBreaker::has_hard_block(&r),
            "a rejection list containing only MonthlyPacingWarning is not a hard block"
        );
    }

    // Catches: Layer 6 comparing `spent + cost > limit` correctly but using the
    // wrong tenant's balance (e.g. iterating all tenants and picking the wrong one).
    #[test]
    fn layer6_tenant_isolation_does_not_bleed_across_tenants() {
        let mut cfg = CostDefenseConfig::default();
        cfg.tenant_daily_caps.insert("cheap-tenant".into(), 1.0);
        cfg.tenant_daily_caps.insert("rich-tenant".into(), 1000.0);
        let mut cb = breaker_with(cfg);
        // cheap-tenant is at cap; rich-tenant has plenty of headroom.
        cb.state.record_cost("cheap-tenant", 1.0);

        let r = cb.check_before_task(60, "t1", "rich-tenant", "local", 0.50);
        assert!(
            !r.iter().any(|x| matches!(x, CostDefenseRejection::TenantBudgetExhausted { tenant_id, .. } if tenant_id == "rich-tenant")),
            "a tenant within its cap must not be rejected due to another tenant's overspend"
        );
    }

    // Catches: record_cost applying cost only to daily_spent_usd but not to
    // monthly_spent_usd, causing the monthly pacing layer to undercount.
    #[test]
    fn record_cost_updates_both_daily_and_monthly() {
        let mut state = CostDefenseState::default();
        state.record_cost("t", 5.0);
        assert!(
            (state.daily_spent_usd - 5.0).abs() < f64::EPSILON,
            "daily must increase"
        );
        assert!(
            (state.monthly_spent_usd - 5.0).abs() < f64::EPSILON,
            "monthly must increase"
        );
    }

    // Catches: record_cost adding to monthly_spent_usd but creating a new entry
    // for tenant_spent_usd instead of accumulating, losing multi-call spend history.
    #[test]
    fn record_cost_accumulates_tenant_spend_across_multiple_calls() {
        let mut state = CostDefenseState::default();
        state.record_cost("tenant-x", 3.0);
        state.record_cost("tenant-x", 7.0);
        let got = state
            .tenant_spent_usd
            .get("tenant-x")
            .copied()
            .unwrap_or(0.0);
        assert!(
            (got - 10.0).abs() < f64::EPSILON,
            "tenant spend must accumulate; got {got}"
        );
    }

    // Catches: reset_monthly forgetting to invoke reset_daily, leaving stale
    // daily counters that allow over-budget tasks through on the same day.
    #[test]
    fn reset_monthly_also_resets_daily_state() {
        let mut state = CostDefenseState::default();
        state.daily_spent_usd = 20.0;
        state.tenant_spent_usd.insert("t".into(), 20.0);
        state.task_retry_counts.insert("task-a".into(), 2);
        state.reset_monthly();
        assert!(
            state.daily_spent_usd.abs() < f64::EPSILON,
            "daily_spent must be zero after monthly reset"
        );
        assert!(
            state.tenant_spent_usd.is_empty(),
            "tenant spend must be cleared after monthly reset"
        );
        assert!(
            state.task_retry_counts.is_empty(),
            "retry counts must be cleared after monthly reset"
        );
        assert!(
            state.monthly_spent_usd.abs() < f64::EPSILON,
            "monthly_spent must be zero after monthly reset"
        );
    }

    // Catches: check_before_task returning an empty Vec when *multiple* layers
    // fire simultaneously (e.g., short-circuiting on the first rejection).
    #[test]
    fn multiple_layers_fire_simultaneously_all_reported() {
        let mut cfg = CostDefenseConfig::default();
        cfg.per_task_timeout_secs = 10;
        cfg.daily_budget_usd = 1.0;
        cfg.model_pinning_enabled = true;
        let mut cb = breaker_with(cfg);
        cb.state.daily_spent_usd = 0.99;

        // Exceeds timeout (200 > 10), exceeds daily budget (0.99 + 0.02 > 1.0),
        // unknown model tier.
        let r = cb.check_before_task(200, "t1", "tenant-a", "ultra-secret", 0.02);
        let has_timeout = r
            .iter()
            .any(|x| matches!(x, CostDefenseRejection::TaskTimeout { .. }));
        let has_budget = r
            .iter()
            .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. }));
        let has_model = r
            .iter()
            .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. }));
        assert!(
            has_timeout,
            "timeout rejection missing from multi-layer failure"
        );
        assert!(
            has_budget,
            "budget rejection missing from multi-layer failure"
        );
        assert!(
            has_model,
            "model rejection missing from multi-layer failure"
        );
    }

    // Catches: ScalingPolicy deserialization ignoring unknown fields and crashing
    // instead of using `#[serde(default)]` / `deny_unknown_fields`-free approach.
    #[test]
    fn from_yaml_str_tolerates_extra_unknown_fields() {
        let yaml = "schema_version: 3\nunknown_future_key: 99\nbaseline_id: v3";
        let result = ScalingPolicy::from_yaml_str(yaml);
        assert!(
            result.is_ok(),
            "extra YAML fields must not cause a parse error: {:?}",
            result
        );
    }

    // Catches: Thresholds::default() returning zero for max_file_bytes_hint,
    // which would make every file appear over the streaming threshold.
    #[test]
    fn thresholds_default_max_file_bytes_is_nonzero() {
        let t = Thresholds::default();
        assert!(
            t.max_file_bytes_hint > 0,
            "default max_file_bytes_hint must be > 0"
        );
    }

    // Catches: per-crate overrides being swallowed silently when per_crate_overrides
    // YAML key is present; i.e. the Vec never gets populated.
    #[test]
    fn from_yaml_str_populates_per_crate_overrides() {
        let yaml = "schema_version: 1\nper_crate_overrides:\n  - crate_name: vox-foo\n    allow_blocking_fs_in_async: true\n    notes: legacy IO";
        let p = ScalingPolicy::from_yaml_str(yaml).expect("valid YAML");
        assert_eq!(p.per_crate_overrides.len(), 1);
        assert_eq!(p.per_crate_overrides[0].crate_name, "vox-foo");
        assert!(p.per_crate_overrides[0].allow_blocking_fs_in_async);
    }

    // Catches: has_hard_block returning true on an empty rejection list (e.g. the
    // `any` predicate being negated incorrectly), blocking all clean tasks.
    #[test]
    fn has_hard_block_returns_false_for_empty_rejection_list() {
        let empty: Vec<CostDefenseRejection> = vec![];
        assert!(
            !CostCircuitBreaker::has_hard_block(&empty),
            "empty rejection list must not be classified as a hard block"
        );
    }
}
