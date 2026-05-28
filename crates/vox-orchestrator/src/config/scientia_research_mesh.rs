//! SCIENTIA research mesh intake (orchestrator broadcast → on-disk JSON → promoted ledger).

use serde::{Deserialize, Serialize};

use super::defaults::default_false;
use super::orchestrator_fields::OrchestratorConfig;

fn default_intake_consumer_poll_interval_ms() -> u64 {
    30_000
}

/// Tunables for SCIENTIA [`vox_publisher::research_mesh`] intake and its optional consumer loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScientiaResearchMeshConfig {
    /// When true, mesh subscriber may write intake JSON under `.vox/scientia/research-mesh-intake/`.
    /// Independent of social/news syndication; see also [`crate::config::OrchestratorConfig::research_mesh_intake_writer_active`].
    #[serde(default = "default_false")]
    pub intake_writer_enabled: bool,
    /// Background poll that promotes pending intake files into the promoted JSONL ledger.
    #[serde(default = "default_false")]
    pub intake_consumer_poll_enabled: bool,
    /// Interval between consumer ticks (milliseconds). Clamped to ≥ 1000 at spawn sites.
    #[serde(default = "default_intake_consumer_poll_interval_ms")]
    pub intake_consumer_poll_interval_ms: u64,
}

impl Default for ScientiaResearchMeshConfig {
    fn default() -> Self {
        Self {
            intake_writer_enabled: false,
            intake_consumer_poll_enabled: false,
            intake_consumer_poll_interval_ms: default_intake_consumer_poll_interval_ms(),
        }
    }
}

impl OrchestratorConfig {
    /// Intake JSON writes are enabled when explicitly configured or when news syndication is on.
    #[must_use]
    pub fn research_mesh_intake_writer_active(&self) -> bool {
        self.scientia_research_mesh.intake_writer_enabled || self.news.enabled
    }
}

#[cfg(test)]
mod tests {
    use crate::config::OrchestratorConfig;

    #[test]
    fn intake_writer_active_follows_flags() {
        let mut c = OrchestratorConfig::default();
        assert!(!c.research_mesh_intake_writer_active());
        c.scientia_research_mesh.intake_writer_enabled = true;
        assert!(c.research_mesh_intake_writer_active());
        c.scientia_research_mesh.intake_writer_enabled = false;
        c.news.enabled = true;
        assert!(c.research_mesh_intake_writer_active());
    }
}
