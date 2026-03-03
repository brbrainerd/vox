//! Routing service: file-affinity and group-based task routing.
//!
//! Decides which agent (existing or to be spawned) should receive a task
//! based on file manifest, affinity map, affinity groups, and load.

use std::collections::HashMap;

use crate::affinity::FileAffinityMap;
use crate::config::OrchestratorConfig;
use crate::groups::AffinityGroupRegistry;
use crate::queue::AgentQueue;
use crate::types::{AgentId, FileAffinity};

/// Result of a routing decision: either use an existing agent or spawn one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResult {
    /// Route to this existing agent.
    Existing(AgentId),
    /// Spawn a new agent with this name and route the task to it.
    SpawnAgent(String),
}

/// Stateless routing service implementing file-affinity and group voting.
pub struct RoutingService;

impl RoutingService {
    /// Route a task to the best agent based on file affinity and group voting.
    ///
    /// Returns either an existing agent ID or the name to use when spawning
    /// a new agent. The caller is responsible for spawning when `SpawnAgent` is returned.
    pub fn route(
        manifest: &[FileAffinity],
        affinity_map: &FileAffinityMap,
        groups: &AffinityGroupRegistry,
        agents: &HashMap<AgentId, AgentQueue>,
        config: &OrchestratorConfig,
    ) -> RouteResult {
        if manifest.is_empty() {
            return Self::least_loaded_or_spawn(agents, config);
        }

        let mut scores: HashMap<AgentId, f64> = HashMap::new();

        // 1. Direct file affinity (strongest signal)
        for fa in manifest {
            if let Some(owner) = affinity_map.lookup(&fa.path) {
                *scores.entry(owner).or_insert(0.0) += 10.0;
            }
        }

        // 2. Group affinity voting
        for fa in manifest {
            if let Some(group) = groups.resolve(&fa.path) {
                if let Some(default_agent) = group.default_agent {
                    if agents.contains_key(&default_agent) {
                        *scores.entry(default_agent).or_insert(0.0) += 15.0;
                    }
                }
                for (agent_id, queue) in agents {
                    if queue.name == group.name {
                        *scores.entry(*agent_id).or_insert(0.0) += 5.0;
                    }
                }
            }
        }

        // 3. Weight by load (prefer emptier agents on ties)
        for (agent_id, score) in scores.iter_mut() {
            if let Some(queue) = agents.get(agent_id) {
                *score -= queue.weighted_load() * 0.1;
            }
        }

        if let Some((&best_agent, _)) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            return RouteResult::Existing(best_agent);
        }

        // No overlap: spawn by group or fallback
        let group_name = manifest
            .first()
            .and_then(|fa| groups.resolve(&fa.path))
            .map(|g| g.name.clone());
        if let Some(name) = group_name {
            return RouteResult::SpawnAgent(name);
        }
        if config.fallback_to_single_agent {
            Self::least_loaded_or_spawn(agents, config)
        } else {
            RouteResult::SpawnAgent("general".to_string())
        }
    }

    /// Choose least-loaded existing agent or request spawn of "default".
    pub fn least_loaded_or_spawn(
        agents: &HashMap<AgentId, AgentQueue>,
        _config: &OrchestratorConfig,
    ) -> RouteResult {
        if agents.is_empty() {
            return RouteResult::SpawnAgent("default".to_string());
        }
        let least_loaded = agents
            .iter()
            .min_by(|(_, q_a), (_, q_b)| {
                q_a.weighted_load()
                    .partial_cmp(&q_b.weighted_load())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id);
        match least_loaded {
            Some(id) => RouteResult::Existing(id),
            None => RouteResult::SpawnAgent("default".to_string()),
        }
    }
}
