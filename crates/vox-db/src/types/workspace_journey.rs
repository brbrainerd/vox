//! Workspace-journey store mode enum.
//!
//! Moved from `vox_db::workspace_journey_store` so consumers can name the mode
//! without depending on the heavy `vox-db` crate. The env-resolution helper
//! (`workspace_journey_store_mode_from_env`) and the connect/diagnostics
//! helpers stay in `vox-db` because they call into `VoxDb`.

/// How repo-backed interactive surfaces resolve their primary `VoxDb` handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceJourneyStoreMode {
    /// `.vox/store.db` under `vox_repository::RepositoryContext::root`.
    Project,
    /// `vox_db::DbConfig::resolve_canonical` (user-global or `VOX_DB_URL`).
    Canonical,
}
