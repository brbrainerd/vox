//! Repository-aware **workspace journey** VoxDB resolution (Unified Vox Request Journey).
//!
//! MCP and repo-scoped daemons default to `.vox/store.db` under the discovered repository root so
//! interactive sessions, transcripts, and orchestration telemetry stay clone-local. Set
//! `VOX_WORKSPACE_JOURNEY_STORE=canonical` to restore the legacy user-global / Turso canonical path.
//!
//! When the project store open fails and `VOX_WORKSPACE_JOURNEY_FALLBACK_CANONICAL` is truthy
//! (default), surfaces may fall back to `super::connect_canonical_optional`.

use std::path::Path;

use crate::DbConnectSurface;
use crate::StoreError;
use crate::VoxDb;
use crate::connect_canonical_optional;
use crate::project_store::open_project_db_at_root;

pub use vox_db_types::WorkspaceJourneyStoreMode;

/// Read `VOX_WORKSPACE_JOURNEY_STORE`: `project` (default) or `canonical`.
#[must_use]
pub fn workspace_journey_store_mode_from_env() -> WorkspaceJourneyStoreMode {
    let val = vox_config::env_parse::resolve_config_str("VOX_WORKSPACE_JOURNEY_STORE", "project");
    if val.trim().eq_ignore_ascii_case("canonical") {
        WorkspaceJourneyStoreMode::Canonical
    } else {
        WorkspaceJourneyStoreMode::Project
    }
}

fn workspace_journey_fallback_canonical_enabled() -> bool {
    vox_config::env_parse::resolve_config_bool("VOX_WORKSPACE_JOURNEY_FALLBACK_CANONICAL", true)
}

/// Connect the primary DB for a repo-backed journey using CWD discovery.
///
/// - [`WorkspaceJourneyStoreMode::Canonical`]: same as [`connect_canonical_optional`].
/// - [`WorkspaceJourneyStoreMode::Project`]: opens `.vox/store.db` under discovered repo root;
///   on failure, optionally falls back to canonical when enabled.
pub async fn connect_workspace_journey_optional(
    surface: DbConnectSurface,
    skip_log: bool,
) -> Option<VoxDb> {
    connect_workspace_journey_optional_at(
        surface,
        std::env::current_dir().ok().as_deref(),
        skip_log,
    )
    .await
}

/// Like [`connect_workspace_journey_optional`], but uses `start_dir` for repository discovery
/// (falls back to `"."` if `None`).
pub async fn connect_workspace_journey_optional_at(
    surface: DbConnectSurface,
    start_dir: Option<&Path>,
    skip_log: bool,
) -> Option<VoxDb> {
    match workspace_journey_store_mode_from_env() {
        WorkspaceJourneyStoreMode::Canonical => connect_canonical_optional(surface, skip_log).await,
        WorkspaceJourneyStoreMode::Project => {
            let hint = start_dir
                .map(std::path::Path::to_path_buf)
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            let discover_hint =
                vox_repository::find_project_manifest_root(&hint).unwrap_or_else(|| hint.clone());
            let repo = vox_repository::discover_repository_or_fallback(&discover_hint);
            match open_project_db_at_root(&repo.root).await {
                Ok(db) => Some(db),
                Err(e) => {
                    if !skip_log {
                        tracing::warn!(
                            target: "vox_db::workspace_journey_store",
                            surface = surface.as_str(),
                            error = %e,
                            repo_root = %repo.root.display(),
                            "open workspace journey store (.vox/store.db) failed"
                        );
                    }
                    if workspace_journey_fallback_canonical_enabled() {
                        if !skip_log {
                            tracing::info!(
                                target: "vox_db::workspace_journey_store",
                                surface = surface.as_str(),
                                "falling back to canonical VoxDB (VOX_WORKSPACE_JOURNEY_FALLBACK_CANONICAL)"
                            );
                        }
                        connect_canonical_optional(surface, skip_log).await
                    } else {
                        None
                    }
                }
            }
        }
    }
}

/// Human-readable summary for diagnostics / `orch.workspace_journey`.
#[must_use]
pub fn workspace_journey_diagnostics_json(
    repo_root: &Path,
    repository_id: &str,
) -> serde_json::Value {
    let mode = workspace_journey_store_mode_from_env();
    let store_path = repo_root.join(crate::store::DEFAULT_PROJECT_STORE_PATH);
    serde_json::json!({
        "workspace_journey_store_mode": match mode {
            WorkspaceJourneyStoreMode::Project => "project",
            WorkspaceJourneyStoreMode::Canonical => "canonical",
        },
        "project_store_path": store_path.to_string_lossy(),
        "repository_id": repository_id,
        "fallback_canonical": workspace_journey_fallback_canonical_enabled(),
    })
}

/// Format error for tests / callers.
#[allow(dead_code)]
pub fn format_project_open_err(e: &StoreError) -> String {
    format!("workspace journey store: {e}")
}
