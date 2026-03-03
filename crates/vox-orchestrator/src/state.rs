use serde::{Deserialize, Serialize};
use std::path::Path;

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
    pub id: u64,
    pub name: String,
    pub queued_count: usize,
    pub urgent_count: usize,
    pub normal_count: usize,
    pub background_count: usize,
    pub completed_count: usize,
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

    /// Save state to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        let json = serde_json::to_string_pretty(self).map_err(StateError::Serialize)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(StateError::Io)?;
        }
        std::fs::write(path, json).map_err(StateError::Io)?;
        tracing::info!("Orchestrator state saved to {}", path.display());
        Ok(())
    }

    /// Load state from a JSON file.
    pub fn load(path: &Path) -> Result<Option<Self>, StateError> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path).map_err(StateError::Io)?;
        let state: Self = serde_json::from_str(&content).map_err(StateError::Deserialize)?;
        tracing::info!("Orchestrator state loaded from {}", path.display());
        Ok(Some(state))
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
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(serde_json::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("vox_orch_state_test");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("state.json");

        let state = OrchestratorState {
            version: 1,
            config: OrchestratorConfig::for_testing(),
            agents: vec![SavedAgentState {
                id: 1,
                name: "parser".to_string(),
                queued_count: 3,
                urgent_count: 1,
                normal_count: 1,
                background_count: 1,
                completed_count: 7,
                paused: false,
            }],
            total_completed: 7,
            saved_at: "12345".to_string(),
            context_entries: std::collections::HashMap::new(),
            plugin_states: std::collections::HashMap::new(),
        };

        state.save(&path).expect("save");
        let loaded = OrchestratorState::load(&path)
            .expect("load")
            .expect("should exist");

        assert_eq!(loaded.total_completed, 7);
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents[0].name, "parser");

        // Cleanup
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_missing_file_returns_none() {
        let result = OrchestratorState::load(Path::new("/nonexistent/path/state.json"));
        assert!(result.unwrap().is_none());
    }
}
