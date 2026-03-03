//! Cost-aware rebalancing and dynamic scaling for orchestrator agents.

use crate::budget::BudgetManager;
use crate::models::ModelConfig;
use crate::queue::AgentQueue;
use crate::types::AgentId;
use std::collections::HashMap;

/// Strategies for rebalancing work across agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RebalanceStrategy {
    /// Simply distribute to the agent with the shortest queue.
    ShortestQueue,
    /// Distribute based on model cost (prefer cheaper agents if capable).
    LowestCost,
    /// Balance both queue size and cost.
    Hybrid,
}

/// Decisions for dynamic scaling.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalingAction {
    /// Start a new dynamic agent to handle load.
    ScaleUp {
        agent_name: String,
        model: ModelConfig,
    },
    /// Stop an idle agent to save cost.
    ScaleDown { agent_id: AgentId },
    /// No action needed.
    None,
}

/// Logic for cost-aware rebalancing and dynamic scaling.
pub struct LoadBalancer {
    strategy: RebalanceStrategy,
    scale_up_threshold: usize,
}

impl LoadBalancer {
    pub fn new(strategy: RebalanceStrategy) -> Self {
        Self {
            strategy,
            scale_up_threshold: 10, // More than 10 items in queue
        }
    }

    /// Determine which agent should receive a new task.
    pub fn pick_agent(
        &self,
        queues: &HashMap<AgentId, AgentQueue>,
        budgets: &BudgetManager,
        _required_model_tags: &[String],
    ) -> Option<AgentId> {
        let mut candidates: Vec<(AgentId, f64)> = Vec::new();

        for (id, queue) in queues {
            let queue_size = queue.len() as f64;
            let cost_score = budgets.cost_usd(*id) * 1000.0; // weighting

            let score = match self.strategy {
                RebalanceStrategy::ShortestQueue => queue_size,
                RebalanceStrategy::LowestCost => cost_score,
                RebalanceStrategy::Hybrid => queue_size + (cost_score * 0.5),
            };

            candidates.push((*id, score));
        }

        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.first().map(|(id, _)| *id)
    }

    /// Evaluate if we need to scale up or down.
    pub fn evaluate_scaling(
        &self,
        queues: &HashMap<AgentId, AgentQueue>,
        dynamic_agents: &[AgentId],
    ) -> ScalingAction {
        let total_queued: usize = queues.values().map(|q| q.len()).sum();
        let avg_load = if queues.is_empty() {
            0.0
        } else {
            total_queued as f64 / queues.len() as f64
        };

        if avg_load > self.scale_up_threshold as f64 {
            return ScalingAction::ScaleUp {
                agent_name: "dynamic-worker".to_string(),
                model: ModelConfig::default(),
            };
        }

        // Check if any dynamic agent is totally idle
        for id in dynamic_agents {
            if let Some(q) = queues.get(id) {
                if q.is_empty() && q.in_progress_count() == 0 {
                    return ScalingAction::ScaleDown { agent_id: *id };
                }
            }
        }

        ScalingAction::None
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new(RebalanceStrategy::Hybrid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentId;

    #[test]
    fn pick_agent_shortest_queue() {
        let lb = LoadBalancer::new(RebalanceStrategy::ShortestQueue);
        let mut queues = HashMap::new();
        queues.insert(AgentId(1), AgentQueue::new(AgentId(1), "a"));
        queues.insert(AgentId(2), AgentQueue::new(AgentId(2), "b"));

        // Add task to agent 1
        let mut q1 = AgentQueue::new(AgentId(1), "a");
        q1.enqueue(crate::types::AgentTask::new(
            crate::types::TaskId(1),
            "t1",
            crate::types::TaskPriority::Normal,
            vec![],
        ));
        queues.insert(AgentId(1), q1);

        let budgets = BudgetManager::new();
        let picked = lb.pick_agent(&queues, &budgets, &[]);
        assert_eq!(picked, Some(AgentId(2)));
    }

    #[test]
    fn scale_up_on_high_load() {
        let lb = LoadBalancer::new(RebalanceStrategy::Hybrid);
        let mut queues = HashMap::new();
        let mut q = AgentQueue::new(AgentId(1), "test");
        for i in 0..15 {
            q.enqueue(crate::types::AgentTask::new(
                crate::types::TaskId(i),
                "t",
                crate::types::TaskPriority::Normal,
                vec![],
            ));
        }
        queues.insert(AgentId(1), q);

        let action = lb.evaluate_scaling(&queues, &[]);
        assert!(matches!(action, ScalingAction::ScaleUp { .. }));
    }
}
