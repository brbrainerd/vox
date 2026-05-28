//! Live wiring: MCP tool classification → [`crate::orchestrator_policy::PolicyContext`] for D5 risk overlay.
//!
//! [`AgentosPolicyLedger`] is updated after each completed MCP tool dispatch and consumed when callers
//! evaluate unified orchestrator policy for an agent.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::orchestrator_policy::{
    OrchestratorPolicy, OrchestratorPolicyConfig, PolicyContext, PolicyDecision,
};

use super::mutation_classifier::mutation_kind_for_tool;

/// Thread-safe ledger: last `mutation_kind` per MCP numeric `agent_id`, plus stateful [`OrchestratorPolicy`].
pub struct AgentosPolicyLedger {
    inner: Mutex<AgentosPolicyLedgerInner>,
}

struct AgentosPolicyLedgerInner {
    policy: OrchestratorPolicy,
    /// Last MCP tool `mutation_kind` string per `agent_id` from tool args (`0` = unknown / global fallback).
    last_mutation_kind_by_agent: HashMap<u64, String>,
}

impl AgentosPolicyLedger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(AgentosPolicyLedgerInner {
                policy: OrchestratorPolicy::new(
                    OrchestratorPolicyConfig::default().for_agentos_policy_ledger(),
                ),
                last_mutation_kind_by_agent: HashMap::new(),
            }),
        }
    }

    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Record classification after an MCP tool completes (success or tool-level failure).
    pub fn record_mcp_tool(&self, agent_id: Option<u64>, canonical_tool_name: &str) {
        let mk = mutation_kind_for_tool(canonical_tool_name).to_string();
        let mut g = self.inner.lock();
        let key = agent_id.unwrap_or(0);
        g.last_mutation_kind_by_agent.insert(key, mk);
    }

    /// Evaluate policy with AgentOS mutation overlay for the given MCP agent id (`None` → key `0`).
    #[must_use]
    pub fn evaluate_for_agent(&self, agent_id: Option<u64>) -> PolicyDecision {
        let mut g = self.inner.lock();
        let key = agent_id.unwrap_or(0);
        let mk = g
            .last_mutation_kind_by_agent
            .get(&key)
            .cloned()
            .or_else(|| g.last_mutation_kind_by_agent.get(&0).cloned());
        let ctx = PolicyContext {
            agentos_last_mutation_kind: mk,
            ..PolicyContext::default()
        };
        g.policy.evaluate(&ctx)
    }
}

impl Default for AgentosPolicyLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_records_mutation_and_boosts_risk_for_external_vs_read() {
        let ledger = AgentosPolicyLedger::new();
        ledger.record_mcp_tool(Some(7), "vox_git_status");
        let r_read = ledger.evaluate_for_agent(Some(7)).risk_score;
        ledger.record_mcp_tool(Some(7), "vox_run_shell");
        let r_ext = ledger.evaluate_for_agent(Some(7)).risk_score;
        assert!(r_ext > r_read, "read={r_read} ext={r_ext}");
    }
}
