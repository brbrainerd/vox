use std::collections::HashMap;
use std::time::Duration;
// Removed unused Mutex, Arc
use crate::continuation::{ContinuationEngine, ContinuationStrategy};
use crate::events::EventBus;
use crate::types::AgentId;

/// AI Monitor for idle detection and continuation prompts.
#[derive(Debug)]
pub struct AiMonitor {
    engine: ContinuationEngine,
    /// Last known activity timestamp for each agent
    last_activity: HashMap<AgentId, std::time::SystemTime>,
    idle_threshold: Duration,
}

impl AiMonitor {
    pub fn new(cooldown_ms: u64, max_auto_continuations: u32, idle_threshold_ms: u64) -> Self {
        Self {
            engine: ContinuationEngine::new(cooldown_ms, max_auto_continuations),
            last_activity: HashMap::new(),
            idle_threshold: Duration::from_millis(idle_threshold_ms),
        }
    }

    /// Update activity timestamp for an agent
    pub fn record_activity(&mut self, agent_id: AgentId) {
        self.last_activity
            .insert(agent_id, std::time::SystemTime::now());
        self.engine.reset_cooldown(agent_id);
    }

    /// Check for idle agents and return continuation prompts if any.
    pub fn check_idle_agents(
        &mut self,
        active_agents: &[(AgentId, usize)], // (id, pending_task_count)
        event_bus: &EventBus,
    ) -> Vec<(AgentId, String)> {
        let now = std::time::SystemTime::now();
        let mut intents = Vec::new();

        for &(agent_id, pending_tasks) in active_agents {
            if pending_tasks == 0 {
                continue;
            }

            if let Some(last) = self.last_activity.get(&agent_id) {
                if let Ok(elapsed) = now.duration_since(*last) {
                    if elapsed >= self.idle_threshold {
                        let strategy = if self.engine.continuation_count(agent_id) > 2 {
                            ContinuationStrategy::AssessRemaining
                        } else {
                            ContinuationStrategy::Continue
                        };

                        if let Some(prompt) = self.engine.generate_continuation(
                            agent_id,
                            strategy,
                            pending_tasks,
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
