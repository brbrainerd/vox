//! Routing service: file-affinity and group-based task routing.
//!
//! Decides which agent (existing or to be spawned) should receive a task
//! based on file manifest, affinity map, affinity groups, and load.

use std::collections::HashMap;

use std::sync::Arc;

use crate::affinity::FileAffinityMap;
use crate::config::OrchestratorConfig;
use crate::contract::TaskCapabilityHints;
use crate::groups::AffinityGroupRegistry;
use crate::populi_federation::RemotePopuliRoutingHint;
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
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        config: &OrchestratorConfig,
        local_tokens: u64,
        agent_reliability: Option<&HashMap<AgentId, f64>>,
        task_capability_requirements: Option<&TaskCapabilityHints>,
        task_description: Option<&str>,
        remote_populi_hints: Option<&[RemotePopuliRoutingHint]>,
        // Multi-dimensional trust rollups keyed by agent id (task_completion dimension).
        task_completion_trust_scores: Option<&HashMap<AgentId, f64>>,
        // Phase 15: prefer agents with higher trust to reduce pilot interrupts.
        attention_trust_scores: Option<&HashMap<AgentId, crate::attention::AgentTrustScore>>,
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
                for (agent_id, queue_lock) in agents {
                    let queue = crate::sync_lock::rw_read(queue_lock);
                    if queue.name == group.name {
                        *scores.entry(*agent_id).or_insert(0.0) += 5.0;
                    }
                }
            }
        }

        // 3. Weight by load (prefer emptier agents on ties)
        for (agent_id, score) in scores.iter_mut() {
            if let Some(queue_lock) = agents.get(agent_id) {
                let queue = crate::sync_lock::rw_read(queue_lock);
                *score -= queue.weighted_load() * 0.1;
            }
        }

        // 3b. GPU / capability fit (penalize agents that cannot satisfy task requirements)
        if let Some(req) = task_capability_requirements {
            Self::apply_capability_penalties(&mut scores, agents, req);
        }

        // 3c. Experimental mens visibility (read-only federation); never routes off-process.
        if config.populi_routing_experimental {
            Self::apply_experimental_populi_routing_signals(
                &mut scores,
                agents,
                task_capability_requirements,
                remote_populi_hints,
            );
        }
        if config.populi_training_routing_experimental {
            if let Some(req) = task_capability_requirements {
                Self::apply_training_task_signals(
                    &mut scores,
                    agents,
                    req,
                    task_description,
                    remote_populi_hints,
                    config.populi_training_budget_pressure,
                );
            }
        }

        // 3d. Hardware breakeven: if local token budget exceeded, severely penalize non-local agents.
        if local_tokens > config.local_breakeven_tokens {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if q.capabilities.routing_tier.as_deref() != Some("local") {
                        *score -= 50_000.0;
                    }
                }
            }
        }

        // 3e. Repo shard workflow specialization and reliability penalties.
        Self::apply_repo_shard_phase_signals(&mut scores, agents, config, task_description);

        // 3e. Dimension-specific trust floor and utility blend.
        if let Some(trust_scores) = task_completion_trust_scores {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(ts) = trust_scores.get(agent_id) {
                    if *ts < config.trust_task_completion_floor {
                        // Hard floor: keep the agent selectable only as last resort.
                        *score -= 10_000.0;
                    }
                    *score += ts * config.trust_task_completion_weight;
                }
            }
        }

        // 3f. Attention-aware routing: UCB exploration (Task 61) replaces greedy trust selection.
        //     New/uncertain agents get exploration bonus proportional to sqrt(variance).
        if config.attention_enabled {
            if rand::random::<f64>() < config.routing_exploration_epsilon {
                let eligible: Vec<_> = agents
                    .keys()
                    .filter(|k| scores.get(k).copied().unwrap_or(0.0) >= -1000.0)
                    .copied()
                    .collect();
                if !eligible.is_empty() {
                    use rand::Rng;
                    let idx = rand::thread_rng().gen_range(0..eligible.len());
                    return RouteResult::Existing(eligible[idx]);
                }
            }

            if let Some(trust_map) = attention_trust_scores {
                let c = config.attention_trust_routing_weight;
                for (agent_id, score) in scores.iter_mut() {
                    if let Some(ts) = trust_map.get(agent_id) {
                        *score += ts.ucb_score(c);
                    }
                }
            }
        }

        // 4. Optional reliability blend (Arca `agent_reliability`, schema V10)
        let rep_w = config.socrates_reputation_weight;
        if config.socrates_reputation_routing {
            for (agent_id, score) in scores.iter_mut() {
                let mut agent_base = 0.5;
                if let Some(rel) = agent_reliability {
                    if let Some(r) = rel.get(agent_id) {
                        agent_base = *r;
                    }
                }
                *score += agent_base * rep_w;

                // Blend in skill & workflow EWMA scores if present
                if let Some(queue_lock) = agents.get(agent_id) {
                    let queue = crate::sync_lock::rw_read(queue_lock);
                    let mut skill_rel_sum = 0.0;
                    let mut skill_count = 0;
                    for rel in queue.active_skills.values() {
                        skill_rel_sum += *rel;
                        skill_count += 1;
                    }
                    if skill_count > 0 {
                        // Blend average skill reliability at 50% reputation weight
                        *score += (skill_rel_sum / skill_count as f64) * rep_w * 0.5;
                    }

                    // Small contextual boost if this agent is dedicated to a workflow
                    if queue.workflow_context.is_some() {
                        *score += rep_w * 0.1;
                    }
                }
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

    fn apply_capability_penalties(
        scores: &mut HashMap<AgentId, f64>,
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        req: &TaskCapabilityHints,
    ) {
        const PENALTY: f64 = 10_000.0;
        if req.gpu_cuda {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.gpu_cuda {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.gpu_metal {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.gpu_metal {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.gpu_vulkan {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.gpu_vulkan {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.gpu_webgpu {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.gpu_webgpu {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.npu {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.npu {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.visus_eligible {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.visus_eligible {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if req.multi_modal {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    if !q.capabilities.multi_modal {
                        *score -= PENALTY;
                    }
                }
            }
        }
        if let Some(min_v) = req.min_vram_mb {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    let ok = q
                        .capabilities
                        .min_vram_mb
                        .map(|v| v >= min_v)
                        .unwrap_or(false);
                    if !ok {
                        *score -= PENALTY * 0.5;
                    }
                }
            }
        }
        if let Some(min_c) = req.min_cpu_cores {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    let have = q.capabilities.cpu_cores.unwrap_or(0);
                    if have < min_c {
                        *score -= PENALTY * 0.5;
                    }
                }
            }
        }
        if req.prefer_gpu_compute {
            for (agent_id, score) in scores.iter_mut() {
                if let Some(q_lock) = agents.get(agent_id) {
                    let q = crate::sync_lock::rw_read(q_lock);
                    let has_gpu = q.capabilities.gpu_cuda
                        || q.capabilities.gpu_metal
                        || q.capabilities.gpu_vulkan
                        || q.capabilities.gpu_webgpu
                        || q.capabilities.npu;
                    if !has_gpu {
                        *score -= PENALTY * 0.15;
                    }
                }
            }
        }
    }

    fn labels_cover(have: &[String], need: &[String]) -> bool {
        need.iter()
            .all(|n| have.iter().any(|h| h.as_str() == n.as_str()))
    }

    fn remote_hint_matches_task(r: &RemotePopuliRoutingHint, req: &TaskCapabilityHints) -> bool {
        if !r.is_federation_schedulable() {
            return false;
        }
        if req.labels.is_empty() {
            return false;
        }
        Self::labels_cover(&r.labels, &req.labels)
            && (!req.gpu_cuda || r.gpu_cuda)
            && (!req.gpu_metal || r.gpu_metal)
            && (!req.visus_eligible || r.capabilities.visus_eligible)
            && (!req.multi_modal || r.capabilities.multi_modal)
            && (!(req.gpu_cuda || req.gpu_metal || req.prefer_gpu_compute)
                || r.is_federation_gpu_eligible())
            && match req.min_vram_mb {
                None => true,
                Some(need) => r.min_vram_mb.is_some_and(|have| have >= need),
            }
    }

    /// Soft score bump + tracing when cached remote mens nodes align with task labels (no remote execute).
    fn apply_experimental_populi_routing_signals(
        scores: &mut HashMap<AgentId, f64>,
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        task_capability_requirements: Option<&TaskCapabilityHints>,
        remote_populi_hints: Option<&[RemotePopuliRoutingHint]>,
    ) {
        const LABEL_BUMP: f64 = 0.25;
        let Some(req) = task_capability_requirements else {
            return;
        };
        let Some(remote) = remote_populi_hints.filter(|s| !s.is_empty()) else {
            return;
        };
        let remote_schedulable = remote
            .iter()
            .filter(|r| r.is_federation_schedulable())
            .count();
        if remote_schedulable == 0 {
            return;
        }
        if !req.labels.is_empty() {
            let local_matches = agents.values().any(|q_lock| {
                Self::labels_cover(
                    &crate::sync_lock::rw_read(q_lock).capabilities.labels,
                    &req.labels,
                )
            });
            let remote_candidates = remote
                .iter()
                .filter(|r| Self::remote_hint_matches_task(r, req))
                .count();
            if !local_matches && remote_candidates > 0 {
                tracing::info!(
                    target: "vox.orchestrator.routing",
                    decision = "remote_label_match_only",
                    remote_candidates,
                    "populi_routing_experimental: no local agent matches task labels; mens lists remote candidates (no remote execute)"
                );
            } else if local_matches && remote_candidates > 0 {
                tracing::debug!(
                    target: "vox.orchestrator.routing",
                    decision = "local_and_remote_label_match",
                    remote_candidates,
                    "populi_routing_experimental: preferring local placement"
                );
            }
            if local_matches {
                for (agent_id, score) in scores.iter_mut() {
                    if let Some(q_lock) = agents.get(agent_id) {
                        if Self::labels_cover(
                            &crate::sync_lock::rw_read(q_lock).capabilities.labels,
                            &req.labels,
                        ) {
                            *score += LABEL_BUMP;
                        }
                    }
                }
            }
        }
        if req.prefer_gpu_compute || req.gpu_cuda || req.gpu_metal {
            let remote_gpu = remote
                .iter()
                .filter(|r| r.is_federation_schedulable() && r.is_federation_gpu_eligible())
                .count();
            if remote_gpu > 0 {
                tracing::trace!(
                    target: "vox.orchestrator.routing",
                    remote_gpu_candidates = remote_gpu,
                    "mens federation GPU visibility (experimental)"
                );
            }
        }
    }

    fn is_training_task(req: &TaskCapabilityHints) -> bool {
        req.labels.iter().any(|l| {
            let lower = l.to_ascii_lowercase();
            lower == "workload=mens-train"
                || lower == "workload=train"
                || lower.starts_with("pool=train")
        })
    }

    fn apply_training_task_signals(
        scores: &mut HashMap<AgentId, f64>,
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        req: &TaskCapabilityHints,
        task_description: Option<&str>,
        remote_populi_hints: Option<&[RemotePopuliRoutingHint]>,
        budget_pressure: f64,
    ) {
        let description_training = task_description
            .map(|d| d.to_ascii_lowercase().contains("train"))
            .unwrap_or(false);
        if !Self::is_training_task(req) && !description_training {
            return;
        }
        let budget_pressure = budget_pressure.clamp(0.0, 1.0);
        let mut boosted_gpu_agents: usize = 0;
        let mut boosted_vram_agents: usize = 0;
        let mut penalized_vram_agents: usize = 0;
        let mut label_matched_agents: usize = 0;
        for (agent_id, score) in scores.iter_mut() {
            let Some(q_lock) = agents.get(agent_id) else {
                continue;
            };
            let q = crate::sync_lock::rw_read(q_lock);
            let has_gpu = q.capabilities.gpu_cuda
                || q.capabilities.gpu_metal
                || q.capabilities.gpu_vulkan
                || q.capabilities.gpu_webgpu
                || q.capabilities.npu;
            if has_gpu {
                *score += 0.75;
                boosted_gpu_agents += 1;
            }
            if let Some(req_vram) = req.min_vram_mb {
                let vram_ok = q
                    .capabilities
                    .min_vram_mb
                    .is_some_and(|have| have >= req_vram);
                if vram_ok {
                    *score += 0.75;
                    boosted_vram_agents += 1;
                } else {
                    *score -= 0.5;
                    penalized_vram_agents += 1;
                }
                if req_vram >= 12_288 && budget_pressure > 0.0 {
                    *score -= budget_pressure * 2.0;
                }
            }
            let label_match =
                req.labels.is_empty() || Self::labels_cover(&q.capabilities.labels, &req.labels);
            if label_match {
                *score += 0.25;
                label_matched_agents += 1;
            }
        }
        tracing::debug!(
            target: "vox.orchestrator.routing",
            boosted_gpu_agents,
            boosted_vram_agents,
            penalized_vram_agents,
            label_matched_agents,
            budget_pressure,
            "training routing score signals applied"
        );

        if let Some(remote) = remote_populi_hints.filter(|h| !h.is_empty()) {
            let remote_train_gpu = remote
                .iter()
                .filter(|h| {
                    h.is_federation_schedulable()
                        && h.training_labels.iter().any(|l| l.starts_with("workload="))
                        && (h.gpu_cuda || h.gpu_metal)
                        && match req.min_vram_mb {
                            None => true,
                            Some(need) => h.min_vram_mb.is_some_and(|have| have >= need),
                        }
                })
                .count();
            if remote_train_gpu > 0 {
                tracing::info!(
                    target: "vox.orchestrator.routing",
                    remote_training_candidates = remote_train_gpu,
                    budget_pressure,
                    "training routing signal: remote mens nodes advertise matching training capabilities (local placement retained)"
                );
            }
        }
    }

    fn apply_repo_shard_phase_signals(
        scores: &mut HashMap<AgentId, f64>,
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        config: &OrchestratorConfig,
        task_description: Option<&str>,
    ) {
        let phase = task_description
            .map(str::to_ascii_uppercase)
            .unwrap_or_default();
        let is_shard_gen = phase.contains("[PHASE:SHARD_GEN]");
        let is_shard_validate = phase.contains("[PHASE:SHARD_VALIDATE]");
        let is_reduce = phase.contains("[PHASE:REDUCE]");
        let now_ms = crate::types::now_unix_ms();

        for (agent_id, score) in scores.iter_mut() {
            if let Some(queue_lock) = agents.get(agent_id) {
                let queue = crate::sync_lock::rw_read(queue_lock);

                if is_shard_gen && let Some(rel) = queue.active_skills.get("shard_gen") {
                    *score += rel * config.repo_shard_specialization_weight;
                }
                if is_shard_validate && let Some(rel) = queue.active_skills.get("shard_validate") {
                    *score += rel * config.repo_shard_specialization_weight;
                }
                if is_reduce && let Some(rel) = queue.active_skills.get("reduce") {
                    *score += rel * config.repo_shard_specialization_weight;
                }

                if is_shard_validate && queue.recent_shard_validation_failures > 0 {
                    *score -= queue.recent_shard_validation_failures as f64
                        * config.repo_shard_validation_failure_penalty;
                }

                if is_reduce
                    && let Some(cooldown_until) = queue.reducer_cooldown_until_ms
                    && cooldown_until > now_ms
                {
                    *score -= config.repo_reduce_conflict_cooldown_penalty;
                }
            }
        }
    }

    /// Choose least-loaded existing agent or request spawn of "default".
    pub fn least_loaded_or_spawn(
        agents: &HashMap<AgentId, Arc<std::sync::RwLock<AgentQueue>>>,
        _config: &OrchestratorConfig,
    ) -> RouteResult {
        if agents.is_empty() {
            return RouteResult::SpawnAgent("default".to_string());
        }
        let least_loaded = agents
            .iter()
            .min_by(|(_, q_a_lock), (_, q_b_lock)| {
                let q_a = crate::sync_lock::rw_read(q_a_lock);
                let q_b = crate::sync_lock::rw_read(q_b_lock);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::AffinityGroup;
    use crate::populi_federation::RemotePopuliRoutingHint;
    use crate::types::{TaskId, TaskPriority};

    #[test]
    fn reliability_blend_prefers_higher_reliability_when_enabled() {
        let manifest = vec![FileAffinity::write("src/lib.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "core-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);

        let mut agents = HashMap::new();
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        agents.insert(
            a1,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a1, "core-group"))),
        );
        agents.insert(
            a2,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a2, "core-group"))),
        );

        let mut config = OrchestratorConfig::for_testing();
        config.socrates_reputation_routing = true;
        config.routing_exploration_epsilon = 0.0;

        let mut rel = HashMap::new();
        rel.insert(a1, 0.15);
        rel.insert(a2, 0.95);

        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &config,
            0,
            Some(&rel),
            None,
            None,
            None,
            None,
            None, // attention_trust_scores
        );
        assert_eq!(route, RouteResult::Existing(a2));
    }

    #[test]
    fn least_loaded_fallback_prefers_lower_weighted_queue() {
        let mut agents = HashMap::new();
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        let mut q1 = AgentQueue::new(a1, "one");
        let mut q2 = AgentQueue::new(a2, "two");
        q1.enqueue(crate::types::AgentTask::new(
            TaskId(1),
            "urgent",
            TaskPriority::Urgent,
            vec![],
        ));
        q2.enqueue(crate::types::AgentTask::new(
            TaskId(2),
            "normal",
            TaskPriority::Normal,
            vec![],
        ));
        agents.insert(a1, Arc::new(std::sync::RwLock::new(q1)));
        agents.insert(a2, Arc::new(std::sync::RwLock::new(q2)));

        let route = RoutingService::least_loaded_or_spawn(&agents, &OrchestratorConfig::default());
        assert_eq!(route, RouteResult::Existing(a2));
    }

    #[test]
    fn prefer_gpu_compute_soft_penalty_favors_gpu_agent() {
        let manifest = vec![FileAffinity::write("src/lib.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "core-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);

        let mut agents = HashMap::new();
        let cpu = AgentId(1);
        let gpu = AgentId(2);
        let mut q_cpu = AgentQueue::new(cpu, "core-group");
        q_cpu.capabilities.gpu_cuda = false;
        let mut q_gpu = AgentQueue::new(gpu, "core-group");
        q_gpu.capabilities.gpu_cuda = true;
        agents.insert(cpu, Arc::new(std::sync::RwLock::new(q_cpu)));
        agents.insert(gpu, Arc::new(std::sync::RwLock::new(q_gpu)));

        let hints = TaskCapabilityHints {
            prefer_gpu_compute: true,
            ..Default::default()
        };
        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &OrchestratorConfig::default(),
            0,
            None,
            Some(&hints),
            None,
            None,
            None,
            None, // attention_trust_scores
        );
        assert_eq!(route, RouteResult::Existing(gpu));
    }

    #[test]
    fn experimental_mesh_bumps_local_when_labels_match() {
        let manifest = vec![FileAffinity::write("src/lib.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "core-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);

        let mut agents = HashMap::new();
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        let mut q1 = AgentQueue::new(a1, "core-group");
        q1.capabilities.labels = vec!["pool=a".to_string()];
        let mut q2 = AgentQueue::new(a2, "core-group");
        q2.capabilities.labels = vec!["pool=b".to_string()];
        agents.insert(a1, Arc::new(std::sync::RwLock::new(q1)));
        agents.insert(a2, Arc::new(std::sync::RwLock::new(q2)));

        let mut config = OrchestratorConfig::for_testing();
        config.populi_routing_experimental = true;
        // Deterministic winner: default `attention_enabled` uses epsilon-random exploration.
        config.attention_enabled = false;

        let hints = TaskCapabilityHints {
            labels: vec!["pool=a".to_string()],
            ..Default::default()
        };
        let remote = vec![RemotePopuliRoutingHint {
            node_id: "remote-1".into(),
            capabilities: TaskCapabilityHints {
                labels: vec!["pool=a".to_string()],
                ..Default::default()
            },
            labels: vec!["pool=a".to_string()],
            gpu_cuda: false,
            gpu_metal: false,
            min_vram_mb: None,
            gpu_total_count: None,
            gpu_healthy_count: None,
            gpu_allocatable_count: None,
            gpu_inventory_source: None,
            gpu_truth_layer: None,
            training_labels: vec![],
            maintenance: false,
            quarantined: false,
            heartbeat_stale: false,
            nvidia_driver_version: None,
            cuda_driver_version: None,
            gpu_readiness_ok: None,
            gpu_readiness_reason: None,
            gpu_readiness_checked_unix_ms: None,
        }];
        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &config,
            0,
            None,
            Some(&hints),
            None,
            Some(remote.as_slice()),
            None,
            None, // attention_trust_scores
        );
        assert_eq!(route, RouteResult::Existing(a1));
    }

    #[test]
    fn remote_hint_matching_ignores_quarantined_maintenance_or_stale_nodes() {
        let req = TaskCapabilityHints {
            labels: vec!["pool=a".to_string()],
            gpu_cuda: true,
            min_vram_mb: Some(12_288),
            ..Default::default()
        };
        let mut quarantined = RemotePopuliRoutingHint {
            node_id: "remote-q".into(),
            capabilities: TaskCapabilityHints {
                labels: vec!["pool=a".into()],
                gpu_cuda: true,
                min_vram_mb: Some(24_576),
                ..Default::default()
            },
            labels: vec!["pool=a".into()],
            gpu_cuda: true,
            gpu_metal: false,
            min_vram_mb: Some(24_576),
            gpu_total_count: None,
            gpu_healthy_count: None,
            gpu_allocatable_count: None,
            gpu_inventory_source: None,
            gpu_truth_layer: None,
            training_labels: vec!["workload=mens-train".into()],
            maintenance: false,
            quarantined: true,
            heartbeat_stale: false,
            nvidia_driver_version: None,
            cuda_driver_version: None,
            gpu_readiness_ok: None,
            gpu_readiness_reason: None,
            gpu_readiness_checked_unix_ms: None,
        };
        assert!(!RoutingService::remote_hint_matches_task(
            &quarantined,
            &req
        ));

        quarantined.quarantined = false;
        quarantined.maintenance = true;
        assert!(!RoutingService::remote_hint_matches_task(
            &quarantined,
            &req
        ));

        quarantined.maintenance = false;
        assert!(RoutingService::remote_hint_matches_task(&quarantined, &req));

        quarantined.heartbeat_stale = true;
        assert!(!RoutingService::remote_hint_matches_task(
            &quarantined,
            &req
        ));
    }

    #[test]
    fn remote_hint_matching_requires_allocatable_or_healthy_gpu_for_gpu_tasks() {
        let req = TaskCapabilityHints {
            labels: vec!["pool=a".to_string()],
            gpu_cuda: true,
            ..Default::default()
        };
        let mut hint = RemotePopuliRoutingHint {
            node_id: "remote-gpu".into(),
            capabilities: TaskCapabilityHints {
                labels: vec!["pool=a".into()],
                gpu_cuda: true,
                ..Default::default()
            },
            labels: vec!["pool=a".into()],
            gpu_cuda: true,
            gpu_metal: false,
            min_vram_mb: Some(8_192),
            gpu_total_count: Some(2),
            gpu_healthy_count: Some(0),
            gpu_allocatable_count: Some(0),
            gpu_inventory_source: Some("probed".into()),
            gpu_truth_layer: Some("layer_b_allocatable".into()),
            training_labels: vec![],
            maintenance: false,
            quarantined: false,
            heartbeat_stale: false,
            nvidia_driver_version: None,
            cuda_driver_version: None,
            gpu_readiness_ok: None,
            gpu_readiness_reason: None,
            gpu_readiness_checked_unix_ms: None,
        };
        assert!(!RoutingService::remote_hint_matches_task(&hint, &req));
        hint.gpu_healthy_count = Some(2);
        hint.gpu_allocatable_count = Some(1);
        assert!(RoutingService::remote_hint_matches_task(&hint, &req));
        hint.gpu_readiness_ok = Some(false);
        assert!(!RoutingService::remote_hint_matches_task(&hint, &req));
    }

    #[test]
    fn training_routing_prefers_agent_with_vram_and_gpu() {
        let manifest = vec![FileAffinity::write("src/train.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "train-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);

        let mut agents = HashMap::new();
        let low = AgentId(11);
        let high = AgentId(22);
        let mut q_low = AgentQueue::new(low, "train-group");
        q_low.capabilities.gpu_cuda = true;
        q_low.capabilities.min_vram_mb = Some(8_192);
        q_low.capabilities.labels = vec!["workload=mens-train".into()];
        let mut q_high = AgentQueue::new(high, "train-group");
        q_high.capabilities.gpu_cuda = true;
        q_high.capabilities.min_vram_mb = Some(24_576);
        q_high.capabilities.labels = vec!["workload=mens-train".into()];
        agents.insert(low, Arc::new(std::sync::RwLock::new(q_low)));
        agents.insert(high, Arc::new(std::sync::RwLock::new(q_high)));

        let mut config = OrchestratorConfig::for_testing();
        config.populi_training_routing_experimental = true;
        config.populi_training_budget_pressure = 0.0;

        let req = TaskCapabilityHints {
            gpu_cuda: true,
            min_vram_mb: Some(16_384),
            prefer_gpu_compute: true,
            labels: vec!["workload=mens-train".into()],
            ..Default::default()
        };
        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &config,
            0,
            None,
            Some(&req),
            None,
            None,
            None,
            None,
        );
        assert_eq!(route, RouteResult::Existing(high));
    }

    #[test]
    fn attention_trust_routing_prefers_higher_trust_when_enabled() {
        let manifest = vec![FileAffinity::write("src/lib.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "core-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);
        let mut agents = HashMap::new();
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        agents.insert(
            a1,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a1, "core-group"))),
        );
        agents.insert(
            a2,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a2, "core-group"))),
        );

        let mut config = OrchestratorConfig::for_testing();
        config.attention_enabled = true;
        // Keep weight moderate so UCB trust + exploration does not clamp both agents to the
        // same ceiling (would make `max_by` tie-break on HashMap order).
        config.attention_trust_routing_weight = 2.0;
        config.routing_exploration_epsilon = 0.0;

        let mut trust = HashMap::new();
        trust.insert(
            a1,
            crate::attention::AgentTrustScore {
                agent_id: a1,
                trust_score: 0.1,
                tier: crate::attention::TrustTier::Trusted,
                total_outcomes: 20,
                successful_outcomes: 2,
                below_tier_streak: 0,
                last_updated_ms: 0,
                variance: 0.10,
                is_override: false,
                outcome_window: Default::default(),
            },
        );
        trust.insert(
            a2,
            crate::attention::AgentTrustScore {
                agent_id: a2,
                trust_score: 0.95,
                tier: crate::attention::TrustTier::Trusted,
                total_outcomes: 20,
                successful_outcomes: 19,
                below_tier_streak: 0,
                last_updated_ms: 0,
                variance: 0.05,
                is_override: false,
                outcome_window: Default::default(),
            },
        );

        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &config,
            0,
            None,
            None,
            None,
            None,
            None,
            Some(&trust),
        );
        assert_eq!(route, RouteResult::Existing(a2));
    }

    #[test]
    fn task_completion_trust_floor_disqualifies_low_score_agents() {
        let manifest = vec![FileAffinity::write("src/lib.rs")];
        let affinity = FileAffinityMap::new();
        let groups = AffinityGroupRegistry::new(vec![AffinityGroup {
            name: "core-group".to_string(),
            patterns: vec!["**/src/**".to_string()],
            default_agent: None,
        }]);
        let mut agents = HashMap::new();
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        agents.insert(
            a1,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a1, "core-group"))),
        );
        agents.insert(
            a2,
            Arc::new(std::sync::RwLock::new(AgentQueue::new(a2, "core-group"))),
        );

        let mut config = OrchestratorConfig::for_testing();
        config.trust_task_completion_floor = 0.4;
        config.trust_task_completion_weight = 2.0;

        let mut trust_scores = HashMap::new();
        trust_scores.insert(a1, 0.10);
        trust_scores.insert(a2, 0.80);

        let route = RoutingService::route(
            &manifest,
            &affinity,
            &groups,
            &agents,
            &config,
            0,
            None,
            None,
            None,
            None,
            Some(&trust_scores),
            None,
        );
        assert_eq!(route, RouteResult::Existing(a2));
    }
}
