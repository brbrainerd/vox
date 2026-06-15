//! 5-layer cost defense circuit breakers for multi-agent mesh economics.
//!
//! Research (Multi-Agent Mesh Economics §5) proves that without hard budget
//! enforcement, agentic loops silently escalate to frontier-tier API costs.
//! This module implements the five mandatory defense layers:
//!
//! 1. **Per-task timeout** — hard wall-clock limit per task (default 300s)
//! 2. **Recovery anti-loops** — max re-attempts per task/day (default 3)
//! 3. **Daily budget kill switch** — centralized aggregate cost ceiling
//! 4. **Model pinning** — prevent silent fallback to expensive frontier models
//! 5. **Monthly pacing** — early-warning at configurable spend percentage

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;

// ── Configuration ────────────────────────────────────────────────────────────

/// Cost defense policy configuration.
///
/// Loaded from `contracts/scaling/policy.yaml` (future) or constructed
/// programmatically. All limits use conservative defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDefenseConfig {
    /// Layer 1: Maximum wall-clock seconds per task before forced termination.
    pub per_task_timeout_secs: u64,
    /// Layer 2: Maximum re-attempts for a single task within a calendar day.
    pub max_retries_per_task_day: u32,
    /// Layer 3: Hard daily budget ceiling in USD. Tasks are rejected once exceeded.
    pub daily_budget_usd: f64,
    /// Layer 4: When true, tasks must declare an explicit model tier; silent
    /// fallback to frontier models is blocked.
    pub model_pinning_enabled: bool,
    /// Layer 5: Percentage of monthly budget at which a warning is emitted.
    /// E.g. `0.80` means warn at 80% spend.
    pub monthly_pacing_warn_pct: f64,
    /// Monthly budget ceiling in USD (Layer 5 denominator).
    pub monthly_budget_usd: f64,
    /// Per-tenant daily budget ceilings in USD. Key = tenant_id.
    pub tenant_daily_caps: HashMap<String, f64>,
    /// Allowed model tier names when `model_pinning_enabled` is true.
    /// Tasks requesting a tier not in this list are rejected.
    pub allowed_model_tiers: Vec<String>,
}

impl Default for CostDefenseConfig {
    fn default() -> Self {
        Self {
            per_task_timeout_secs: 300,
            max_retries_per_task_day: 3,
            daily_budget_usd: 25.0,
            model_pinning_enabled: true,
            monthly_pacing_warn_pct: 0.80,
            monthly_budget_usd: 500.0,
            tenant_daily_caps: HashMap::new(),
            allowed_model_tiers: vec![
                "local".to_string(),
                "mid".to_string(),
                "frontier".to_string(),
            ],
        }
    }
}

// ── Circuit breaker state ────────────────────────────────────────────────────

/// Reason a task was rejected by the cost defense layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CostDefenseRejection {
    /// Layer 1: task would exceed the per-task timeout.
    TaskTimeout { limit_secs: u64 },
    /// Layer 2: task has been retried too many times today.
    RetryLimitExceeded {
        task_id: String,
        attempts: u32,
        limit: u32,
    },
    /// Layer 3: daily budget is exhausted.
    DailyBudgetExhausted { spent_usd: f64, limit_usd: f64 },
    /// Layer 4: requested model tier is not pinned/allowed.
    ModelNotPinned { requested_tier: String },
    /// Layer 5: monthly pacing threshold breached (warning, not hard block).
    MonthlyPacingWarning { spent_usd: f64, warn_at_usd: f64 },
    /// Layer 6: Tenant-specific daily budget is exhausted.
    TenantBudgetExhausted {
        tenant_id: String,
        spent_usd: f64,
        limit_usd: f64,
    },
}

/// Mutable state tracked by the circuit breaker across tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostDefenseState {
    /// Cumulative USD spent today (resets at midnight or on explicit reset).
    pub daily_spent_usd: f64,
    /// Cumulative USD spent this month.
    pub monthly_spent_usd: f64,
    /// Per-tenant USD spent today. Key = tenant_id.
    pub tenant_spent_usd: HashMap<String, f64>,
    /// Per-task retry counters for the current day. Key = task_id.
    pub task_retry_counts: HashMap<String, u32>,
}

impl CostDefenseState {
    /// Record a completed task's cost.
    pub fn record_cost(&mut self, tenant_id: &str, cost_usd: f64) {
        self.daily_spent_usd += cost_usd;
        self.monthly_spent_usd += cost_usd;
        *self
            .tenant_spent_usd
            .entry(tenant_id.to_string())
            .or_insert(0.0) += cost_usd;
    }

    /// Record a retry attempt for a task.
    pub fn record_retry(&mut self, task_id: &str) {
        *self
            .task_retry_counts
            .entry(task_id.to_string())
            .or_insert(0) += 1;
    }

    /// Reset daily counters (e.g. at midnight).
    pub fn reset_daily(&mut self) {
        self.daily_spent_usd = 0.0;
        self.tenant_spent_usd.clear();
        self.task_retry_counts.clear();
    }

    /// Reset monthly counters (e.g. at month boundary).
    pub fn reset_monthly(&mut self) {
        self.monthly_spent_usd = 0.0;
        self.reset_daily();
    }
}

// ── Circuit breaker checks ───────────────────────────────────────────────────

/// 5-layer cost defense circuit breaker.
///
/// Call [`Self::check_before_task`] before dispatching any task. The method returns
/// a list of rejections (empty = task is allowed). Callers should treat any
/// non-empty rejection list as a hard block except for `MonthlyPacingWarning`
/// which is advisory.
pub struct CostCircuitBreaker {
    pub config: CostDefenseConfig,
    pub state: CostDefenseState,
}

impl CostCircuitBreaker {
    pub fn new(config: CostDefenseConfig) -> Self {
        Self {
            config,
            state: CostDefenseState::default(),
        }
    }

    /// Run all 5 layers of cost defense checks before dispatching a task.
    ///
    /// `estimated_duration_secs`: caller's estimate of task wall-clock time.
    /// `task_id`: unique identifier for retry tracking.
    /// `requested_model_tier`: the model tier the task wants to use.
    /// `estimated_cost_usd`: estimated incremental cost for this task.
    pub fn check_before_task(
        &self,
        estimated_duration_secs: u64,
        task_id: &str,
        tenant_id: &str,
        requested_model_tier: &str,
        estimated_cost_usd: f64,
    ) -> Vec<CostDefenseRejection> {
        let mut rejections = Vec::new();

        // Layer 1: Per-task timeout
        if estimated_duration_secs > self.config.per_task_timeout_secs {
            rejections.push(CostDefenseRejection::TaskTimeout {
                limit_secs: self.config.per_task_timeout_secs,
            });
        }

        // Layer 2: Recovery anti-loops
        let attempts = self
            .state
            .task_retry_counts
            .get(task_id)
            .copied()
            .unwrap_or(0);
        if attempts >= self.config.max_retries_per_task_day {
            rejections.push(CostDefenseRejection::RetryLimitExceeded {
                task_id: task_id.to_string(),
                attempts,
                limit: self.config.max_retries_per_task_day,
            });
        }

        // Layer 3: Daily budget kill switch
        let projected_daily = self.state.daily_spent_usd + estimated_cost_usd;
        if projected_daily > self.config.daily_budget_usd {
            rejections.push(CostDefenseRejection::DailyBudgetExhausted {
                spent_usd: self.state.daily_spent_usd,
                limit_usd: self.config.daily_budget_usd,
            });
        }

        // Layer 4: Model pinning
        if self.config.model_pinning_enabled {
            let tier_lower = requested_model_tier.to_ascii_lowercase();
            let allowed = self
                .config
                .allowed_model_tiers
                .iter()
                .any(|t| t.to_ascii_lowercase() == tier_lower);
            if !allowed {
                rejections.push(CostDefenseRejection::ModelNotPinned {
                    requested_tier: requested_model_tier.to_string(),
                });
            }
        }

        // Layer 5: Monthly pacing warning
        let warn_threshold = self.config.monthly_budget_usd * self.config.monthly_pacing_warn_pct;
        let projected_monthly = self.state.monthly_spent_usd + estimated_cost_usd;
        if projected_monthly > warn_threshold {
            warn!(
                monthly_spent = self.state.monthly_spent_usd,
                projected = projected_monthly,
                threshold = warn_threshold,
                "cost defense: monthly pacing warning"
            );
            rejections.push(CostDefenseRejection::MonthlyPacingWarning {
                spent_usd: self.state.monthly_spent_usd,
                warn_at_usd: warn_threshold,
            });
        }

        // Layer 6: Tenant budget isolation
        if let Some(limit_usd) = self.config.tenant_daily_caps.get(tenant_id) {
            let spent = self
                .state
                .tenant_spent_usd
                .get(tenant_id)
                .copied()
                .unwrap_or(0.0);
            if spent + estimated_cost_usd > *limit_usd {
                rejections.push(CostDefenseRejection::TenantBudgetExhausted {
                    tenant_id: tenant_id.to_string(),
                    spent_usd: spent,
                    limit_usd: *limit_usd,
                });
            }
        }

        rejections
    }

    /// Returns true if any rejection is a hard block (not just a warning).
    pub fn has_hard_block(rejections: &[CostDefenseRejection]) -> bool {
        rejections
            .iter()
            .any(|r| !matches!(r, CostDefenseRejection::MonthlyPacingWarning { .. }))
    }

    /// Record a completed task's cost and update state.
    pub fn record_task_completion(&mut self, task_id: &str, tenant_id: &str, actual_cost_usd: f64) {
        self.state.record_cost(tenant_id, actual_cost_usd);
        self.state.record_retry(task_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_breaker() -> CostCircuitBreaker {
        CostCircuitBreaker::new(CostDefenseConfig::default())
    }

    #[test]
    fn layer1_timeout_blocks_long_tasks() {
        let cb = default_breaker();
        let r = cb.check_before_task(600, "t1", "tenant-1", "local", 0.01);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::TaskTimeout { .. }))
        );
    }

    #[test]
    fn layer1_timeout_allows_short_tasks() {
        let cb = default_breaker();
        let r = cb.check_before_task(60, "t1", "tenant-1", "local", 0.01);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::TaskTimeout { .. }))
        );
    }

    #[test]
    fn layer2_retry_limit() {
        let mut cb = default_breaker();
        for _ in 0..3 {
            cb.state.record_retry("t1");
        }
        let r = cb.check_before_task(10, "t1", "tenant-1", "local", 0.01);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::RetryLimitExceeded { .. }))
        );
    }

    #[test]
    fn layer3_daily_budget_kill_switch() {
        let mut cb = default_breaker();
        cb.state.daily_spent_usd = 24.0;
        let r = cb.check_before_task(10, "t1", "tenant-1", "local", 2.0);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. }))
        );
    }

    #[test]
    fn layer3_daily_budget_allows_within_limit() {
        let mut cb = default_breaker();
        cb.state.daily_spent_usd = 10.0;
        let r = cb.check_before_task(10, "t1", "tenant-1", "local", 5.0);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::DailyBudgetExhausted { .. }))
        );
    }

    #[test]
    fn layer4_model_pinning_blocks_unknown_tier() {
        let cb = default_breaker();
        let r = cb.check_before_task(10, "t1", "tenant-1", "super-premium", 0.01);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. }))
        );
    }

    #[test]
    fn layer4_model_pinning_allows_known_tier() {
        let cb = default_breaker();
        let r = cb.check_before_task(10, "t1", "tenant-1", "mid", 0.01);
        assert!(
            !r.iter()
                .any(|x| matches!(x, CostDefenseRejection::ModelNotPinned { .. }))
        );
    }

    #[test]
    fn layer5_monthly_pacing_warning() {
        let mut cb = default_breaker();
        cb.state.monthly_spent_usd = 450.0; // > 80% of 500
        let r = cb.check_before_task(10, "t1", "tenant-1", "local", 1.0);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::MonthlyPacingWarning { .. }))
        );
    }

    #[test]
    fn monthly_pacing_is_not_hard_block() {
        let r = vec![CostDefenseRejection::MonthlyPacingWarning {
            spent_usd: 450.0,
            warn_at_usd: 400.0,
        }];
        assert!(!CostCircuitBreaker::has_hard_block(&r));
    }

    #[test]
    fn daily_budget_is_hard_block() {
        let r = vec![CostDefenseRejection::DailyBudgetExhausted {
            spent_usd: 25.0,
            limit_usd: 25.0,
        }];
        assert!(CostCircuitBreaker::has_hard_block(&r));
    }

    #[test]
    fn record_task_completion_updates_state() {
        let mut cb = default_breaker();
        cb.record_task_completion("t1", "tenant-1", 1.50);
        assert!((cb.state.daily_spent_usd - 1.5).abs() < f64::EPSILON);
        assert!((cb.state.monthly_spent_usd - 1.5).abs() < f64::EPSILON);
        assert_eq!(cb.state.tenant_spent_usd.get("tenant-1"), Some(&1.5));
        assert_eq!(cb.state.task_retry_counts.get("t1"), Some(&1));
    }

    #[test]
    fn reset_daily_clears_counters() {
        let mut cb = default_breaker();
        cb.state.daily_spent_usd = 10.0;
        cb.state.record_retry("t1");
        cb.state.reset_daily();
        assert!((cb.state.daily_spent_usd - 0.0).abs() < f64::EPSILON);
        assert!(cb.state.task_retry_counts.is_empty());
    }

    #[test]
    fn config_defaults_reasonable() {
        let cfg = CostDefenseConfig::default();
        assert_eq!(cfg.per_task_timeout_secs, 300);
        assert_eq!(cfg.max_retries_per_task_day, 3);
        assert!((cfg.daily_budget_usd - 25.0).abs() < f64::EPSILON);
        assert!(cfg.model_pinning_enabled);
        assert!((cfg.monthly_pacing_warn_pct - 0.80).abs() < f64::EPSILON);
        assert_eq!(cfg.allowed_model_tiers.len(), 3);
    }

    #[test]
    fn clean_task_no_rejections() {
        let cb = default_breaker();
        let r = cb.check_before_task(60, "t1", "tenant-1", "local", 0.50);
        assert!(r.is_empty(), "clean task should pass all layers: {:?}", r);
    }

    #[test]
    fn layer6_tenant_budget_enforced() {
        let mut cfg = CostDefenseConfig::default();
        cfg.tenant_daily_caps
            .insert("expensive-tenant".into(), 10.0);
        let mut cb = CostCircuitBreaker::new(cfg);
        cb.state.record_cost("expensive-tenant", 9.0);

        let r = cb.check_before_task(10, "t1", "expensive-tenant", "local", 2.0);
        assert!(
            r.iter()
                .any(|x| matches!(x, CostDefenseRejection::TenantBudgetExhausted { .. }))
        );
    }
}
