//! Cost aggregation and per-session/per-agent cost tracking.
//!
//! Tracks costs incurred by AI API calls (OpenRouter, Gemini, etc.)
//! and provides aggregation, budget alerts, and reporting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single cost record for an API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    /// Agent that incurred the cost.
    pub agent_id: String,
    /// Session identifier (for grouping).
    pub session_id: Option<String>,
    /// Provider name (e.g., "openrouter", "ollama", "gemini").
    pub provider: String,
    /// Model used.
    pub model: String,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens generated.
    pub output_tokens: u32,
    /// Cost in USD.
    pub cost_usd: f64,
    /// Timestamp in unix milliseconds.
    pub timestamp_ms: u64,
}

impl CostRecord {
    /// Create a new cost record with current timestamp.
    pub fn new(
        agent_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            agent_id: agent_id.into(),
            session_id: None,
            provider: provider.into(),
            model: model.into(),
            input_tokens,
            output_tokens,
            cost_usd,
            timestamp_ms,
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

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
        *self.by_model.entry(record.model.clone()).or_default() += record.cost_usd;
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

/// In-memory cost aggregator.
///
/// Tracks costs per agent (and optionally per session) and provides
/// budget alert functionality.
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
mod tests {
    use super::*;

    #[test]
    fn record_and_summarize() {
        let mut agg = CostAggregator::new();

        agg.record(CostRecord::new(
            "agent-1",
            "openrouter",
            "claude-3",
            100,
            50,
            0.005,
        ));
        agg.record(CostRecord::new(
            "agent-1",
            "openrouter",
            "claude-3",
            200,
            100,
            0.010,
        ));
        agg.record(CostRecord::new(
            "agent-2", "ollama", "llama3", 500, 200, 0.0,
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

        agg.record(CostRecord::new(
            "agent-1",
            "openrouter",
            "gpt4",
            100,
            50,
            0.50,
        ));
        assert!(!agg.budget_alert("agent-1")); // 50% used

        agg.record(CostRecord::new(
            "agent-1",
            "openrouter",
            "gpt4",
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
        agg.record(CostRecord::new(
            "agent-1",
            "openrouter",
            "claude-3",
            100,
            50,
            0.005,
        ));
        let report = agg.report_markdown();
        assert!(report.contains("# Cost Report"));
        assert!(report.contains("$0.0050"));
    }
}
