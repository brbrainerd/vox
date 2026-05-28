//! Bridges heartbeats to [`ContinuationEngine`] for idle-agent nudges.
//!
//! [`AiMonitor`] tracks last activity timestamps and asks the continuation
//! layer to emit prompts when agents sit idle past a threshold.

use std::collections::HashMap;
use std::time::Duration;
// Removed unused Mutex, Arc
use crate::continuation::{ContinuationEngine, ContinuationStrategy};
use crate::events::EventBus;
use crate::types::AgentId;

/// AI Monitor for idle detection and continuation prompts.
pub struct AiMonitor {
    engine: ContinuationEngine,
    /// Last known activity timestamp for each agent
    last_activity: HashMap<AgentId, std::time::SystemTime>,
    idle_threshold: Duration,
    /// Registry for looking up procedural skill instructions.
    skill_registry: std::sync::Arc<vox_skills::SkillRegistry>,
}

impl std::fmt::Debug for AiMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiMonitor")
            .field("engine", &self.engine)
            .field("last_activity", &self.last_activity)
            .field("idle_threshold", &self.idle_threshold)
            .finish()
    }
}

impl AiMonitor {
    /// Configures continuation cooldown, max auto-nudges per agent, and idle detection window.
    pub fn new(
        cooldown_ms: u64,
        max_auto_continuations: u32,
        idle_threshold_ms: u64,
        skill_registry: std::sync::Arc<vox_skills::SkillRegistry>,
    ) -> Self {
        Self {
            engine: ContinuationEngine::new(cooldown_ms, max_auto_continuations),
            last_activity: HashMap::new(),
            idle_threshold: Duration::from_millis(idle_threshold_ms),
            skill_registry,
        }
    }

    /// Update activity timestamp for an agent
    pub fn record_activity(&mut self, agent_id: AgentId) {
        self.last_activity
            .insert(agent_id, std::time::SystemTime::now());
    }

    /// Explicitly reset continuation cooldown when real forward progress is observed.
    pub fn record_progress(&mut self, agent_id: AgentId) {
        self.last_activity
            .insert(agent_id, std::time::SystemTime::now());
        self.engine.reset_cooldown(agent_id);
    }

    /// Check for idle agents and return continuation prompts if any.
    pub fn check_idle_agents(
        &mut self,
        active_agents: &[(AgentId, usize, bool, u64, Option<String>)], // (id, pending_task_count, has_in_progress, stalled_in_progress_ms, active_skill)
        event_bus: &EventBus,
    ) -> Vec<(AgentId, String)> {
        let now = std::time::SystemTime::now();
        let mut intents = Vec::new();

        for &(agent_id, pending_tasks, has_in_progress, stalled_in_progress_ms, ref active_skill_id) in
            active_agents
        {
            if pending_tasks == 0 {
                continue;
            }

            if let Some(last) = self.last_activity.get(&agent_id) {
                if let Ok(elapsed) = now.duration_since(*last) {
                    let observed_elapsed = if has_in_progress && stalled_in_progress_ms > 0 {
                        elapsed.max(Duration::from_millis(stalled_in_progress_ms))
                    } else {
                        elapsed
                    };
                    if observed_elapsed >= self.idle_threshold {
                        let strategy = if (has_in_progress && stalled_in_progress_ms > 0)
                            || self.engine.continuation_count(agent_id) > 2
                        {
                            ContinuationStrategy::AssessRemaining
                        } else {
                            ContinuationStrategy::Continue
                        };

                        let active_skill_content = active_skill_id.as_ref().and_then(|id| {
                            self.skill_registry
                                .lookup(id)
                                .ok()
                                .map(|skill| skill.body)
                        });

                        if let Some(prompt) = self.engine.generate_continuation(
                            agent_id,
                            strategy,
                            pending_tasks,
                            observed_elapsed.as_secs(),
                            active_skill_id.as_deref(),
                            active_skill_content.as_deref(),
                            event_bus,
                        ) {
                            intents.push((agent_id, prompt.prompt_text));
                        }
                    }
                }
            } else {
                self.record_activity(agent_id);
            }
        }
        intents
    }
}
