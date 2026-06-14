//! JSON snapshot format for persisting orchestrator queue and context state.
//!
//! [`OrchestratorState`](crate::state::OrchestratorState) is written by tooling that needs warm restarts without
//! replaying the full oplog.

use serde::{Deserialize, Serialize};

use crate::config::OrchestratorConfig;
use crate::orchestrator::OrchestratorStatus;

/// Serializable snapshot of orchestrator state for session persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorState {
    /// Schema version for the state file.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Configuration used when this state was saved.
    pub config: OrchestratorConfig,
    /// Summary of agents and their queue sizes at save time.
    pub agents: Vec<SavedAgentState>,
    /// Total tasks completed in this session.
    pub total_completed: usize,
    /// Timestamp when state was saved (ISO 8601).
    pub saved_at: String,
    /// Dump of current shared context values.
    #[serde(default)]
    pub context_entries: std::collections::HashMap<String, crate::context::ContextEntry>,
    /// Per-plugin persistent state dumps.
    #[serde(default)]
    pub plugin_states: std::collections::HashMap<String, serde_json::Value>,
}

fn default_version() -> u32 {
    1
}

/// Serialized state of a single agent.
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedAgentState {
    /// Raw agent id (`AgentId.0`).
    pub id: u64,
    /// Queue display name.
    pub name: String,
    /// Pending tasks total.
    pub queued_count: usize,
    /// Urgent backlog depth.
    pub urgent_count: usize,
    /// Normal backlog depth.
    pub normal_count: usize,
    /// Background backlog depth.
    pub background_count: usize,
    /// Lifetime completions for this agent.
    pub completed_count: usize,
    /// Operator pause flag.
    pub paused: bool,
}

impl OrchestratorState {
    /// Create a saveable state from the current orchestrator status.
    pub fn from_status(status: &OrchestratorStatus, config: &OrchestratorConfig) -> Self {
        Self {
            version: 1,
            config: config.clone(),
            agents: status
                .agents
                .iter()
                .map(|a| SavedAgentState {
                    id: a.id.0,
                    name: a.name.clone(),
                    queued_count: a.queued,
                    urgent_count: a.urgent_count,
                    normal_count: a.normal_count,
                    background_count: a.background_count,
                    completed_count: a.completed,
                    paused: a.paused,
                })
                .collect(),
            total_completed: status.total_completed,
            saved_at: chrono_iso_now(),
            context_entries: status.context_entries.clone(),
            plugin_states: std::collections::HashMap::new(),
        }
    }

    /// Save state to VoxDb.
    pub async fn save_to_db(&self, db: &vox_db::VoxDb) -> Result<(), StateError> {
        let json = serde_json::to_string_pretty(self).map_err(StateError::Serialize)?;
        db.save_memory(vox_db::SaveMemoryParams {
            agent_id: "global",
            session_id: "global",
            memory_type: "orchestrator_state",
            content: &json,
            metadata: None,
            importance: 1.0,
            vcs_snapshot_id: None,
        })
        .await
        .map_err(|e| StateError::Io(std::io::Error::other(e.to_string())))?;
        tracing::info!("Orchestrator state saved to VoxDb");
        Ok(())
    }

    /// Load state from VoxDb.
    pub async fn load_from_db(db: &vox_db::VoxDb) -> Result<Option<Self>, StateError> {
        let memories = db
            .recall_memory("global", Some("orchestrator_state"), 1, None)
            .await
            .unwrap_or_default();
        if let Some(mem) = memories.into_iter().next() {
            let state: Self =
                serde_json::from_str(&mem.content).map_err(StateError::Deserialize)?;
            tracing::info!("Orchestrator state loaded from VoxDb");
            return Ok(Some(state));
        }
        Ok(None)
    }
}

/// Get the current timestamp in ISO 8601 format (without chrono dependency).
fn chrono_iso_now() -> String {
    // Use a simple timestamp since we don't want to add chrono as a dep
    format!(
        "{:?}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}

/// Errors for state persistence.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// Filesystem failure while reading/writing the snapshot path.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// `OrchestratorState` could not be encoded.
    #[error("Serialization error: {0}")]
    Serialize(serde_json::Error),
    /// On-disk JSON did not match the expected schema.
    #[error("Deserialization error: {0}")]
    Deserialize(serde_json::Error),
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn chrono_iso_now_returns_parseable_positive_epoch_seconds() {
        let s = chrono_iso_now();
        // Despite the "iso" name, the output is the Debug-format of u64 epoch seconds:
        // a plain decimal integer string with no separators or alphabetic characters.
        let secs: u64 = s
            .parse()
            .expect("chrono_iso_now() must return a plain u64 decimal string");
        // The system clock is after the UNIX epoch, so seconds-since-epoch is positive.
        assert!(secs > 0, "epoch seconds should be positive, got {secs}");
        assert!(
            s.chars().all(|c| c.is_ascii_digit()),
            "output should contain only ASCII digits, got {s:?}"
        );
    }
}
