//! Parity Gates for usage tracking and rate limiting.
//!
//! Inspired by Greater Fool's "Gates" system, this module provides
//! middleware to intercept AI requests and enforce budgets/limits.

use crate::attention::AttentionBudget;
use crate::budget::BudgetManager;
use crate::types::AgentId;
use crate::usage::{DEFAULT_RATE_LIMIT_RETRY_SECS, LlmUsageKey, UsageTracker};
use async_trait::async_trait;
#[cfg(not(test))]
use tracing::info;

/// A localized inter-agent lock strictly for managing OS disk contention
/// when spinning up heavy Node.js or Cargo tests which rely on singular
/// target/ or node_modules/ caches. Populi node execution will queue here.
#[cfg(not(test))]
static BEHAVIORAL_TEST_LOCK: tokio::sync::OnceCell<tokio::sync::Mutex<()>> =
    tokio::sync::OnceCell::const_new();

/// A gate that can allow or deny an AI request.
#[async_trait]
pub trait Gate: Send + Sync {
    /// Check if the request is allowed.
    async fn allow(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        estimated_tokens: u64,
    ) -> GateResult;

    /// Record the actual usage after a successful request.
    async fn record_usage(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
    );
}

/// Result of a gate check.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GateResult {
    /// Request allowed.
    Allowed,
    /// Request denied due to budget exhaustion.
    BudgetExceeded {
        /// Human-readable explanation shown to the caller.
        message: String,
    },
    /// Request denied due to rate limiting.
    RateLimited {
        /// Hint for when to retry; `None` if the backend did not specify.
        retry_after_secs: Option<u64>,
    },
    /// Request denied because the pilot's attention budget is exhausted (Phase 15).
    AttentionExhausted {
        /// Human-readable explanation.
        message: String,
        /// Milliseconds of attention consumed this session.
        spent_ms: u64,
        /// Configured maximum for this session.
        max_ms: u64,
    },
    /// Request denied because behavioral tests failed (OAPV phase).
    BehavioralTestFailed { message: String },
    /// Request denied because the agent appears to be in a doom-loop (cost-without-progress).
    DoomLoop {
        /// Human-readable explanation shown to the caller.
        message: String,
    },
}

/// A gate enforcing behavioral tests (e.g. `cargo test`).
pub struct BehavioralGate {
    require_tests: bool,
}

impl BehavioralGate {
    pub fn new(require_tests: bool) -> Self {
        Self { require_tests }
    }

    /// Evaluates `cargo test` for passing.
    #[cfg_attr(test, allow(unused_variables))]
    pub async fn check_behavior(&self, module_path: Option<&str>) -> GateResult {
        if !self.require_tests {
            return GateResult::Allowed;
        }
        // `orch_smoke` and similar unit tests complete tasks through the real completion path,
        // which would otherwise spawn a nested workspace `cargo test` here. That re-enters the
        // same crate, fights for the target directory lock, and often loses MSVC/nvcc env — so
        // skip only while this crate is built for its own `cargo test` (`cfg(test)` on the lib).
        #[cfg(test)]
        {
            GateResult::Allowed
        }

        #[cfg(not(test))]
        {
            let mut is_js = false;
            if let Ok(cwd) = std::env::current_dir() {
                if cwd.join("package.json").exists() {
                    is_js = true;
                }
            }

            let mut cmd = if is_js {
                let mut c =
                    tokio::process::Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
                c.arg("test");
                c
            } else {
                let mut c = tokio::process::Command::new("cargo");
                c.arg("test");
                if let Some(p) = module_path {
                    c.arg(p);
                }
                c.arg("--color=never").arg("--message-format=json");
                c
            };

            // Acquire the static inter-agent lock to ensure Cargo/npm OS caches don't collide
            info!(
                "BehavioralGate: Agent {} requesting OS test lock...",
                "agent"
            );
            let mtx = BEHAVIORAL_TEST_LOCK
                .get_or_init(|| async { tokio::sync::Mutex::new(()) })
                .await;
            let _lock = mtx.lock().await;
            info!("BehavioralGate: Lock acquired. Executing tests...");

            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    GateResult::Allowed
                } else {
                    let msg = String::from_utf8_lossy(&output.stderr);
                    GateResult::BehavioralTestFailed {
                        message: format!(
                            "Behavioral Gate Failed: Process tests failed to pass.\n{}",
                            msg
                        ),
                    }
                }
            } else {
                GateResult::BehavioralTestFailed {
                    message: "Behavioral Gate Failed: Failed to execute test runner.".to_string(),
                }
            }
        }
    }
}

/// A gate that enforces budgets via the `BudgetManager`.
pub struct BudgetGate<'a> {
    budget_manager: &'a BudgetManager,
    usage_tracker: &'a UsageTracker<'a>,
    orchestrator_config: &'a crate::config::OrchestratorConfig,
}

impl<'a> BudgetGate<'a> {
    /// Wires budget caps together with persisted usage counters from Codex.
    pub fn new(
        budget_manager: &'a BudgetManager,
        usage_tracker: &'a UsageTracker<'a>,
        orchestrator_config: &'a crate::config::OrchestratorConfig,
    ) -> Self {
        Self {
            budget_manager,
            usage_tracker,
            orchestrator_config,
        }
    }

    /// Static check for in-memory token/cost budget. Stateless; no DB access.
    pub fn check(
        manager: &BudgetManager,
        agent_id: AgentId,
        _config: &crate::config::OrchestratorConfig,
    ) -> GateResult {
        if let Some(budget) = manager.check_budget(agent_id) {
            if budget.cost_exceeded() {
                return GateResult::BudgetExceeded {
                    message: format!(
                        "Cost budget exceeded: ${:.4} used (cap: ${:.4})",
                        budget.cost_usd,
                        budget
                            .allocation
                            .as_ref()
                            .map(|a| a.max_cost_usd)
                            .unwrap_or(0.0)
                    ),
                };
            }
            if budget.tokens_available() == 0 {
                let cap = budget.effective_max_tokens();
                return GateResult::BudgetExceeded {
                    message: format!(
                        "Token budget exceeded: {} of {} tokens used",
                        budget.tokens_used, cap
                    ),
                };
            }
        }
        GateResult::Allowed
    }

    /// Check pilot attention from a cloned [`AttentionBudget`] snapshot (safe before `.await`).
    #[must_use]
    pub fn check_attention_snapshot(
        snap: &AttentionBudget,
        config: &crate::config::OrchestratorConfig,
    ) -> GateResult {
        if !config.attention_enabled {
            return GateResult::Allowed;
        }
        if snap.exhausted() {
            GateResult::AttentionExhausted {
                message: format!(
                    "Attention budget exhausted: {}ms of {}ms used this session. \
                     Consider a break before continuing.",
                    snap.spent_ms, snap.max_attention_ms
                ),
                spent_ms: snap.spent_ms,
                max_ms: snap.max_attention_ms,
            }
        } else {
            GateResult::Allowed
        }
    }

    pub fn check_attention(
        manager: &BudgetManager,
        config: &crate::config::OrchestratorConfig,
    ) -> GateResult {
        let snap = manager.attention_snapshot();
        Self::check_attention_snapshot(&snap, config)
    }

    /// Check whether an agent is stuck in a doom-loop (spending cost without making progress).
    #[must_use]
    pub fn check_doom_loop(manager: &BudgetManager, agent_id: AgentId) -> GateResult {
        if let Some(reason) = manager.doom_loop_cost_check(agent_id) {
            GateResult::DoomLoop { message: reason }
        } else {
            GateResult::Allowed
        }
    }

    /// Check whether the pilot's attention budget can sustain an interruption right now
    /// based on the `InterruptionSignals`.
    #[must_use]
    pub fn can_interrupt(
        manager: &BudgetManager,
        config: &crate::config::OrchestratorConfig,
        signals: &crate::attention::InterruptionSignals,
    ) -> crate::attention::InterruptionDecision {
        crate::attention::evaluate_interruption(
            signals,
            &manager.attention_snapshot(),
            config.attention_enabled,
            config.attention_alert_threshold,
        )
    }

    /// Record usage with provider reconciliation metadata.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage_detailed(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        provider_request_id: Option<&str>,
        provider_reported_cost_usd: Option<f64>,
        estimated_cost_usd: Option<f64>,
        reconciled_cost_usd: Option<f64>,
        cost_source: Option<&str>,
        task_category: Option<&str>,
    ) {
        self.budget_manager
            .record_usage(agent_id, (tokens_in + tokens_out) as usize);
        self.budget_manager.record_cost(agent_id, cost_usd, None);
        let _ = self
            .usage_tracker
            .record_call_detailed(
                &usage.provider,
                &usage.model,
                tokens_in,
                tokens_out,
                cost_usd,
                provider_request_id,
                provider_reported_cost_usd,
                estimated_cost_usd,
                reconciled_cost_usd,
                cost_source,
                task_category,
                Some(&agent_id.to_string()),
            )
            .await;
    }

    /// Like [`Gate::allow`], but pilot attention is read from `pilot_attention` when provided
    /// (e.g. embedded orchestrator ledger in MCP). When `None`, falls back to [`BudgetManager::attention_snapshot`]
    /// on this gate's token/cost manager.
    pub async fn allow_with_pilot_attention(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        pilot_attention: Option<AttentionBudget>,
        _estimated_tokens: u64,
    ) -> GateResult {
        let result = BudgetGate::check(self.budget_manager, agent_id, self.orchestrator_config);
        if result != GateResult::Allowed {
            return result;
        }

        let snap = pilot_attention.unwrap_or_else(|| self.budget_manager.attention_snapshot());
        let att = BudgetGate::check_attention_snapshot(&snap, self.orchestrator_config);
        if att != GateResult::Allowed {
            return att;
        }

        let budgets = match self.usage_tracker.remaining_all().await {
            Ok(b) => b,
            Err(_) => return GateResult::Allowed,
        };

        if let Some(b) = budgets.iter().find(|b| {
            b.provider == usage.provider
                && (b.model == usage.model
                    || b.model == "*"
                    || (b.model == ":free" && usage.model == ":free"))
        }) {
            if b.rate_limited {
                return GateResult::RateLimited {
                    retry_after_secs: Some(DEFAULT_RATE_LIMIT_RETRY_SECS),
                };
            }
            if b.remaining == 0 {
                return GateResult::BudgetExceeded {
                    message: format!("Provider {}/{} daily limit reached", b.provider, b.model),
                };
            }
        }

        GateResult::Allowed
    }
}

#[async_trait]
impl<'a> Gate for BudgetGate<'a> {
    async fn allow(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        _estimated_tokens: u64,
    ) -> GateResult {
        self.allow_with_pilot_attention(agent_id, usage, None, _estimated_tokens)
            .await
    }

    async fn record_usage(
        &self,
        agent_id: AgentId,
        usage: &LlmUsageKey,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
    ) {
        // Record in memory (budget manager)
        self.budget_manager
            .record_usage(agent_id, (tokens_in + tokens_out) as usize);
        self.budget_manager.record_cost(agent_id, cost_usd, None);

        // Record in DB (usage tracker) using the same keys as `LIMITS`.
        let _ = self
            .usage_tracker
            .record_call_detailed(
                &usage.provider,
                &usage.model,
                tokens_in,
                tokens_out,
                cost_usd,
                None,
                None,
                Some(cost_usd),
                Some(cost_usd),
                Some("estimated"),
                None,
                Some(&agent_id.to_string()),
            )
            .await;
    }
}

#[cfg(test)]
mod budget_gate_tests {
    use super::*;
    use crate::budget::BudgetManager;
    use crate::config::OrchestratorConfig;

    #[test]
    fn check_attention_snapshot_blocks_when_enabled_and_exhausted() {
        let cfg = OrchestratorConfig {
            attention_enabled: true,
            ..OrchestratorConfig::default()
        };
        let mgr = BudgetManager::new(None);
        mgr.init_attention(500);
        mgr.add_questioning_attention_debit_ms(500);
        let snap = mgr.attention_snapshot();
        assert!(matches!(
            BudgetGate::check_attention_snapshot(&snap, &cfg),
            GateResult::AttentionExhausted { .. }
        ));
    }

    #[test]
    fn check_doom_loop_returns_doom_loop_when_threshold_exceeded() {
        use crate::budget::BudgetManager;
        use crate::types::AgentId;
        let mgr = BudgetManager::new(None);
        let agent = AgentId(1);
        // Lower the threshold so a small spend triggers doom-loop detection.
        mgr.set_doom_loop_cost_threshold(0.10);
        mgr.record_cost_progress(agent, 0.50);
        let result = BudgetGate::check_doom_loop(&mgr, agent);
        assert!(
            matches!(result, GateResult::DoomLoop { .. }),
            "expected DoomLoop, got {result:?}"
        );
    }

    #[test]
    fn check_doom_loop_allows_after_task_completion() {
        use crate::budget::BudgetManager;
        use crate::types::AgentId;
        let mgr = BudgetManager::new(None);
        let agent = AgentId(2);
        mgr.set_doom_loop_cost_threshold(0.10);
        mgr.record_cost_progress(agent, 0.50);
        assert!(
            matches!(
                BudgetGate::check_doom_loop(&mgr, agent),
                GateResult::DoomLoop { .. }
            ),
            "expected DoomLoop before completion"
        );
        mgr.record_task_completion(agent);
        assert_eq!(
            BudgetGate::check_doom_loop(&mgr, agent),
            GateResult::Allowed,
            "expected Allowed after task completion"
        );
    }

    #[test]
    fn check_attention_snapshot_allows_when_disabled_even_if_spent_high() {
        let cfg = OrchestratorConfig {
            attention_enabled: false,
            ..OrchestratorConfig::default()
        };
        let mgr = BudgetManager::new(None);
        mgr.init_attention(100);
        mgr.add_questioning_attention_debit_ms(500);
        let snap = mgr.attention_snapshot();
        assert_eq!(
            BudgetGate::check_attention_snapshot(&snap, &cfg),
            GateResult::Allowed
        );
    }
}
