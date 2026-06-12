//! Thin driver surface for embedding [`vox_orchestrator::Orchestrator`] in CLI/MCP hosts.
//!
//! Extracted from `vox-cli-core::orchestrator_driver` to invert the CLI ↔ orchestrator
//! dependency over time (see `docs/src/architecture/layers.toml`).

use std::path::PathBuf;
use std::sync::Arc;

use vox_orchestrator::orchestrator::OrchestratorStatus;
use vox_orchestrator::{Orchestrator, OrchestratorConfig, build_repo_scoped_orchestrator};

/// Host-facing trait for spawning and querying an embedded orchestrator.
pub trait OrchestratorDriver {
    /// Loaded config (after Vox.toml discovery and env overrides).
    fn config(&self) -> &OrchestratorConfig;

    /// Shared orchestrator handle.
    fn orchestrator(&self) -> Arc<Orchestrator>;

    /// Start background pollers/sidecars owned by the orchestrator.
    fn spawn_background_tasks(&self);

    /// Snapshot status for CLI/MCP dashboards.
    fn status(&self) -> OrchestratorStatus;
}

/// In-process orchestrator built from discovered `Vox.toml` + repo-scoped bootstrap.
pub struct EmbeddedOrchestratorDriver {
    config: OrchestratorConfig,
    orchestrator: Arc<Orchestrator>,
}

impl EmbeddedOrchestratorDriver {
    /// Discover config from the workspace, then build a repo-scoped orchestrator.
    #[must_use]
    pub fn load_and_build() -> Self {
        let config = build_embedded_orchestrator_config();
        Self::from_config(config)
    }

    /// Build from an already-loaded config (tests / callers with custom config).
    #[must_use]
    pub fn from_config(config: OrchestratorConfig) -> Self {
        let orchestrator =
            Arc::new(build_repo_scoped_orchestrator(config.clone(), None).orchestrator);
        Self {
            config,
            orchestrator,
        }
    }
}

impl OrchestratorDriver for EmbeddedOrchestratorDriver {
    fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    fn orchestrator(&self) -> Arc<Orchestrator> {
        self.orchestrator.clone()
    }

    fn spawn_background_tasks(&self) {
        self.orchestrator.clone().spawn_background_tasks();
    }

    fn status(&self) -> OrchestratorStatus {
        self.orchestrator.status()
    }
}

/// Load orchestrator config from the nearest readable `Vox.toml`, then apply env overrides.
///
/// Discovery order matches the MCP stdio server: manifest-root `Vox.toml` first,
/// then cwd `Vox.toml`, then defaults.
#[must_use]
pub fn build_embedded_orchestrator_config() -> OrchestratorConfig {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "current_dir failed; using \".\" for Vox.toml discovery");
            PathBuf::from(".")
        }
    };
    let mut candidates = Vec::new();
    if let Some(root) = vox_repository::find_project_manifest_root(&cwd) {
        candidates.push(root.join("Vox.toml"));
    }
    candidates.push(PathBuf::from("Vox.toml"));

    let mut config = OrchestratorConfig::default();
    let mut loaded = false;
    for toml_path in candidates {
        if toml_path.is_file() {
            match OrchestratorConfig::load_from_toml(&toml_path) {
                Ok(cfg) => {
                    tracing::info!(path = %toml_path.display(), "loaded orchestrator config from Vox.toml");
                    config = cfg;
                    loaded = true;
                    break;
                }
                Err(e) => tracing::warn!(
                    path = %toml_path.display(),
                    "failed to load Vox.toml: {e}, trying next candidate"
                ),
            }
        }
    }
    if !loaded {
        tracing::info!("no readable Vox.toml found, using defaults");
    }

    config.merge_env_overrides();
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn build_embedded_orchestrator_config_without_toml_uses_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Stop manifest discovery from walking up into the real workspace.
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"isolated_orchestrator_driver_test\"\n",
        )
        .expect("write anchor manifest");
        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(tmp.path()).expect("chdir");
        let cfg = build_embedded_orchestrator_config();
        std::env::set_current_dir(prev).expect("restore cwd");
        let mut expected = OrchestratorConfig::default();
        expected.merge_env_overrides();
        assert_eq!(cfg.max_agents, expected.max_agents);
    }

    #[test]
    fn build_embedded_orchestrator_config_reads_manifest_root_toml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("proj");
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(root.join("Vox.toml"), "[orchestrator]\nmax_agents = 9\n").expect("write toml");

        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&root).expect("chdir");
        let cfg = build_embedded_orchestrator_config();
        std::env::set_current_dir(prev).expect("restore cwd");

        assert_eq!(cfg.max_agents, 9);
    }

    #[test]
    fn embedded_driver_exposes_status() {
        let driver = EmbeddedOrchestratorDriver::load_and_build();
        let _status = driver.status();
    }
}
