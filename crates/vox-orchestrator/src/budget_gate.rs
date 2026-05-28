//! Token / cost budget gate for autonomous session management (D7).
//!
//! Tracks budget fraction consumed and gates model selection:
//! ≥ 0.80 → Downgrade (switch to cheaper model); ≥ 0.95 → Halt.
//! The `is_exhausted()` signal feeds into [`crate::tier_cascade::CompositeSignal`].
//! All logic is pure: no async, no I/O.

use serde::{Deserialize, Serialize};

/// Current budget status for tokens and/or cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BudgetStatus {
    /// Under the downgrade threshold — proceed normally.
    Ok,
    /// Fraction ≥ downgrade threshold — switch to cheaper model.
    Downgrade,
    /// Fraction ≥ halt threshold — stop issuing new requests.
    Halt,
}

impl std::fmt::Display for BudgetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Downgrade => write!(f, "downgrade"),
            Self::Halt => write!(f, "halt"),
        }
    }
}

/// Gate decision including which dimension triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDecision {
    pub status: BudgetStatus,
    /// Fraction that caused the status (token fraction, cost fraction, or 0.0 if Ok).
    pub triggering_fraction: f64,
}

impl BudgetDecision {
    /// Returns true when the budget is exhausted (Halt status).
    #[must_use]
    #[inline]
    pub fn is_exhausted(&self) -> bool {
        self.status == BudgetStatus::Halt
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantTierLimit {
    pub tier: String,
    pub max_tokens_per_month: i64,
    pub max_concurrent_sessions: u32,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub fallback_tier: Option<String>,
}

/// Thresholds loaded from contract YAML. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetGateConfig {
    /// Fraction ≥ this → Downgrade.
    pub downgrade_fraction: f64,
    /// Fraction ≥ this → Halt.
    pub halt_fraction: f64,
    #[serde(default)]
    pub global_limits: Vec<TenantTierLimit>,
}

impl Default for BudgetGateConfig {
    fn default() -> Self {
        Self {
            downgrade_fraction: 0.80,
            halt_fraction: 0.95,
            global_limits: vec![
                TenantTierLimit {
                    tier: "free".to_string(),
                    max_tokens_per_month: 1000000,
                    max_concurrent_sessions: 2,
                    action: Some("degrade_tier".to_string()),
                    fallback_tier: Some("light".to_string()),
                },
                TenantTierLimit {
                    tier: "pro".to_string(),
                    max_tokens_per_month: 10000000,
                    max_concurrent_sessions: 10,
                    action: Some("block".to_string()),
                    fallback_tier: None,
                },
                TenantTierLimit {
                    tier: "enterprise".to_string(),
                    max_tokens_per_month: -1,
                    max_concurrent_sessions: 100,
                    action: None,
                    fallback_tier: None,
                },
            ],
        }
    }
}

/// Pure budget gate.
///
/// Note: this crate already has a [`crate::gate::BudgetGate`] which is a task-level
/// gate trait. This struct is the *orchestrator policy* budget gate (D7) and lives
/// in `budget_gate.rs` to avoid naming conflicts.
pub struct OrchestratorBudgetGate {
    config: BudgetGateConfig,
}

impl OrchestratorBudgetGate {
    pub fn new(config: BudgetGateConfig) -> Self {
        Self { config }
    }

    /// Evaluate a single budget fraction (tokens used / budget, or cost used / budget).
    #[must_use]
    #[inline]
    pub fn evaluate_fraction(&self, fraction: f64) -> BudgetStatus {
        let f = fraction.clamp(0.0, 1.0);
        if f >= self.config.halt_fraction {
            BudgetStatus::Halt
        } else if f >= self.config.downgrade_fraction {
            BudgetStatus::Downgrade
        } else {
            BudgetStatus::Ok
        }
    }

    /// Evaluate both token and cost fractions; worst status wins.
    /// Inputs are clamped to `[0, 1]` and the triggering fraction reported back is the
    /// clamped value, so a returned `BudgetDecision` is never out-of-range.
    /// When the resulting status is `Ok`, `triggering_fraction` is reset to `0.0`
    /// per the field's documented contract.
    #[must_use]
    pub fn evaluate(&self, token_fraction: f64, cost_fraction: f64) -> BudgetDecision {
        let token = token_fraction.clamp(0.0, 1.0);
        let cost = cost_fraction.clamp(0.0, 1.0);
        let ts = self.evaluate_fraction(token);
        let cs = self.evaluate_fraction(cost);
        let (status, trigger) = if ts > cs {
            (ts, token)
        } else if cs > ts {
            (cs, cost)
        } else {
            (ts, token.max(cost))
        };
        BudgetDecision {
            status,
            triggering_fraction: if status == BudgetStatus::Ok {
                0.0
            } else {
                trigger
            },
        }
    }

    /// Evaluate tenant monthly token budget.
    pub fn check_tenant_monthly_budget(
        &self,
        tier: &str,
        current_tokens: i64,
        estimated_task_tokens: i64,
    ) -> Result<(), String> {
        let limit = self.config.global_limits.iter().find(|l| l.tier == tier);
        if let Some(limit) = limit {
            if limit.max_tokens_per_month == -1 {
                return Ok(());
            }
            if current_tokens + estimated_task_tokens > limit.max_tokens_per_month {
                return Err(format!(
                    "Tenant monthly token budget exceeded for tier '{}': {} used (limit: {})",
                    tier,
                    current_tokens + estimated_task_tokens,
                    limit.max_tokens_per_month
                ));
            }
        }
        Ok(())
    }
}

/// Metric payload emitted when budget status changes.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetDecisionEvent {
    pub metric_type: &'static str,
    pub status: String,
    pub triggering_fraction: f64,
    pub session_id: Option<String>,
}

impl BudgetDecisionEvent {
    pub fn new(decision: &BudgetDecision, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_BUDGET_DECISION,
            status: decision.status.to_string(),
            triggering_fraction: decision.triggering_fraction,
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> OrchestratorBudgetGate {
        OrchestratorBudgetGate::new(BudgetGateConfig::default())
    }

    #[test]
    fn low_fraction_is_ok() {
        let g = gate();
        assert_eq!(g.evaluate_fraction(0.50), BudgetStatus::Ok);
    }

    #[test]
    fn at_downgrade_threshold_downgrades() {
        let g = gate();
        assert_eq!(g.evaluate_fraction(0.80), BudgetStatus::Downgrade);
    }

    #[test]
    fn above_downgrade_but_below_halt_downgrades() {
        let g = gate();
        assert_eq!(g.evaluate_fraction(0.90), BudgetStatus::Downgrade);
    }

    #[test]
    fn at_halt_threshold_halts() {
        let g = gate();
        assert_eq!(g.evaluate_fraction(0.95), BudgetStatus::Halt);
    }

    #[test]
    fn above_halt_threshold_halts() {
        let g = gate();
        assert_eq!(g.evaluate_fraction(1.0), BudgetStatus::Halt);
    }

    #[test]
    fn worst_of_token_and_cost_wins() {
        let g = gate();
        // cost fraction in halt zone, token fraction OK
        let d = g.evaluate(0.50, 0.96);
        assert_eq!(d.status, BudgetStatus::Halt);
    }

    #[test]
    fn is_exhausted_true_only_on_halt() {
        let g = gate();
        assert!(!g.evaluate(0.85, 0.0).is_exhausted());
        assert!(g.evaluate(0.96, 0.0).is_exhausted());
    }

    #[test]
    fn budget_decision_event_has_correct_metric_type() {
        let d = BudgetDecision {
            status: BudgetStatus::Ok,
            triggering_fraction: 0.0,
        };
        let ev = BudgetDecisionEvent::new(&d, None);
        assert_eq!(ev.metric_type, "orch.budget.decision");
    }
}
