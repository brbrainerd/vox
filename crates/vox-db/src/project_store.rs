use std::path::Path;

use crate::DbConfig;
use crate::VoxDb;
use crate::store::{DEFAULT_PROJECT_STORE_PATH, StoreError};

fn ensure_parent_dir(path: &std::path::Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
    }
    Ok(())
}

/// Opens `.vox/store.db` under an explicit repository root (workspace journey store).
#[allow(clippy::missing_errors_doc)]
pub async fn open_project_db_at_root(repo_root: &Path) -> Result<VoxDb, StoreError> {
    let db_path = repo_root.join(DEFAULT_PROJECT_STORE_PATH);
    ensure_parent_dir(&db_path)?;

    let config = DbConfig::Local {
        path: db_path.to_string_lossy().to_string(),
    };
    VoxDb::connect(config).await
}

/// Opens the **project-local** VoxDb (`.vox/store.db`) for repo-scoped artifacts only.
///
/// This is **not** the canonical user-global Codex store ([`crate::canonical_store`]); use
/// [`crate::DbConfig::resolve_canonical`] + [`crate::VoxDb::connect`] for authoritative relational data.
/// Discovers the repository root, ensures `.vox/` exists, and opens Turso.
pub async fn open_project_db() -> Result<VoxDb, StoreError> {
    let cwd = std::env::current_dir().map_err(StoreError::Io)?;
    let repo = vox_repository::discover_repository_or_fallback(&cwd);

    open_project_db_at_root(&repo.root).await
}

#[deprecated(note = "renamed to open_project_db")]
pub async fn open_project_code_store() -> Result<VoxDb, StoreError> {
    open_project_db().await
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn open_project_db_at_root_creates_dot_vox_store_under_given_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        // Precondition: nothing exists yet under the fresh root.
        let expected_db = root.join(".vox").join("store.db");
        assert!(
            !expected_db.exists(),
            "store.db must not exist before the call"
        );

        let db = open_project_db_at_root(root).await;
        assert!(
            db.is_ok(),
            "open_project_db_at_root should succeed for a writable root"
        );

        // Observable effect: the .vox/ parent dir was created and the store file now exists.
        assert!(
            root.join(".vox").is_dir(),
            "ensure_parent_dir must have created the .vox/ directory"
        );
        assert!(
            expected_db.exists(),
            "the local store.db must exist at <root>/.vox/store.db after connect"
        );
    }
}
