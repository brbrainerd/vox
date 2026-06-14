//! Cost aggregation and per-session/per-agent cost tracking.
//!
//! Tracks costs incurred by AI API calls (OpenRouter, Gemini, etc.)
//! and provides aggregation, budget alerts, and reporting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::db::CostRecord;

/// Aggregated cost summary for an agent or session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostSummary {
    /// Total number of API calls.
    pub call_count: u32,
    /// Total input tokens.
    pub total_input_tokens: u64,
    /// Total output tokens.
    pub total_output_tokens: u64,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Breakdown by provider.
    pub by_provider: HashMap<String, f64>,
    /// Breakdown by model.
    pub by_model: HashMap<String, f64>,
}

impl CostSummary {
    /// Add a record to this summary.
    pub fn add(&mut self, record: &CostRecord) {
        self.call_count += 1;
        self.total_input_tokens += record.input_tokens as u64;
        self.total_output_tokens += record.output_tokens as u64;
        self.total_cost_usd += record.cost_usd;
        *self.by_provider.entry(record.provider.clone()).or_default() += record.cost_usd;
        if let Some(m) = &record.model {
            *self.by_model.entry(m.clone()).or_default() += record.cost_usd;
        }
    }

    /// Average cost per call.
    pub fn avg_cost_per_call(&self) -> f64 {
        if self.call_count == 0 {
            0.0
        } else {
            self.total_cost_usd / self.call_count as f64
        }
    }
}

#[derive(Debug, Default)]
pub struct CostAggregator {
    /// Per-agent records.
    records: HashMap<String, Vec<CostRecord>>,
    /// Optional budget limit per agent (USD).
    budget_limits: HashMap<String, f64>,
}

impl CostAggregator {
    /// Create a new empty aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cost.
    pub fn record(&mut self, record: CostRecord) {
        self.records
            .entry(record.agent_id.clone())
            .or_default()
            .push(record);
    }

    /// Set a budget limit for an agent (in USD).
    pub fn set_budget_limit(&mut self, agent_id: impl Into<String>, limit_usd: f64) {
        self.budget_limits.insert(agent_id.into(), limit_usd);
    }

    /// Get the aggregated cost summary for an agent.
    pub fn agent_summary(&self, agent_id: &str) -> CostSummary {
        let mut summary = CostSummary::default();
        if let Some(records) = self.records.get(agent_id) {
            for r in records {
                summary.add(r);
            }
        }
        summary
    }

    /// Get the total cost across all agents.
    pub fn total_summary(&self) -> CostSummary {
        let mut summary = CostSummary::default();
        for records in self.records.values() {
            for r in records {
                summary.add(r);
            }
        }
        summary
    }

    /// Check if an agent has exceeded its budget limit.
    /// Returns Some(remaining) if within budget, None if no limit set.
    pub fn budget_remaining(&self, agent_id: &str) -> Option<f64> {
        let limit = self.budget_limits.get(agent_id)?;
        let summary = self.agent_summary(agent_id);
        Some(limit - summary.total_cost_usd)
    }

    /// Check if an agent is approaching its budget (> 80% used).
    pub fn budget_alert(&self, agent_id: &str) -> bool {
        match self.budget_limits.get(agent_id) {
            Some(limit) => {
                let summary = self.agent_summary(agent_id);
                summary.total_cost_usd > limit * 0.8
            }
            None => false,
        }
    }

    /// Generate a markdown cost report.
    pub fn report_markdown(&self) -> String {
        let total = self.total_summary();
        let mut md = String::new();

        md.push_str("# Cost Report\n\n");
        md.push_str(&format!(
            "**Total:** ${:.4} across {} calls ({} input + {} output tokens)\n\n",
            total.total_cost_usd,
            total.call_count,
            total.total_input_tokens,
            total.total_output_tokens,
        ));

        if !total.by_provider.is_empty() {
            md.push_str("## By Provider\n\n");
            md.push_str("| Provider | Cost |\n|---|---|\n");
            for (provider, cost) in &total.by_provider {
                md.push_str(&format!("| {} | ${:.4} |\n", provider, cost));
            }
            md.push('\n');
        }

        // Per-agent breakdown
        if self.records.len() > 1 {
            md.push_str("## By Agent\n\n");
            md.push_str("| Agent | Calls | Cost | Budget |\n|---|---|---|---|\n");
            for (agent_id, records) in &self.records {
                let mut summary = CostSummary::default();
                for r in records {
                    summary.add(r);
                }
                let budget_str = self
                    .budget_remaining(agent_id)
                    .map(|r| format!("${:.4} remaining", r))
                    .unwrap_or_else(|| "unlimited".to_string());
                md.push_str(&format!(
                    "| {} | {} | ${:.4} | {} |\n",
                    agent_id, summary.call_count, summary.total_cost_usd, budget_str
                ));
            }
            md.push('\n');
        }

        md
    }

    /// Get the number of agents tracked.
    pub fn agent_count(&self) -> usize {
        self.records.len()
    }

    /// Get all agent IDs.
    pub fn agent_ids(&self) -> Vec<&str> {
        self.records.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod semcov_wave13_tests {
    use super::*;

    fn rec(agent: &str, cost: f64) -> CostRecord {
        CostRecord::new_ephemeral(agent, "openrouter", Some("m".into()), 100, 50, cost)
    }

    // ── avg_cost_per_call math ─────────────────────────────────────────────

    #[test]
    fn avg_cost_zero_calls_is_zero_not_nan() {
        // Catches: division by zero returning NaN instead of 0.0
        let s = CostSummary::default();
        assert_eq!(s.avg_cost_per_call(), 0.0);
        assert!(!s.avg_cost_per_call().is_nan());
    }

    #[test]
    fn avg_cost_single_call_equals_total() {
        // Catches: off-by-one in call_count; avg != cost when count wrong
        let mut s = CostSummary::default();
        s.add(&rec("a", 0.42));
        assert!((s.avg_cost_per_call() - 0.42).abs() < 1e-12);
    }

    #[test]
    fn avg_cost_two_calls_is_half_total() {
        // Catches: avg formula divides by wrong denominator
        let mut s = CostSummary::default();
        s.add(&rec("a", 0.20));
        s.add(&rec("a", 0.40));
        let avg = s.avg_cost_per_call();
        assert!((avg - 0.30).abs() < 1e-12, "avg={avg}");
    }

    // ── budget_alert threshold ─────────────────────────────────────────────

    #[test]
    fn budget_alert_false_at_exactly_80_percent() {
        // Catches: > vs >= boundary: 80% should NOT trigger alert (strictly > 0.8)
        let mut agg = CostAggregator::new();
        agg.set_budget_limit("a", 1.00);
        agg.record(rec("a", 0.80)); // exactly 80%
        assert!(
            !agg.budget_alert("a"),
            "alert should be false at exactly 80% (boundary is exclusive)"
        );
    }

    #[test]
    fn budget_alert_true_at_80_percent_plus_epsilon() {
        // Catches: wrong threshold direction (< instead of >)
        let mut agg = CostAggregator::new();
        agg.set_budget_limit("a", 1.00);
        agg.record(rec("a", 0.80001));
        assert!(agg.budget_alert("a"), "alert should fire just above 80%");
    }

    #[test]
    fn budget_remaining_negative_when_over_budget() {
        // Catches: budget_remaining returning None or 0 instead of negative
        let mut agg = CostAggregator::new();
        agg.set_budget_limit("a", 1.00);
        agg.record(rec("a", 1.50));
        let remaining = agg.budget_remaining("a").unwrap();
        assert!(
            remaining < 0.0,
            "remaining={remaining} should be negative when over budget"
        );
    }

    // ── token accumulation ─────────────────────────────────────────────────

    #[test]
    fn tokens_accumulate_across_calls() {
        // Catches: add() overwriting instead of accumulating token counts
        let mut s = CostSummary::default();
        s.add(&CostRecord::new_ephemeral("a", "p", None, 100, 50, 0.0));
        s.add(&CostRecord::new_ephemeral("a", "p", None, 200, 75, 0.0));
        assert_eq!(s.total_input_tokens, 300);
        assert_eq!(s.total_output_tokens, 125);
    }

    // ── no cross-agent leakage ─────────────────────────────────────────────

    #[test]
    fn agent_summary_does_not_include_other_agents() {
        // Catches: total_summary used instead of agent-filtered summary
        let mut agg = CostAggregator::new();
        agg.record(rec("agent-A", 5.0));
        agg.record(rec("agent-B", 3.0));
        let summary = agg.agent_summary("agent-A");
        assert_eq!(summary.call_count, 1);
        assert!((summary.total_cost_usd - 5.0).abs() < 1e-12);
    }

    // ── by_provider breakdown ─────────────────────────────────────────────

    #[test]
    fn by_provider_accumulates_across_calls_same_provider() {
        // Catches: by_provider inserting instead of summing per provider
        let mut s = CostSummary::default();
        s.add(&CostRecord::new_ephemeral(
            "a",
            "openrouter",
            None,
            10,
            5,
            1.0,
        ));
        s.add(&CostRecord::new_ephemeral(
            "a",
            "openrouter",
            None,
            20,
            10,
            2.0,
        ));
        let cost = s.by_provider.get("openrouter").copied().unwrap_or(0.0);
        assert!((cost - 3.0).abs() < 1e-12, "by_provider cost={cost}");
    }

    // ── model-less record handled ─────────────────────────────────────────

    #[test]
    fn record_with_no_model_skips_by_model_entry() {
        // Catches: unwrap() on None model panicking instead of skipping
        let mut s = CostSummary::default();
        s.add(&CostRecord::new_ephemeral("a", "p", None, 10, 5, 0.5));
        assert!(
            s.by_model.is_empty(),
            "no model → by_model should remain empty"
        );
        assert_eq!(s.call_count, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_summarize() {
        let mut agg = CostAggregator::new();

        agg.record(CostRecord::new_ephemeral(
            "agent-1",
            "openrouter",
            Some("claude-3".to_string()),
            100,
            50,
            0.005,
        ));
        agg.record(CostRecord::new_ephemeral(
            "agent-1",
            "openrouter",
            Some("claude-3".to_string()),
            200,
            100,
            0.010,
        ));
        agg.record(CostRecord::new_ephemeral(
            "agent-2",
            "ollama",
            Some("llama3".to_string()),
            500,
            200,
            0.0,
        ));

        let summary = agg.agent_summary("agent-1");
        assert_eq!(summary.call_count, 2);
        assert_eq!(summary.total_input_tokens, 300);
        assert!((summary.total_cost_usd - 0.015).abs() < 1e-10);

        let total = agg.total_summary();
        assert_eq!(total.call_count, 3);
        assert_eq!(agg.agent_count(), 2);
    }

    #[test]
    fn budget_tracking() {
        let mut agg = CostAggregator::new();
        agg.set_budget_limit("agent-1", 1.00);

        agg.record(CostRecord::new_ephemeral(
            "agent-1",
            "openrouter",
            Some("gpt4".to_string()),
            100,
            50,
            0.50,
        ));
        assert!(!agg.budget_alert("agent-1")); // 50% used

        agg.record(CostRecord::new_ephemeral(
            "agent-1",
            "openrouter",
            Some("gpt4".to_string()),
            100,
            50,
            0.40,
        ));
        assert!(agg.budget_alert("agent-1")); // 90% used

        let remaining = agg.budget_remaining("agent-1").unwrap();
        assert!((remaining - 0.10).abs() < 1e-10);
    }

    #[test]
    fn no_budget_set() {
        let agg = CostAggregator::new();
        assert!(!agg.budget_alert("agent-1"));
        assert!(agg.budget_remaining("agent-1").is_none());
    }

    #[test]
    fn markdown_report() {
        let mut agg = CostAggregator::new();
        agg.record(CostRecord::new_ephemeral(
            "agent-1",
            "openrouter",
            Some("gpt4".to_string()),
            1000,
            500,
            5.0,
        ));
        let report = agg.report_markdown();
        assert!(report.contains("# Cost Report"));
        assert!(report.contains("$5.0000"));
    }
}
