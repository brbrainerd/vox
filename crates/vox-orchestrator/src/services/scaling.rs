//! Scaling service: scale-up and scale-down decisions based on load and policy.
//!
//! Produces scaling actions (spawn dynamic agents, retire idle ones) that
//! the orchestrator applies. Scale-down is guarded so agents with critical
//! work are not retired.

use std::time::SystemTime;

use crate::config::OrchestratorConfig;
use crate::orchestrator::OrchestratorStatus;
use crate::types::AgentId;

/// Action recommended by the scaling service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingAction {
    /// No change.
    NoOp,
    /// Spawn one dynamic agent with the given name.
    ScaleUp { name: String },
    /// Retire these dynamic agent IDs (idle and past retirement threshold).
    ScaleDown { agent_ids: Vec<AgentId> },
}

/// Idle dynamic agent candidate for scale-down (agent id and last activity time).
pub type IdleDynamicAgent = (AgentId, SystemTime);

/// Stateless scaling service.
pub struct ScalingService;

impl ScalingService {
    /// Decide scale-up or scale-down based on current status, config, profile, and history.
    ///
    /// `idle_dynamic` should list dynamic agents that are currently idle (queued == 0, !in_progress)
    /// with their last_active time. Scale-down only retires those past effective retirement time.
    /// Never retires below `min_agents`. Scale-up when per-agent load exceeds effective threshold.
    /// Also scales down aggressively if total cost exceeds alert threshold.
    pub fn decide_scaling(
        status: &OrchestratorStatus,
        config: &OrchestratorConfig,
        _load_history: &[f64],
        idle_dynamic: &[IdleDynamicAgent],
        budgets: &crate::budget::BudgetManager,
    ) -> ScalingAction {
        if !config.scaling_enabled {
            return ScalingAction::NoOp;
        }

        let cost_critical = !budgets.agents_in_alert().is_empty();

        let profile = config.scaling_profile;
        let agent_count = status.agent_count;
        let threshold = config.scaling_threshold as f64 * profile.threshold_multiplier();
        let max_agents = config.max_agents;
        let min_agents = config.min_agents;
        let retirement_ms = if cost_critical {
            ((config.idle_retirement_ms as f64 * profile.retirement_multiplier()) / 2.0) as u64
        } else {
            (config.idle_retirement_ms as f64 * profile.retirement_multiplier()) as u64
        };

        let max_relevant_load = status.total_weighted_load.max(status.predicted_load);

        // Scale up: current or predicted load per agent exceeds effective threshold
        // Guard: don't scale up if we are already in cost alert
        if agent_count < max_agents && !cost_critical {
            let per_agent = if agent_count > 0 {
                max_relevant_load / agent_count as f64
            } else {
                max_relevant_load
            };
            if per_agent >= threshold {
                let name = format!("transient-{}", agent_count + 1);
                return ScalingAction::ScaleUp { name };
            }
        } else if agent_count == 0 && max_relevant_load > 0.0 {
            return ScalingAction::ScaleUp {
                name: "default".to_string(),
            };
        }

        // Scale down: retire idle dynamic agents past effective retirement time (safe: only idle agents)
        let now = SystemTime::now();
        let mut to_retire = Vec::new();
        for (agent_id, last_active) in idle_dynamic {
            if (agent_count - to_retire.len()) <= min_agents {
                break;
            }
            if let Ok(elapsed) = now.duration_since(*last_active) {
                if elapsed.as_millis() as u64 > retirement_ms {
                    to_retire.push(*agent_id);
                }
            }
        }

        if !to_retire.is_empty() {
            return ScalingAction::ScaleDown {
                agent_ids: to_retire,
            };
        }

        ScalingAction::NoOp
    }

    /// Compute predicted load from load history (for status display).
    pub fn predicted_load(load_history: &[f64], current: f64) -> f64 {
        if load_history.is_empty() {
            return current;
        }
        let avg: f64 = load_history.iter().copied().sum::<f64>() / load_history.len() as f64;
        if load_history.len() >= 2 {
            let last = load_history[load_history.len() - 1];
            let trend = last - avg;
            (last + trend).max(0.0)
        } else {
            avg
        }
    }
}
