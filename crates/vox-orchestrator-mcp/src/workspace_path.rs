//! Repository-relative path resolution for MCP tools (deterministic workspace root joining).

use std::path::{Path, PathBuf};

use crate::server_state::ServerState;

/// Confirm the path is inside the MCP workspace, exists, and is readable UTF-8 text.
pub(crate) const REM_VALIDATE_IO: &str =
    "Confirm the path is inside the MCP workspace, exists, and is readable UTF-8 text.";

/// Relative path must stay under the repository root after canonicalization.
pub(crate) const REM_PATH_ESCAPE: &str =
    "Use a path relative to the MCP workspace root, or an absolute path inside the repository.";

#[derive(Debug)]
pub(crate) enum ResolveRepoPathError {
    NotFound { requested: String },
    OutsideRepository,
    CanonicalizeRoot(String),
    CanonicalizeFile(String),
}

impl ResolveRepoPathError {
    pub(crate) fn remediation(&self) -> &'static str {
        match self {
            ResolveRepoPathError::NotFound { .. } => REM_VALIDATE_IO,
            ResolveRepoPathError::OutsideRepository => REM_PATH_ESCAPE,
            ResolveRepoPathError::CanonicalizeRoot(_)
            | ResolveRepoPathError::CanonicalizeFile(_) => REM_PATH_ESCAPE,
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            ResolveRepoPathError::NotFound { requested } => {
                format!("file not found: {requested}")
            }
            ResolveRepoPathError::OutsideRepository => {
                "path resolves outside the MCP repository root".to_string()
            }
            ResolveRepoPathError::CanonicalizeRoot(e) => {
                format!("failed to canonicalize repository root: {e}")
            }
            ResolveRepoPathError::CanonicalizeFile(e) => {
                format!("failed to canonicalize path: {e}")
            }
        }
    }
}

/// Join `path_str` to [`ServerState::repository`] root when relative; absolute paths are used as-is.
#[must_use]
pub(crate) fn resolve_under_repository_root(state: &ServerState, path_str: &str) -> PathBuf {
    let p = Path::new(path_str);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        state.repository.root.join(p)
    }
}

/// Resolve a path for reading files inside the bound repository (validation, codegen gates).
///
/// After verifying the path exists, canonicalizes file and repository root and rejects traversal
/// outside the repository.
pub(crate) fn resolve_existing_path_in_repository(
    state: &ServerState,
    path_str: &str,
) -> Result<PathBuf, ResolveRepoPathError> {
    let candidate = resolve_under_repository_root(state, path_str);
    if !candidate.exists() {
        return Err(ResolveRepoPathError::NotFound {
            requested: path_str.to_string(),
        });
    }
    let root_canon = std::fs::canonicalize(&state.repository.root)
        .map_err(|e| ResolveRepoPathError::CanonicalizeRoot(e.to_string()))?;
    let file_canon = std::fs::canonicalize(&candidate)
        .map_err(|e| ResolveRepoPathError::CanonicalizeFile(e.to_string()))?;
    if !file_canon.starts_with(&root_canon) {
        return Err(ResolveRepoPathError::OutsideRepository);
    }
    Ok(file_canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;

    fn test_state_with_root(root: PathBuf) -> ServerState {
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-workspace-path-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root,
            git_root: None,
            repository_id: "workspace-path-test".into(),
            origin_url: None,
            capabilities: RepoCapabilities {
                vox_project: false,
                cargo_workspace: false,
                cargo_package: false,
                node_workspace: false,
                python_project: false,
                go_module: false,
                git: false,
            },
            has_vox_agents_dir: false,
            vox_toml: None,
        };
        ServerState::hermetic_stub(
            cfg,
            repository,
            Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
            Arc::new(Mutex::new(session_manager)),
            new_registry_arc(),
        )
    }

    #[test]
    fn relative_path_joins_repo_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        fs::create_dir_all(root.join("pkg")).expect("mkdir");
        let f = root.join("pkg").join("hello.vox");
        fs::write(&f, "fn main() {}\n").expect("write");
        let state = test_state_with_root(root.clone());
        let got = resolve_existing_path_in_repository(&state, "pkg/hello.vox").expect("resolve");
        assert_eq!(got, f.canonicalize().expect("canon file"));
    }

    #[test]
    fn rejects_relative_path_that_escapes_repo() {
        let parent = tempfile::tempdir().expect("parent");
        let repo = parent.path().join("repo");
        let sibling = parent.path().join("sibling");
        fs::create_dir_all(&repo).expect("repo");
        fs::create_dir_all(&sibling).expect("sibling");
        fs::write(sibling.join("leak.vox"), "fn main() {}\n").expect("write");
        let state = test_state_with_root(repo);
        let err = resolve_existing_path_in_repository(&state, "../sibling/leak.vox").unwrap_err();
        assert!(
            matches!(err, ResolveRepoPathError::OutsideRepository),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_absolute_path_outside_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        fs::create_dir_all(&root).expect("repo root");
        let outside = tempfile::tempdir().expect("out");
        let f = outside.path().join("secret.vox");
        fs::write(&f, "fn main() {}\n").expect("write");
        let state = test_state_with_root(root);
        let err = resolve_existing_path_in_repository(&state, f.to_str().expect("utf8 path"))
            .unwrap_err();
        assert!(
            matches!(err, ResolveRepoPathError::OutsideRepository),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn validate_file_accepts_repo_relative_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        fs::create_dir_all(root.join("deep")).expect("mkdir");
        fs::write(root.join("deep").join("t.vox"), "fn main() {}\n").expect("write");
        let state = test_state_with_root(root);
        let json = crate::code_validator::validate_file(
            &state,
            crate::params::ValidateFileParams {
                path: "deep/t.vox".into(),
            },
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(v["success"], true, "{json}");
    }
}
