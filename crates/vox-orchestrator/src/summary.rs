//! Context summarization logic.
//!
//! > **NOTE: This module is used only for metrics/observation.**
//! > Vox handles context compaction natively via the `SummaryManager`.
//! > This module should not be used for actual agent task memory.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::types::AgentId;

/// A single interaction within a context window.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Interaction {
    pub prompt: String,
    pub response: String,
    pub token_count: usize,
}

/// A progressively summarized chain of agent context.
#[derive(Debug, Clone)]
pub struct SummaryChain {
    pub agent_id: AgentId,
    pub interactions: Vec<Interaction>,
    pub compressed_summary: String,
}

impl SummaryChain {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            interactions: Vec::new(),
            compressed_summary: String::new(),
        }
    }

    /// Add a new interaction to the chain.
    pub fn add_interaction(&mut self, prompt: String, response: String, token_count: usize) {
        self.interactions.push(Interaction {
            prompt,
            response,
            token_count,
        });
    }

    /// Generate the current summary context.
    pub fn get_summary(&self) -> String {
        let mut buf = String::new();
        if !self.compressed_summary.is_empty() {
            buf.push_str("Summary of previous interactions:\n");
            buf.push_str(&self.compressed_summary);
            buf.push_str("\n\n---\n\n");
        }

        if !self.interactions.is_empty() {
            buf.push_str("Recent Interactions:\n");
            for (idx, interaction) in self.interactions.iter().enumerate() {
                buf.push_str(&format!(
                    "Q[{}]: {}\nA[{}]: {}\n",
                    idx, interaction.prompt, idx, interaction.response
                ));
            }
        }

        if buf.is_empty() {
            "No prior context.".to_string()
        } else {
            buf
        }
    }
}

/// Manager tracking summary chains for all agents globally.
#[derive(Debug, Clone, Default)]
pub struct SummaryManager {
    inner: Arc<RwLock<HashMap<AgentId, SummaryChain>>>,
}

impl SummaryManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add an interaction for an agent.
    pub fn add_interaction(
        &self,
        agent_id: AgentId,
        prompt: String,
        response: String,
        token_count: usize,
    ) {
        let mut map = self.inner.write().expect("summary lock poisoned");
        let chain = map
            .entry(agent_id)
            .or_insert_with(|| SummaryChain::new(agent_id));
        chain.add_interaction(prompt, response, token_count);
    }

    /// Retrieve the current summarized context string for an agent.
    pub fn get_summary(&self, agent_id: AgentId) -> String {
        let map = self.inner.read().expect("summary lock poisoned");
        if let Some(chain) = map.get(&agent_id) {
            chain.get_summary()
        } else {
            "No prior context.".to_string()
        }
    }

    /// Hands off compressed context from one agent to another.
    pub fn handoff(&self, from_agent: AgentId, to_agent: AgentId) {
        let mut map = self.inner.write().expect("summary lock poisoned");
        // We cannot borrow two items mutably natively without split or removing,
        // so we remove the source or clone it. Let's clone the summary string.
        let summary = if let Some(chain) = map.get(&from_agent) {
            chain.get_summary()
        } else {
            "No prior context.".to_string()
        };

        let target_chain = map
            .entry(to_agent)
            .or_insert_with(|| SummaryChain::new(to_agent));
        // We set the target's compressed summary to the source's full context.
        target_chain.compressed_summary = summary;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve_summary() {
        let sm = SummaryManager::new();
        let agent = AgentId(1);
        sm.add_interaction(agent, "Hello".into(), "Hi".into(), 10);
        let text = sm.get_summary(agent);
        assert!(text.contains("Q[0]: Hello"));
        assert!(text.contains("A[0]: Hi"));
    }

    #[test]
    fn handoff_between_agents() {
        let sm = SummaryManager::new();
        sm.add_interaction(AgentId(1), "Look at parser".into(), "Done".into(), 20);

        // Handoff to agent 2
        sm.handoff(AgentId(1), AgentId(2));

        let text = sm.get_summary(AgentId(2));
        assert!(text.contains("Summary of previous interactions"));
        assert!(text.contains("Look at parser"));
    }
}
