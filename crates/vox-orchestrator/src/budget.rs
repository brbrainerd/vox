use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::types::AgentId;

/// Per-agent budget allocation cap.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentBudgetAllocation {
    /// Maximum tokens allowed per period
    pub max_tokens: usize,
    /// Maximum cost in USD allowed per period
    pub max_cost_usd: f64,
    /// Alert when token usage exceeds this fraction (0.0–1.0)
    pub token_alert_threshold: f64,
    /// Alert when cost exceeds this fraction (0.0–1.0)
    pub cost_alert_threshold: f64,
    /// Rollover unused budget to the next period (as fraction 0.0–1.0)
    pub rollover_fraction: f64,
}

impl AgentBudgetAllocation {
    pub fn new(max_tokens: usize, max_cost_usd: f64) -> Self {
        Self {
            max_tokens,
            max_cost_usd,
            token_alert_threshold: 0.8,
            cost_alert_threshold: 0.9,
            rollover_fraction: 0.0,
        }
    }

    pub fn with_rollover(mut self, fraction: f64) -> Self {
        self.rollover_fraction = fraction.clamp(0.0, 1.0);
        self
    }

    pub fn with_alert_thresholds(mut self, token: f64, cost: f64) -> Self {
        self.token_alert_threshold = token.clamp(0.0, 1.0);
        self.cost_alert_threshold = cost.clamp(0.0, 1.0);
        self
    }
}

/// Configuration for an agent's context budget.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextBudget {
    pub agent_id: AgentId,
    pub model_max_tokens: usize,
    pub tokens_used: usize,
    /// Cumulative cost in USD incurred by this agent.
    pub cost_usd: f64,
    /// Optional per-agent hard cap (None = use model_max_tokens as limit)
    pub allocation: Option<AgentBudgetAllocation>,
    /// Rolled-over bonus tokens from the previous period
    pub rollover_tokens: usize,
}

impl ContextBudget {
    pub fn new(agent_id: AgentId, max_tokens: usize) -> Self {
        Self {
            agent_id,
            model_max_tokens: max_tokens,
            tokens_used: 0,
            cost_usd: 0.0,
            allocation: None,
            rollover_tokens: 0,
        }
    }

    pub fn effective_max_tokens(&self) -> usize {
        let base = self
            .allocation
            .as_ref()
            .map(|a| a.max_tokens)
            .unwrap_or(self.model_max_tokens);
        base.saturating_add(self.rollover_tokens)
    }

    pub fn tokens_available(&self) -> usize {
        self.effective_max_tokens().saturating_sub(self.tokens_used)
    }

    pub fn should_summarize(&self) -> bool {
        let threshold = self
            .allocation
            .as_ref()
            .map(|a| a.token_alert_threshold)
            .unwrap_or(0.8);
        self.tokens_used as f64 > (self.effective_max_tokens() as f64 * threshold)
    }

    /// True if token usage exceeds the alert threshold.
    pub fn token_alert(&self) -> bool {
        self.should_summarize()
    }

    /// True if cost exceeds the cost alert threshold.
    pub fn cost_alert(&self) -> bool {
        if let Some(ref alloc) = self.allocation {
            self.cost_usd > alloc.max_cost_usd * alloc.cost_alert_threshold
        } else {
            false
        }
    }

    /// True if cost has exceeded the allocation hard cap.
    pub fn cost_exceeded(&self) -> bool {
        if let Some(ref alloc) = self.allocation {
            self.cost_usd >= alloc.max_cost_usd
        } else {
            false
        }
    }

    /// Roll over unused budget for the next period.
    /// Returns the number of rollover tokens granted.
    pub fn rollover(&mut self) -> usize {
        let unused = self.tokens_available();
        let rollover = if let Some(ref alloc) = self.allocation {
            (unused as f64 * alloc.rollover_fraction).floor() as usize
        } else {
            0
        };
        // Reset usage for new period
        self.tokens_used = 0;
        self.rollover_tokens = rollover;
        rollover
    }
}

/// Tracks agent context budgets globally.
#[derive(Debug, Clone, Default)]
pub struct BudgetManager {
    inner: Arc<RwLock<HashMap<AgentId, ContextBudget>>>,
}

impl BudgetManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register or reset an agent's budget.
    pub fn reset(&self, agent_id: AgentId, max_tokens: usize) {
        let mut map = self.inner.write().expect("budget lock poisoned");
        map.insert(agent_id, ContextBudget::new(agent_id, max_tokens));
    }

    /// Set a per-agent allocation cap (overrides default limits).
    pub fn set_allocation(&self, agent_id: AgentId, allocation: AgentBudgetAllocation) {
        let mut map = self.inner.write().expect("budget lock poisoned");
        let budget = map
            .entry(agent_id)
            .or_insert_with(|| ContextBudget::new(agent_id, allocation.max_tokens));
        budget.allocation = Some(allocation);
    }

    /// Record token usage for an agent.
    pub fn record_usage(&self, agent_id: AgentId, tokens: usize) {
        let mut map = self.inner.write().expect("budget lock poisoned");
        if let Some(budget) = map.get_mut(&agent_id) {
            budget.tokens_used = budget.tokens_used.saturating_add(tokens);
        } else {
            let mut budget = ContextBudget::new(agent_id, 100_000);
            budget.tokens_used = tokens;
            map.insert(agent_id, budget);
        }
    }

    /// Record cost in USD for an agent (e.g., from an OpenRouter API call).
    pub fn record_cost(&self, agent_id: AgentId, cost_usd: f64) {
        let mut map = self.inner.write().expect("budget lock poisoned");
        if let Some(budget) = map.get_mut(&agent_id) {
            budget.cost_usd += cost_usd;
        } else {
            let mut budget = ContextBudget::new(agent_id, 100_000);
            budget.cost_usd = cost_usd;
            map.insert(agent_id, budget);
        }
    }

    /// Check an agent's remaining budget.
    pub fn check_budget(&self, agent_id: AgentId) -> Option<ContextBudget> {
        let map = self.inner.read().expect("budget lock poisoned");
        map.get(&agent_id).cloned()
    }

    /// Check if the agent is approaching context limits and should summarize.
    pub fn should_summarize(&self, agent_id: AgentId) -> bool {
        let map = self.inner.read().expect("budget lock poisoned");
        map.get(&agent_id)
            .map(|b| b.should_summarize())
            .unwrap_or(false)
    }

    /// Get all agents that currently have an active cost or token alert.
    pub fn agents_in_alert(&self) -> Vec<(AgentId, bool, bool)> {
        let map = self.inner.read().expect("budget lock poisoned");
        map.values()
            .filter(|b| b.token_alert() || b.cost_alert())
            .map(|b| (b.agent_id, b.token_alert(), b.cost_alert()))
            .collect()
    }

    /// Trigger a period rollover for all agents.
    /// Returns map of agent_id → rollover_tokens_granted.
    pub fn rollover_all(&self) -> HashMap<AgentId, usize> {
        let mut map = self.inner.write().expect("budget lock poisoned");
        map.values_mut()
            .map(|b| (b.agent_id, b.rollover()))
            .collect()
    }

    /// Total cost across all agents.
    pub fn total_cost_usd(&self) -> f64 {
        let map = self.inner.read().expect("budget lock poisoned");
        map.values().map(|b| b.cost_usd).sum()
    }

    /// Cumulative cost in USD for a specific agent.
    pub fn cost_usd(&self, agent_id: AgentId) -> f64 {
        let map = self.inner.read().expect("budget lock poisoned");
        map.get(&agent_id).map(|b| b.cost_usd).unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_tracking_and_summarization() {
        let manager = BudgetManager::new();
        let agent = AgentId(1);

        manager.reset(agent, 10_000);
        let b = manager.check_budget(agent).unwrap();
        assert_eq!(b.tokens_available(), 10_000);

        manager.record_usage(agent, 7_000);
        assert!(!manager.should_summarize(agent));

        manager.record_usage(agent, 2_000);
        // Total = 9_000, 90% full.
        assert!(manager.should_summarize(agent));
        assert_eq!(
            manager.check_budget(agent).unwrap().tokens_available(),
            1_000
        );
    }

    #[test]
    fn per_agent_allocation_cap() {
        let mgr = BudgetManager::new();
        let agent = AgentId(2);
        let alloc = AgentBudgetAllocation::new(5_000, 1.00).with_alert_thresholds(0.6, 0.8);
        mgr.set_allocation(agent, alloc);

        mgr.record_usage(agent, 3_001); // > 60% of 5000
        let b = mgr.check_budget(agent).unwrap();
        assert!(b.token_alert());
    }

    #[test]
    fn cost_alert_fires_above_threshold() {
        let mgr = BudgetManager::new();
        let agent = AgentId(3);
        let alloc = AgentBudgetAllocation::new(100_000, 10.0).with_alert_thresholds(0.8, 0.9);
        mgr.set_allocation(agent, alloc);
        mgr.record_cost(agent, 9.01); // 90.1% of $10.0
        let b = mgr.check_budget(agent).unwrap();
        assert!(b.cost_alert());
        assert!(!b.cost_exceeded());

        mgr.record_cost(agent, 1.0); // now > $10.0
        let b = mgr.check_budget(agent).unwrap();
        assert!(b.cost_exceeded());
    }

    #[test]
    fn rollover_grants_unused_tokens() {
        let mgr = BudgetManager::new();
        let agent = AgentId(4);
        let alloc = AgentBudgetAllocation::new(10_000, 5.0).with_rollover(0.5);
        mgr.set_allocation(agent, alloc);
        mgr.record_usage(agent, 4_000); // used 4000, 6000 remaining
        let rollovers = mgr.rollover_all();
        let granted = rollovers[&agent];
        assert_eq!(granted, 3_000, "50% of 6000 unused = 3000 rollover");

        // After rollover, usage resets but rollover_tokens = 3000
        let b = mgr.check_budget(agent).unwrap();
        assert_eq!(b.tokens_used, 0);
        assert_eq!(b.rollover_tokens, 3_000);
        assert_eq!(b.effective_max_tokens(), 13_000);
    }

    #[test]
    fn total_cost_aggregation() {
        let mgr = BudgetManager::new();
        mgr.record_cost(AgentId(1), 0.50);
        mgr.record_cost(AgentId(2), 0.75);
        mgr.record_cost(AgentId(3), 0.25);
        let total = mgr.total_cost_usd();
        assert!((total - 1.50).abs() < 1e-9);
    }
}
