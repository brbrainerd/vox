//! Cached effective-config snapshot for [`OrchestratorConfig`].
//!
//! Call [`OrchestratorConfig::snapshot()`] to obtain the env-merged effective
//! config. The result is cached per snapshot revision (see [`vox_config::snapshot`])
//! so repeated reads within the same rev are zero-cost clone operations.

use vox_config::snapshot::SnapshotCache;

use super::orchestrator_fields::OrchestratorConfig;

static ORCHESTRATOR_CACHE: SnapshotCache<OrchestratorConfig> = SnapshotCache::new();

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

            // Apply Vox.toml [orchestrator] section if present.
            let cwd_toml = std::path::Path::new("Vox.toml");
            if let Ok(loaded) = Self::load_from_toml(cwd_toml) {
                cfg = loaded;
            }

            // Apply env overrides (VOX_ORCHESTRATOR_* wins over Vox.toml).
            cfg.merge_env_overrides();

            cfg
        })
    }
}
