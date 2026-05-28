//! Scaling service: scale-up and scale-down decisions based on load and policy.
//!
//! Produces scaling actions (spawn dynamic agents, retire idle ones) that
//! the orchestrator applies. Scale-down is guarded so agents with critical
//! work are not retired.

use crate::config::OrchestratorConfig;
use crate::orchestrator::OrchestratorStatus;
use crate::types::AgentId;

/// Action recommended by the scaling service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingAction {
    /// No change.
    NoOp,
    /// Spawn dynamic agents with the given name prefix.
    ScaleUp {
        /// Worker name prefix for logging and process naming.
        name_prefix: String,
        /// Number of agents to spawn in this batch.
        count: usize,
    },
    /// Retire these dynamic agent IDs (idle and past retirement threshold).
    ScaleDown {
        /// Dynamic agents eligible for teardown.
        agent_ids: Vec<AgentId>,
    },
}

/// Idle dynamic agent candidate for scale-down (agent id and last activity time).
pub type IdleDynamicAgent = (AgentId, std::time::Instant);

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
        _remote_gpu_capacity: usize,
        idle_dynamic: &[IdleDynamicAgent],
        budgets: &crate::budget::BudgetManager,
    ) -> ScalingAction {
        if !config.scaling_enabled {
            return ScalingAction::NoOp;
        }

        let cost_critical = !budgets.agents_in_alert().is_empty() || budgets.is_fatigued();

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

        let queue_pressure_boost = if status.total_queued > config.scaling_threshold {
            0.5
        } else {
            0.0
        };
        // Treat reported remote GPU headroom as capacity relief for scale-up pressure (tests +
        // mesh dashboards pass a non-zero hint). Purely heuristic — does not assume tasks are GPU-bound.
        let remote_gpu_relief = if _remote_gpu_capacity == 0 {
            1.0
        } else {
            1.0 / (1.0 + 0.15 * _remote_gpu_capacity as f64)
        };
        let max_relevant_load =
            status.total_weighted_load.max(status.predicted_load) + queue_pressure_boost;

        // Scale up: current or predicted load per agent exceeds effective threshold
        // Guard: don't scale up if we are already in cost alert
        if agent_count < max_agents && !cost_critical {
            let per_agent = if agent_count > 0 {
                max_relevant_load / agent_count as f64
            } else {
                max_relevant_load
            };
            if per_agent * remote_gpu_relief >= threshold {
                let name_prefix = "transient".to_string();
                let desired_new = ((max_relevant_load - (agent_count as f64 * threshold))
                    / threshold)
                    .ceil() as usize;
                let count = desired_new.clamp(1, max_agents.saturating_sub(agent_count));
                return ScalingAction::ScaleUp { name_prefix, count };
            }
        } else if agent_count == 0 && max_relevant_load > 0.0 {
            return ScalingAction::ScaleUp {
                name_prefix: "default".to_string(),
                count: 1,
            };
        }

        // Scale down: retire idle dynamic agents past effective retirement time (safe: only idle agents)
        let now = std::time::Instant::now();
        let mut to_retire = Vec::new();
        for (agent_id, last_active) in idle_dynamic {
            if (agent_count - to_retire.len()) <= min_agents {
                break;
            }
            let elapsed = now.duration_since(*last_active).as_millis() as u64;
            if elapsed > retirement_ms {
                to_retire.push(*agent_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::BudgetManager;
    use crate::orchestrator::{AgentSummary, OrchestratorStatus};
    use crate::types::AgentId;

    fn status(total_queued: usize, total_weighted_load: f64) -> OrchestratorStatus {
        OrchestratorStatus {
            enabled: true,
            agent_count: 1,
            total_queued,
            total_in_progress: 0,
            total_completed: 0,
            locked_files: 0,
            total_contention: 0,
            total_doubted: 0,
            total_weighted_load,
            predicted_load: total_weighted_load,
            reserved_agents: 0,
            dynamic_agents: 0,
            context_entries: std::collections::HashMap::new(),
            max_handoff_count: 0,
            agents: vec![AgentSummary {
                id: AgentId(1),
                name: "a1".to_string(),
                queued: total_queued,
                urgent_count: 0,
                normal_count: total_queued,
                background_count: 0,
                in_progress: false,
                completed: 0,
                doubted_count: 0,
                paused: false,
                owned_files: 0,
                dynamic: false,
                weighted_load: total_weighted_load,
                agent_session_id: None,
                max_handoff_count: 0,
                active_skill: None,
                current_phase: None,
            }],
        }
    }

    #[test]
    fn scales_up_when_local_pressure_is_high() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.scaling_enabled = true;
        cfg.max_agents = 4;
        cfg.scaling_threshold = 1;
        let action = ScalingService::decide_scaling(
            &status(5, 3.0),
            &cfg,
            &[],
            0,
            &[],
            &BudgetManager::new(None),
        );
        assert!(matches!(action, ScalingAction::ScaleUp { .. }));
    }

    #[test]
    fn remote_gpu_capacity_reduces_scale_up_pressure() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.scaling_enabled = true;
        cfg.max_agents = 4;
        cfg.scaling_threshold = 3;
        let action = ScalingService::decide_scaling(
            &status(3, 3.0),
            &cfg,
            &[],
            3,
            &[],
            &BudgetManager::new(None),
        );
        assert!(matches!(action, ScalingAction::NoOp));
    }
}
