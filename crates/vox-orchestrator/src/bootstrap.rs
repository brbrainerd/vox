//! Repository-aware orchestrator bootstrap helpers shared across CLI and MCP surfaces.

use std::path::{Path, PathBuf};

use crate::{AffinityGroupRegistry, Orchestrator, OrchestratorConfig, load_from_config};

/// Single factory output for embedding `Orchestrator` in MCP, CLI, daemons, and tests.
///
/// Construct via [`build_repo_scoped_orchestrator`] or [`build_repo_scoped_orchestrator_for_repository`]
/// so all surfaces share identical repo-scoped config, affinity groups, and memory paths.
pub struct RepoScopedOrchestratorBuild {
    pub repository: vox_repository::RepositoryContext,
    /// Repository-scoped [`OrchestratorConfig`] (memory shard under `.vox/cache/repos/<repository_id>/`, etc.).
    pub config: OrchestratorConfig,
    pub orchestrator: Orchestrator,
}

/// Discover repository from `start_dir` (or process CWD), then build [`Orchestrator`] with repo-scoped config.
#[must_use]
pub fn build_repo_scoped_orchestrator(
    config: OrchestratorConfig,
    start_dir: Option<&Path>,
) -> RepoScopedOrchestratorBuild {
    let repository = discover_repository_from_cwd(start_dir);
    build_repo_scoped_orchestrator_for_repository(config, &repository)
}

/// Build [`Orchestrator`] for an already-resolved [`vox_repository::RepositoryContext`].
///
/// Use when the repository root was discovered out-of-band (for example `with_workspace_root` in MCP).
#[must_use]
pub fn build_repo_scoped_orchestrator_for_repository(
    config: OrchestratorConfig,
    repository: &vox_repository::RepositoryContext,
) -> RepoScopedOrchestratorBuild {
    let (repo_scoped_config, groups) = repo_scoped_orchestrator_parts(config, repository);
    let orchestrator = Orchestrator::with_groups(repo_scoped_config.clone(), groups);
    RepoScopedOrchestratorBuild {
        repository: repository.clone(),
        config: repo_scoped_config,
        orchestrator,
    }
}

/// Discover repository context from `start_dir`, falling back to process CWD then ".".
#[must_use]
pub fn discover_repository_from_cwd(start_dir: Option<&Path>) -> vox_repository::RepositoryContext {
    let cwd = start_dir
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let hint = vox_repository::find_project_manifest_root(&cwd).unwrap_or_else(|| cwd.clone());
    vox_repository::discover_repository_or_fallback(&hint)
}

/// Build repository-scoped config and affinity groups:
/// - `affinity_groups` from `Vox.toml` when present, else repository-layout detection
/// - memory rooted at `.vox/memory` (canonical workspace path; legacy shard migrated at startup).
#[must_use]
pub fn repo_scoped_orchestrator_parts(
    mut config: OrchestratorConfig,
    repository: &vox_repository::RepositoryContext,
) -> (OrchestratorConfig, AffinityGroupRegistry) {
    let groups = repository
        .vox_toml
        .as_deref()
        .and_then(load_from_config)
        .unwrap_or_else(|| AffinityGroupRegistry::detect_from_repository_layout(&repository.root));

    let mem_root = repository.root.join(".vox").join("memory");
    config.memory.log_dir = mem_root.clone();
    config.memory.memory_md_path = mem_root.join("MEMORY.md");

    (config, groups)
}

/// Convenience helper when only the repository-scoped config is needed.
#[must_use]
pub fn repo_scoped_orchestrator_config(
    config: OrchestratorConfig,
    repository: &vox_repository::RepositoryContext,
) -> OrchestratorConfig {
    repo_scoped_orchestrator_parts(config, repository).0
}

#[cfg(test)]
mod semcov_wave1_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::OrchestratorConfig;
    use std::path::PathBuf;

    #[test]
    fn repo_scoped_orchestrator_config_roots_memory_under_repo_vox_memory() {
        // Override input memory paths to a bogus location to prove they get replaced.
        let mut config = OrchestratorConfig::default();
        config.memory.log_dir = PathBuf::from("/bogus/log/dir");
        config.memory.memory_md_path = PathBuf::from("/bogus/MEMORY.md");

        let tmp = tempfile::tempdir().expect("create tempdir");
        let repository = vox_repository::discover_repository_or_fallback(tmp.path());

        let out = repo_scoped_orchestrator_config(config, &repository);

        let expected_mem_root = repository.root.join(".vox").join("memory");
        assert_eq!(out.memory.log_dir, expected_mem_root);
        assert_eq!(
            out.memory.memory_md_path,
            expected_mem_root.join("MEMORY.md")
        );
    }
}
