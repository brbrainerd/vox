//! Agent leaderboard for ranking agents by various metrics.
//!
//! Tracks tasks completed, code quality, speed, cost efficiency,
//! and generates sortable leaderboard views.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A metric category for leaderboard ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardMetric {
    /// Most tasks completed.
    TasksCompleted,
    /// Highest code quality score.
    CodeQuality,
    /// Fastest average task completion.
    Speed,
    /// Lowest cost per task.
    CostEfficiency,
    /// Most error-free completions.
    Reliability,
    /// Most handoffs completed.
    Collaboration,
}

impl std::fmt::Display for LeaderboardMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TasksCompleted => write!(f, "Tasks Completed"),
            Self::CodeQuality => write!(f, "Code Quality"),
            Self::Speed => write!(f, "Speed"),
            Self::CostEfficiency => write!(f, "Cost Efficiency"),
            Self::Reliability => write!(f, "Reliability"),
            Self::Collaboration => write!(f, "Collaboration"),
        }
    }
}

/// Per-agent stats tracked for the leaderboard.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_id: String,
    pub agent_name: String,
    pub tasks_completed: u32,
    pub tasks_failed: u32,
    pub code_quality_sum: u32,
    pub code_quality_count: u32,
    pub total_duration_ms: u64,
    pub total_cost_usd: f64,
    pub handoffs_completed: u32,
}

impl AgentStats {
    /// Average code quality (0-100).
    pub fn avg_code_quality(&self) -> u32 {
        if self.code_quality_count == 0 {
            50
        } else {
            self.code_quality_sum / self.code_quality_count
        }
    }

    /// Average task duration in milliseconds.
    pub fn avg_duration_ms(&self) -> u64 {
        if self.tasks_completed == 0 {
            0
        } else {
            self.total_duration_ms / self.tasks_completed as u64
        }
    }

    /// Cost per task in USD.
    pub fn cost_per_task(&self) -> f64 {
        if self.tasks_completed == 0 {
            0.0
        } else {
            self.total_cost_usd / self.tasks_completed as f64
        }
    }

    /// Reliability percentage (completed / total).
    pub fn reliability_pct(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            100.0
        } else {
            self.tasks_completed as f64 / total as f64 * 100.0
        }
    }

    /// Get the value for a specific metric (for sorting).
    pub fn metric_value(&self, metric: LeaderboardMetric) -> f64 {
        match metric {
            LeaderboardMetric::TasksCompleted => self.tasks_completed as f64,
            LeaderboardMetric::CodeQuality => self.avg_code_quality() as f64,
            LeaderboardMetric::Speed => -(self.avg_duration_ms() as f64), // Lower is better
            LeaderboardMetric::CostEfficiency => -self.cost_per_task(),   // Lower is better
            LeaderboardMetric::Reliability => self.reliability_pct(),
            LeaderboardMetric::Collaboration => self.handoffs_completed as f64,
        }
    }
}

/// A row in the leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub agent_id: String,
    pub agent_name: String,
    pub value: String,
    pub metric: LeaderboardMetric,
}

/// Agent leaderboard.
#[derive(Debug, Default)]
pub struct Leaderboard {
    stats: HashMap<String, AgentStats>,
}

impl Leaderboard {
    /// Create a new leaderboard.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create stats for an agent.
    pub fn agent_stats(&mut self, agent_id: &str, agent_name: &str) -> &mut AgentStats {
        self.stats
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentStats {
                agent_id: agent_id.to_string(),
                agent_name: agent_name.to_string(),
                ..Default::default()
            })
    }

    /// Record a completed task.
    pub fn record_completion(&mut self, agent_id: &str, duration_ms: u64, cost_usd: f64) {
        let stats = self.stats.entry(agent_id.to_string()).or_default();
        stats.tasks_completed += 1;
        stats.total_duration_ms += duration_ms;
        stats.total_cost_usd += cost_usd;
    }

    /// Record a failed task.
    pub fn record_failure(&mut self, agent_id: &str) {
        let stats = self.stats.entry(agent_id.to_string()).or_default();
        stats.tasks_failed += 1;
    }

    /// Record a code quality score.
    pub fn record_quality(&mut self, agent_id: &str, quality: u32) {
        let stats = self.stats.entry(agent_id.to_string()).or_default();
        stats.code_quality_sum += quality;
        stats.code_quality_count += 1;
    }

    /// Record a handoff.
    pub fn record_handoff(&mut self, agent_id: &str) {
        let stats = self.stats.entry(agent_id.to_string()).or_default();
        stats.handoffs_completed += 1;
    }

    /// Get the ranked leaderboard for a given metric.
    pub fn ranked(&self, metric: LeaderboardMetric) -> Vec<LeaderboardEntry> {
        let mut sorted: Vec<&AgentStats> = self.stats.values().collect();
        sorted.sort_by(|a, b| {
            b.metric_value(metric)
                .partial_cmp(&a.metric_value(metric))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let value = match metric {
                    LeaderboardMetric::TasksCompleted => format!("{}", s.tasks_completed),
                    LeaderboardMetric::CodeQuality => format!("{}%", s.avg_code_quality()),
                    LeaderboardMetric::Speed => format!("{}ms avg", s.avg_duration_ms()),
                    LeaderboardMetric::CostEfficiency => format!("${:.4}/task", s.cost_per_task()),
                    LeaderboardMetric::Reliability => format!("{:.1}%", s.reliability_pct()),
                    LeaderboardMetric::Collaboration => format!("{}", s.handoffs_completed),
                };

                LeaderboardEntry {
                    rank: (i + 1) as u32,
                    agent_id: s.agent_id.clone(),
                    agent_name: s.agent_name.clone(),
                    value,
                    metric,
                }
            })
            .collect()
    }

    /// Generate a markdown leaderboard for a given metric.
    pub fn to_markdown(&self, metric: LeaderboardMetric) -> String {
        let entries = self.ranked(metric);
        let mut md = format!("## {} Leaderboard\n\n", metric);
        md.push_str("| Rank | Agent | Score |\n|---|---|---|\n");
        for entry in &entries {
            let medal = match entry.rank {
                1 => "🥇",
                2 => "🥈",
                3 => "🥉",
                _ => "  ",
            };
            md.push_str(&format!(
                "| {} {} | {} | {} |\n",
                medal, entry.rank, entry.agent_name, entry.value
            ));
        }
        md
    }

    /// Number of tracked agents.
    pub fn agent_count(&self) -> usize {
        self.stats.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_rank() {
        let mut lb = Leaderboard::new();

        lb.agent_stats("a1", "builder");
        lb.agent_stats("a2", "reviewer");

        lb.record_completion("a1", 5000, 0.01);
        lb.record_completion("a1", 3000, 0.005);
        lb.record_completion("a2", 10000, 0.02);

        let ranked = lb.ranked(LeaderboardMetric::TasksCompleted);
        assert_eq!(ranked[0].agent_id, "a1"); // 2 tasks
        assert_eq!(ranked[1].agent_id, "a2"); // 1 task
    }

    #[test]
    fn cost_efficiency_ranking() {
        let mut lb = Leaderboard::new();

        lb.agent_stats("cheap", "cheap-bot");
        lb.agent_stats("expensive", "pricey-bot");

        lb.record_completion("cheap", 1000, 0.001); // $0.001/task
        lb.record_completion("expensive", 1000, 0.05); // $0.05/task

        let ranked = lb.ranked(LeaderboardMetric::CostEfficiency);
        assert_eq!(ranked[0].agent_id, "cheap"); // Better cost efficiency
    }

    #[test]
    fn reliability_ranking() {
        let mut lb = Leaderboard::new();

        lb.agent_stats("reliable", "r-bot");
        lb.agent_stats("flaky", "f-bot");

        lb.record_completion("reliable", 1000, 0.01);
        lb.record_completion("reliable", 1000, 0.01);

        lb.record_completion("flaky", 1000, 0.01);
        lb.record_failure("flaky");

        let ranked = lb.ranked(LeaderboardMetric::Reliability);
        assert_eq!(ranked[0].agent_id, "reliable"); // 100% vs 50%
    }

    #[test]
    fn markdown_output() {
        let mut lb = Leaderboard::new();
        lb.agent_stats("a1", "builder");
        lb.record_completion("a1", 5000, 0.01);

        let md = lb.to_markdown(LeaderboardMetric::TasksCompleted);
        assert!(md.contains("🥇"));
        assert!(md.contains("builder"));
    }
}
