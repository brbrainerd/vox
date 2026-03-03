//! Auto-continuation engine for idle agents.
//!
//! Detects idle agents with pending work and generates continuation
//! prompts. Supports configurable strategies and per-agent cooldowns
//! to prevent spam-continuing.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::events::{AgentEventKind, EventBus};
use crate::types::AgentId;

/// Strategy for auto-continuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationStrategy {
    /// Continue with the current task.
    Continue,
    /// Assess whether there is remaining work.
    AssessRemaining,
    /// Skip to the next task in the queue.
    SkipToNext,
    /// Continue all pending steps.
    ContinueAll,
}

impl std::fmt::Display for ContinuationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Continue => write!(f, "continue"),
            Self::AssessRemaining => write!(f, "assess_remaining"),
            Self::SkipToNext => write!(f, "skip_to_next"),
            Self::ContinueAll => write!(f, "continue_all"),
        }
    }
}

/// A continuation prompt generated for an idle agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationPrompt {
    pub agent_id: AgentId,
    pub strategy: ContinuationStrategy,
    pub prompt_text: String,
    pub generated_at: u64,
}

/// Per-agent cooldown tracking.
#[derive(Debug, Clone)]
struct AgentCooldown {
    last_continuation: Instant,
    continuation_count: u32,
}

/// Auto-continuation engine.
///
/// Watches for idle agents via the heartbeat monitor and generates
/// continuation prompts when appropriate.
#[derive(Debug)]
pub struct ContinuationEngine {
    /// Per-agent cooldown state.
    cooldowns: HashMap<AgentId, AgentCooldown>,
    /// Minimum time between continuations per agent.
    cooldown_duration: Duration,
    /// Maximum continuations before requiring manual intervention.
    max_auto_continuations: u32,
    /// Whether auto-continuation is enabled.
    enabled: bool,
}

impl ContinuationEngine {
    /// Create a new continuation engine.
    pub fn new(cooldown_ms: u64, max_auto_continuations: u32) -> Self {
        Self {
            cooldowns: HashMap::new(),
            cooldown_duration: Duration::from_millis(cooldown_ms),
            max_auto_continuations,
            enabled: true,
        }
    }

    /// Enable or disable auto-continuation.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether auto-continuation is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if an agent can receive a continuation (not in cooldown).
    pub fn can_continue(&self, agent_id: AgentId) -> bool {
        if !self.enabled {
            return false;
        }

        match self.cooldowns.get(&agent_id) {
            Some(cd) => {
                let elapsed = Instant::now().duration_since(cd.last_continuation);
                elapsed >= self.cooldown_duration
                    && cd.continuation_count < self.max_auto_continuations
            }
            None => true,
        }
    }

    /// Generate a continuation prompt for an idle agent.
    ///
    /// Returns `None` if the agent is in cooldown or auto-continuation is disabled.
    pub fn generate_continuation(
        &mut self,
        agent_id: AgentId,
        strategy: ContinuationStrategy,
        pending_task_count: usize,
        event_bus: &EventBus,
    ) -> Option<ContinuationPrompt> {
        if !self.can_continue(agent_id) {
            return None;
        }

        let prompt_text = match strategy {
            ContinuationStrategy::Continue => format!(
                "You have {} pending task(s). Please continue with the current task.",
                pending_task_count
            ),
            ContinuationStrategy::AssessRemaining => {
                "Please assess whether there is any remaining work to be done. \
                 Check the task list and determine if all objectives are met."
                    .to_string()
            }
            ContinuationStrategy::SkipToNext => {
                "The current task appears stalled. Please skip to the next task in your queue."
                    .to_string()
            }
            ContinuationStrategy::ContinueAll => format!(
                "You have {} pending task(s). Please continue with all next steps \
                 until the task list is complete.",
                pending_task_count
            ),
        };

        #[cfg(feature = "runtime")]
        let prompt_text = vox_runtime::prompt_canonical::canonicalize_simple(&prompt_text);

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Update cooldown
        let cd = self.cooldowns.entry(agent_id).or_insert(AgentCooldown {
            last_continuation: Instant::now(),
            continuation_count: 0,
        });
        cd.last_continuation = Instant::now();
        cd.continuation_count += 1;

        // Emit event
        event_bus.emit(AgentEventKind::ContinuationTriggered {
            agent_id,
            strategy: strategy.to_string(),
        });

        Some(ContinuationPrompt {
            agent_id,
            strategy,
            prompt_text,
            generated_at: timestamp_ms,
        })
    }

    /// Reset cooldown for an agent (e.g., after a manual intervention).
    pub fn reset_cooldown(&mut self, agent_id: AgentId) {
        self.cooldowns.remove(&agent_id);
    }

    /// Get the continuation count for an agent.
    pub fn continuation_count(&self, agent_id: AgentId) -> u32 {
        self.cooldowns
            .get(&agent_id)
            .map(|cd| cd.continuation_count)
            .unwrap_or(0)
    }

    /// Check if an agent has exceeded the max auto-continuation limit.
    pub fn is_exhausted(&self, agent_id: AgentId) -> bool {
        self.cooldowns
            .get(&agent_id)
            .map(|cd| cd.continuation_count >= self.max_auto_continuations)
            .unwrap_or(false)
    }
}

impl Default for ContinuationEngine {
    fn default() -> Self {
        Self::new(30_000, 5) // 30s cooldown, max 5 auto-continuations
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_continuation_prompt() {
        let bus = EventBus::new(16);
        let mut engine = ContinuationEngine::new(0, 10); // no cooldown for testing
        let agent = AgentId(1);

        let prompt = engine.generate_continuation(agent, ContinuationStrategy::Continue, 3, &bus);

        assert!(prompt.is_some());
        let p = prompt.unwrap();
        assert_eq!(p.agent_id, agent);
        assert_eq!(p.strategy, ContinuationStrategy::Continue);
        assert!(p.prompt_text.contains("3 pending"));
    }

    #[test]
    fn cooldown_prevents_spam() {
        let bus = EventBus::new(16);
        let mut engine = ContinuationEngine::new(1_000_000, 10); // very long cooldown
        let agent = AgentId(1);

        // First continuation succeeds
        let p1 = engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        assert!(p1.is_some());

        // Second is blocked by cooldown
        let p2 = engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        assert!(p2.is_none());
    }

    #[test]
    fn max_continuations_exhausted() {
        let bus = EventBus::new(16);
        let mut engine = ContinuationEngine::new(0, 2); // max 2
        let agent = AgentId(1);

        engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);

        assert!(engine.is_exhausted(agent));

        let p3 = engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        assert!(p3.is_none());
    }

    #[test]
    fn reset_cooldown_restores() {
        let bus = EventBus::new(16);
        let mut engine = ContinuationEngine::new(0, 2);
        let agent = AgentId(1);

        engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        engine.generate_continuation(agent, ContinuationStrategy::Continue, 1, &bus);
        assert!(engine.is_exhausted(agent));

        engine.reset_cooldown(agent);
        assert!(!engine.is_exhausted(agent));
        assert!(engine.can_continue(agent));
    }

    #[test]
    fn disabled_engine_blocks_all() {
        let bus = EventBus::new(16);
        let mut engine = ContinuationEngine::new(0, 10);
        engine.set_enabled(false);

        let p = engine.generate_continuation(AgentId(1), ContinuationStrategy::Continue, 5, &bus);
        assert!(p.is_none());
    }
}
