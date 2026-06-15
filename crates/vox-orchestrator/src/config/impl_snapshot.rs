//! Cached effective-config snapshot for [`OrchestratorConfig`].
//!
//! Call [`OrchestratorConfig::snapshot()`] to obtain the env-merged effective
//! config. The result is cached per snapshot revision (see [`vox_config::snapshot`])
//! so repeated reads within the same rev are zero-cost clone operations.

use vox_config::snapshot::SnapshotCache;

use super::orchestrator_fields::OrchestratorConfig;

static ORCHESTRATOR_CACHE: SnapshotCache<OrchestratorConfig> = SnapshotCache::new();

/// Walk up from the current executable's location to find `Vox.toml`.
///
/// Using `std::env::current_exe()` (rather than `current_dir()`) is reliable in
/// a Tauri process where the working directory may be `C:\Windows\System32` or the
/// application bundle directory. We walk up at most 10 levels to avoid infinite
/// traversal on unusual path layouts.
fn find_vox_toml() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    for _ in 0..10 {
        let candidate = dir.join("Vox.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    None
}

impl OrchestratorConfig {
    /// Returns the env-merged effective config, cached per snapshot rev.
    ///
    /// Precedence: defaults → `Vox.toml` `[orchestrator]` section → env overrides.
    ///
    /// The result is recomputed only after [`vox_config::snapshot::bump`] is called
    /// (e.g. on a hot-reload or `EnvScratch::drop`). Multiple concurrent calls within
    /// the same rev return a cheap clone of the already-computed value.
    pub fn snapshot() -> Self {
        ORCHESTRATOR_CACHE.get_or_init(|| {
            let mut cfg = Self::default();

            // Resolve Vox.toml by walking up from the binary (not from CWD, which is
            // unreliable in a Tauri/GUI process).
            match find_vox_toml() {
                Some(path) => match Self::load_from_toml(&path) {
                    Ok(loaded) => cfg = loaded,
                    // A parse failure here means an existing Vox.toml is being silently
                    // replaced with all-defaults — surface it at warn so a corrupt config
                    // is visible instead of masquerading as intentional defaults.
                    Err(e) => tracing::warn!(
                        "OrchestratorConfig::snapshot: failed to load {:?}, falling back to defaults: {e}",
                        path
                    ),
                },
                None => tracing::debug!(
                    "OrchestratorConfig::snapshot: Vox.toml not found from exe parent; using defaults"
                ),
            }

            // Apply env overrides (VOX_ORCHESTRATOR_* wins over Vox.toml).
            cfg.merge_env_overrides();

            cfg
        })
    }
}
