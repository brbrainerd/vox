/// Configuration for opening **Codex** / [`crate::VoxDb`] (Turso or libSQL).
#[derive(Debug, Clone)]
pub enum DbConfig {
    /// Remote Turso / libSQL (sync client; see `vox-package` `open_remote`).
    Remote {
        /// Database URL (e.g. `libsql://...`).
        url: String,
        /// Auth token for the remote.
        token: String,
    },

    /// Persistent local file (requires `local` feature).
    #[cfg(feature = "local")]
    Local {
        /// Path passed to `turso::Builder::new_local`.
        path: String,
    },

    /// Ephemeral `:memory:` database for unit tests (requires `local` feature).
    #[cfg(feature = "local")]
    Memory,

    /// Local path replicated against a remote primary (requires `replication` feature).
    #[cfg(feature = "replication")]
    EmbeddedReplica {
        /// On-disk path for the replica.
        local_path: String,
        /// Remote URL.
        url: String,
        /// Remote auth token.
        token: String,
    },
}

use vox_secrets::SecretId;

static LEGACY_TURSO_ENV_WARN: std::sync::Once = std::sync::Once::new();

fn try_remote_from_compat_env() -> Option<DbConfig> {
    let hard_cut_strict = std::env::var("VOX_SECRETS_HARD_CUT")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false);
    let cutover_phase_blocks_compat = std::env::var("VOX_SECRETS_CUTOVER_PHASE")
        .or_else(|_| std::env::var("VOX_SECRETS_MIGRATION_PHASE"))
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .is_some_and(|phase| matches!(phase.as_str(), "enforce" | "decommission"));
    if hard_cut_strict || cutover_phase_blocks_compat {
        return None;
    }

    let res_url = vox_secrets::resolve_secret(SecretId::VoxDbUrl);
    let res_token = vox_secrets::resolve_secret(SecretId::VoxDbToken);

    if let (Some(url), Some(token)) = (res_url.expose(), res_token.expose()) {
        if matches!(
            res_url.status,
            vox_secrets::ResolutionStatus::DeprecatedAliasUsed
        ) || matches!(
            res_token.status,
            vox_secrets::ResolutionStatus::DeprecatedAliasUsed
        ) {
            LEGACY_TURSO_ENV_WARN.call_once(|| {
                tracing::warn!(
                    target: "vox_db::config",
                    "Legacy TURSO_* env vars are deprecated; set VOX_DB_URL and VOX_DB_TOKEN (see docs/src/reference/env-vars.md)"
                );
            });
        }
        return Some(DbConfig::remote(url, token));
    }
    None
}

impl DbConfig {
    /// Create a remote config from URL and token.
    pub fn remote(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::Remote {
            url: url.into(),
            token: token.into(),
        }
    }

    /// Create a local file config (requires `local` feature).
    #[cfg(feature = "local")]
    pub fn local(path: impl Into<String>) -> Self {
        Self::Local { path: path.into() }
    }

    /// Create an in-memory config for testing (requires `local` feature).
    #[cfg(feature = "local")]
    pub fn memory() -> Self {
        Self::Memory
    }

    /// Create an embedded replica config (requires `replication` feature).
    #[cfg(feature = "replication")]
    pub fn embedded_replica(
        local_path: impl Into<String>,
        url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self::EmbeddedReplica {
            local_path: local_path.into(),
            url: url.into(),
            token: token.into(),
        }
    }

    /// Read config from `VOX_DB_URL` + `VOX_DB_TOKEN` (remote), or `VOX_DB_PATH` (local), or all
    /// three for embedded replica when `replication` is enabled. Empty env + `local` → [`Self::Memory`].
    pub fn from_env() -> Result<Self, String> {
        let url = vox_secrets::resolve_secret(SecretId::VoxDbUrl)
            .expose()
            .map(String::from);
        let token = vox_secrets::resolve_secret(SecretId::VoxDbToken)
            .expose()
            .map(String::from);
        let path = std::env::var("VOX_DB_PATH").ok();

        match (url, token, path) {
            (Some(_u), Some(_t), Some(_p)) => {
                #[cfg(feature = "replication")]
                return Ok(Self::embedded_replica(_p, _u, _t));
                #[cfg(not(feature = "replication"))]
                return Err("Embedded replica config requires 'replication' feature".into());
            }
            (Some(u), Some(t), None) => Ok(Self::remote(u, t)),
            (None, None, Some(_p)) => {
                #[cfg(feature = "local")]
                return Ok(Self::local(_p));
                #[cfg(not(feature = "local"))]
                return Err("Local DB config requires 'local' feature".into());
            }
            (None, None, None) => {
                #[cfg(feature = "local")]
                return Ok(Self::memory());
                #[cfg(not(feature = "local"))]
                return Err("Memory DB config requires 'local' feature".into());
            }
            _ => Err("Invalid database configuration in environment variables".into()),
        }
    }

    /// Resolve configuration for long-running apps and CLIs: canonical `VOX_DB_*`, then compatibility
    /// aliases `VOX_TURSO_URL`/`VOX_TURSO_TOKEN`, then legacy `TURSO_URL`/`TURSO_AUTH_TOKEN`, then local
    /// file (`VOX_DB_PATH`, platform default, or `app.db`).
    ///
    /// Unlike [`Self::from_env`], never returns [`Self::Memory`] when the `local` feature is enabled;
    /// an empty environment selects a concrete file path instead.
    ///
    /// For new code, prefer [`Self::resolve_canonical`] (same behavior; documents SSOT intent). See
    /// [`crate::canonical_store`].
    pub fn resolve_standalone() -> Result<Self, String> {
        let path_fallback = || {
            std::env::var("VOX_DB_PATH")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    vox_config::paths::default_db_path().map(|p| p.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "app.db".into())
        };

        match Self::from_env() {
            Ok(cfg) => {
                #[cfg(feature = "local")]
                if matches!(cfg, Self::Memory) {
                    return Ok(Self::local(path_fallback()));
                }
                Ok(cfg)
            }
            Err(_) => {
                if let Some(cfg) = try_remote_from_compat_env() {
                    return Ok(cfg);
                }
                #[cfg(feature = "local")]
                {
                    Ok(Self::local(path_fallback()))
                }
                #[cfg(not(feature = "local"))]
                Err(
                    "Database resolution requires the `local` feature or valid VOX_DB_URL+VOX_DB_TOKEN"
                        .into(),
                )
            }
        }
    }

    /// Authoritative **user-global** Codex / VoxDB configuration.
    ///
    /// Equivalent to [`Self::resolve_standalone`]. Use this for all relational product data except
    /// [`crate::open_project_db`] (repo-local cache) and [`crate::VoxDb::connect_legacy_export_only`].
    ///
    /// See [`crate::canonical_store`] for the full storage policy.
    pub fn resolve_canonical() -> Result<Self, String> {
        Self::resolve_standalone()
    }

    /// Resolve config for the **project** Arca `crate::store::VoxDb` (snippets, share, etc.).
    ///
    /// Uses canonical [`Self::from_env`] (`VOX_DB_*`), mapping an empty environment to the project
    /// file [`crate::store::DEFAULT_PROJECT_STORE_PATH`] instead of the user data default from
    /// [`Self::resolve_standalone`]. On failure of `from_env`, applies the same Turso compatibility
    /// aliases as [`Self::resolve_standalone`], then falls back to the project store path.
    pub fn resolve_project_code_store_config() -> Result<Self, String> {
        match Self::from_env() {
            Ok(cfg) => {
                #[cfg(feature = "local")]
                if matches!(cfg, Self::Memory) {
                    return Ok(Self::local(
                        crate::store::DEFAULT_PROJECT_STORE_PATH.to_string(),
                    ));
                }
                Ok(cfg)
            }
            Err(_) => {
                if let Some(cfg) = try_remote_from_compat_env() {
                    return Ok(cfg);
                }
                #[cfg(feature = "local")]
                {
                    Ok(Self::local(
                        crate::store::DEFAULT_PROJECT_STORE_PATH.to_string(),
                    ))
                }
                #[cfg(not(feature = "local"))]
                Err(
                    "Project VoxDb requires the `local` feature or valid VOX_DB_URL+VOX_DB_TOKEN"
                        .into(),
                )
            }
        }
    }

    /// Resolve configuration specifically for a mens node:
    /// - If `VOX_DB_URL`, `VOX_DB_TOKEN`, AND `VOX_DB_PATH` are set, use `Self::EmbeddedReplica`.
    /// - If only `VOX_DB_URL` + `VOX_DB_TOKEN` are set, use [`Self::Remote`].
    /// - Otherwise, fall back to [`Self::resolve_standalone`] (local file).
    pub fn resolve_for_mesh() -> Result<Self, String> {
        let url = vox_secrets::resolve_secret(SecretId::VoxDbUrl)
            .expose()
            .map(String::from);
        let token = vox_secrets::resolve_secret(SecretId::VoxDbToken)
            .expose()
            .map(String::from);
        let path = std::env::var("VOX_DB_PATH").ok();

        match (url, token, path) {
            (Some(u), Some(t), Some(_p)) => {
                #[cfg(feature = "replication")]
                return Ok(Self::EmbeddedReplica {
                    local_path: _p,
                    url: u,
                    token: t,
                });
                #[cfg(not(feature = "replication"))]
                {
                    tracing::warn!(
                        "EmbeddedReplica requested for mens but 'replication' feature is disabled; falling back to Remote"
                    );
                    Ok(Self::Remote { url: u, token: t })
                }
            }
            (Some(u), Some(t), None) => Ok(Self::Remote { url: u, token: t }),
            _ => Self::resolve_canonical(),
        }
    }
}

/// Resolve the app-plane SQL backend URL (`VOX_APP_DB_URL`) via the secret-policy
/// SSOT (env, Clavis vault, profile overrides) instead of a raw env read.
///
/// Generated app runtimes call this to decide backend routing/guards; never read
/// `VOX_APP_DB_URL` directly.
#[must_use]
pub fn resolve_app_db_url() -> Option<String> {
    vox_secrets::resolve_secret(SecretId::VoxAppDbUrl)
        .expose()
        .map(String::from)
}

/// Resolve the canonical Codex DB URL (`VOX_DB_URL`) via the secret-policy SSOT.
#[must_use]
pub fn resolve_codex_db_url() -> Option<String> {
    vox_secrets::resolve_secret(SecretId::VoxDbUrl)
        .expose()
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::{DbConfig, try_remote_from_compat_env};
    use vox_secrets::SecretId;

    // Re-use the crate-level env lock so local_tests can also serialise against us.
    use crate::TEST_ENV_LOCK as ENV_LOCK;

    #[test]
    #[allow(unsafe_code)]
    fn hard_cut_disables_compat_remote_aliases() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let prev_hard_cut = std::env::var("VOX_SECRETS_HARD_CUT").ok();
        let prev_url = std::env::var(SecretId::VoxDbUrl.spec().canonical_env).ok();
        let prev_token = std::env::var(SecretId::VoxDbToken.spec().canonical_env).ok();
        unsafe {
            std::env::set_var("VOX_SECRETS_HARD_CUT", "1");
            std::env::set_var(
                SecretId::VoxDbUrl.spec().canonical_env,
                "libsql://example.turso.io",
            );
            std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, "token");
        }
        assert!(try_remote_from_compat_env().is_none());
        unsafe {
            match prev_hard_cut {
                Some(v) => std::env::set_var("VOX_SECRETS_HARD_CUT", v),
                None => std::env::remove_var("VOX_SECRETS_HARD_CUT"),
            }
            match prev_url {
                Some(v) => std::env::set_var(SecretId::VoxDbUrl.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbUrl.spec().canonical_env),
            }
            match prev_token {
                Some(v) => std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbToken.spec().canonical_env),
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn lenient_mode_allows_compat_remote_aliases() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let prev_hard_cut = std::env::var("VOX_SECRETS_HARD_CUT").ok();
        let prev_url = std::env::var(SecretId::VoxDbUrl.spec().canonical_env).ok();
        let prev_token = std::env::var(SecretId::VoxDbToken.spec().canonical_env).ok();
        unsafe {
            std::env::set_var("VOX_SECRETS_HARD_CUT", "0");
            std::env::set_var(
                SecretId::VoxDbUrl.spec().canonical_env,
                "libsql://example.turso.io",
            );
            std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, "token");
        }
        let cfg = try_remote_from_compat_env().expect("compat alias should resolve");
        assert!(matches!(cfg, DbConfig::Remote { .. }));
        unsafe {
            match prev_hard_cut {
                Some(v) => std::env::set_var("VOX_SECRETS_HARD_CUT", v),
                None => std::env::remove_var("VOX_SECRETS_HARD_CUT"),
            }
            match prev_url {
                Some(v) => std::env::set_var(SecretId::VoxDbUrl.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbUrl.spec().canonical_env),
            }
            match prev_token {
                Some(v) => std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbToken.spec().canonical_env),
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn enforce_cutover_phase_disables_compat_remote_aliases() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
        let prev_migration = std::env::var("VOX_SECRETS_MIGRATION_PHASE").ok();
        let prev_hard_cut = std::env::var("VOX_SECRETS_HARD_CUT").ok();
        let prev_url = std::env::var(SecretId::VoxDbUrl.spec().canonical_env).ok();
        let prev_token = std::env::var(SecretId::VoxDbToken.spec().canonical_env).ok();
        unsafe {
            std::env::set_var("VOX_SECRETS_HARD_CUT", "0");
            std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "enforce");
            std::env::remove_var("VOX_SECRETS_MIGRATION_PHASE");
            std::env::set_var(
                SecretId::VoxDbUrl.spec().canonical_env,
                "libsql://example.turso.io",
            );
            std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, "token");
        }
        assert!(try_remote_from_compat_env().is_none());
        unsafe {
            match prev_cutover {
                Some(v) => std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", v),
                None => std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE"),
            }
            match prev_migration {
                Some(v) => std::env::set_var("VOX_SECRETS_MIGRATION_PHASE", v),
                None => std::env::remove_var("VOX_SECRETS_MIGRATION_PHASE"),
            }
            match prev_hard_cut {
                Some(v) => std::env::set_var("VOX_SECRETS_HARD_CUT", v),
                None => std::env::remove_var("VOX_SECRETS_HARD_CUT"),
            }
            match prev_url {
                Some(v) => std::env::set_var(SecretId::VoxDbUrl.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbUrl.spec().canonical_env),
            }
            match prev_token {
                Some(v) => std::env::set_var(SecretId::VoxDbToken.spec().canonical_env, v),
                None => std::env::remove_var(SecretId::VoxDbToken.spec().canonical_env),
            }
        }
    }
}
