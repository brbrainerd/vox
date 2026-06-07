use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::paths::dot_vox_user_dir;

/// Flat key-value user config store loaded from `~/.vox/config.toml`.
/// Keys match canonical OperatorEnvSpec names (e.g. "vox_populi::inference_PROFILE").
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct VoxUserConfig {
    #[serde(flatten)]
    pub values: HashMap<String, toml::Value>,
}

static CONFIG_CACHE: OnceLock<Arc<Mutex<VoxUserConfig>>> = OnceLock::new();

fn get_config_path() -> PathBuf {
    dot_vox_user_dir().join("config.toml")
}

fn initialize_cache() -> Arc<Mutex<VoxUserConfig>> {
    let path = get_config_path();
    let config = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        VoxUserConfig::default()
    };
    Arc::new(Mutex::new(config))
}

/// Load `~/.vox/config.toml`; silently returns an empty config if missing or malformed.
/// This uses an in-memory cache for fast, repeated lookups.
pub fn load_user_config() -> VoxUserConfig {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let guard = cache.lock().expect("config cache mutex poisoned");
    guard.clone()
}

/// Re-read `~/.vox/config.toml` from disk into the in-memory cache, discarding
/// any cached state.
///
/// CACHE-COHERENCE CONTRACT: [`crate::config::VoxConfig::save`] writes the file
/// with a direct `fs::write` that bypasses this module's cache. Callers that mix
/// `VoxConfig::save()` with the flat [`set_user_config_value`] / [`unset_user_config_value`]
/// in the same process MUST call `reload_user_config()` after a `VoxConfig::save()`
/// so the flat cache reflects the on-disk truth before the next flat write.
/// (The flat writers below already read-modify-write the file fresh, so they will
/// not clobber sectioned `[vox]`/`[train]`/`[db]` tables even without this call;
/// the reload keeps subsequent *reads* through the cache coherent.)
pub fn reload_user_config() {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let path = get_config_path();
    let fresh = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        VoxUserConfig::default()
    };
    let mut guard = cache.lock().expect("config cache mutex poisoned");
    *guard = fresh;
}

/// Read the on-disk config.toml as a raw top-level TOML table (preserving every
/// section, including the sectioned `[vox]`/`[train]`/`[db]` tables written by
/// [`crate::config::VoxConfig::save`]). Returns an empty table when the file is
/// missing or malformed.
fn read_root_table() -> toml::value::Table {
    let path = get_config_path();
    if !path.exists() {
        return toml::value::Table::new();
    }
    match fs::read_to_string(&path)
        .ok()
        .and_then(|c| toml::from_str::<toml::Value>(&c).ok())
    {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    }
}

fn write_root_table(root: &toml::value::Table) -> Result<(), String> {
    let toml_str = toml::to_string(&toml::Value::Table(root.clone()))
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    fs::write(&path, toml_str).map_err(|e| format!("Failed to write config file: {e}"))?;
    Ok(())
}

/// Persist a flat top-level key-value pair to `~/.vox/config.toml`.
///
/// Read-modify-writes the file fresh so that sectioned tables written by
/// [`crate::config::VoxConfig::save`] (e.g. `[vox]`, `[train]`) are preserved and
/// never clobbered by a stale cache. The cache is updated to match.
pub fn set_user_config_value(key: &str, value: &str) -> Result<(), String> {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let mut guard = cache.lock().expect("config cache mutex poisoned");

    let mut root = read_root_table();
    root.insert(key.to_string(), toml::Value::String(value.to_string()));
    write_root_table(&root)?;

    // Keep the cache coherent: refresh the flat view from the table we just wrote.
    guard.values = root
        .into_iter()
        .filter(|(_, v)| !matches!(v, toml::Value::Table(_)))
        .collect();
    Ok(())
}

/// Remove a flat top-level key from `~/.vox/config.toml`.
///
/// Read-modify-writes the file fresh (see [`set_user_config_value`]).
pub fn unset_user_config_value(key: &str) -> Result<bool, String> {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let mut guard = cache.lock().expect("config cache mutex poisoned");

    let mut root = read_root_table();
    let removed = root.remove(key).is_some();
    if removed {
        write_root_table(&root)?;
        guard.values = root
            .into_iter()
            .filter(|(_, v)| !matches!(v, toml::Value::Table(_)))
            .collect();
    }
    Ok(removed)
}

/// Shared test-only lock + home-redirection guard so config-cache/env tests across modules
/// serialize against one another and never write the user's real `~/.vox/config.toml`.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Mutex;

    /// Process-wide lock serializing all tests that touch process env or the global config cache.
    pub(crate) static CONFIG_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Insert a **typed** TOML value (integer/float/bool) into the in-memory config cache,
    /// bypassing the string-only [`super::set_user_config_value`]. Lets tests exercise the
    /// numeric coercion arms in `env_parse` (e.g. a float stored as a TOML float read as i32).
    pub(crate) fn set_user_config_typed(key: &str, value: toml::Value) {
        let cache = super::CONFIG_CACHE.get_or_init(super::initialize_cache);
        let mut guard = cache.lock().expect("config cache mutex poisoned");
        guard.values.insert(key.to_string(), value);
    }

    /// Redirects `HOME`/`USERPROFILE` to a temp dir; restores them on drop.
    #[allow(unsafe_code)]
    pub(crate) struct HomeGuard {
        _tmp: tempfile::TempDir,
        prev: Vec<(&'static str, Option<String>)>,
    }

    impl HomeGuard {
        /// Temp directory standing in for the user's home.
        pub(crate) fn home(&self) -> &std::path::Path {
            self._tmp.path()
        }

        #[allow(unsafe_code)]
        pub(crate) fn new() -> Self {
            let tmp = tempfile::tempdir().expect("tempdir");
            let keys = ["HOME", "USERPROFILE"];
            let prev = keys
                .iter()
                .map(|k| (*k, std::env::var(k).ok()))
                .collect::<Vec<_>>();
            for k in keys {
                unsafe {
                    std::env::set_var(k, tmp.path());
                }
            }
            Self { _tmp: tmp, prev }
        }
    }

    impl Drop for HomeGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            for (k, v) in &self.prev {
                unsafe {
                    match v {
                        Some(val) => std::env::set_var(k, val),
                        None => std::env::remove_var(k),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod cache_coherence_tests {
    use super::test_support::{CONFIG_TEST_LOCK, HomeGuard};

    /// Regression: writing a `VoxConfig`-tier field (sectioned `[vox].model`) AND an
    /// inference-tier flat key (top-level `OPENROUTER_BASE_URL`) into the SAME
    /// `~/.vox/config.toml` in one process must not let either writer clobber the
    /// other's value via a stale flat cache.
    ///
    /// We force `VoxConfig::save()` (which uses `data_dir()`) and the flat
    /// `set_user_config_value` (which uses `dot_vox_user_dir()`) onto the same file by
    /// pointing `VOX_DATA_DIR` at `<home>/.vox`.
    #[test]
    fn flat_and_sectioned_writes_do_not_clobber_each_other() {
        let _lock = CONFIG_TEST_LOCK.lock().expect("test lock");
        let home = HomeGuard::new();
        let dot_vox = home.home().join(".vox");
        std::fs::create_dir_all(&dot_vox).expect("mkdir .vox");

        let prev_data_dir = std::env::var("VOX_DATA_DIR").ok();
        unsafe {
            std::env::set_var("VOX_DATA_DIR", &dot_vox);
        }
        // Ensure the flat-key resolver never reads a real env override.
        let prev_or = std::env::var("OPENROUTER_BASE_URL").ok();
        unsafe {
            std::env::remove_var("OPENROUTER_BASE_URL");
        }

        // Both writers now resolve to <home>/.vox/config.toml.
        assert_eq!(
            super::get_config_path(),
            dot_vox.join("config.toml"),
            "flat writer path must be the shared file",
        );
        assert_eq!(
            crate::config::persist_test_global_config_path(),
            Some(dot_vox.join("config.toml")),
            "VoxConfig::save path must be the shared file",
        );

        // 1) Write a VoxConfig-tier field via VoxConfig::save().
        let mut cfg = crate::config::VoxConfig::default();
        cfg.model = "test-org/test-model".to_string();
        cfg.save().expect("VoxConfig::save");

        // The flat cache may now be stale relative to the sectioned tables on disk;
        // refresh it per the cache-coherence contract.
        super::reload_user_config();

        // 2) Write an inference-tier flat key.
        super::set_user_config_value("OPENROUTER_BASE_URL", "https://gateway.example/api")
            .expect("set flat key");

        // 3) Reload from disk and assert BOTH persisted.
        super::reload_user_config();
        let reloaded = crate::config::VoxConfig::load();
        assert_eq!(
            reloaded.model, "test-org/test-model",
            "sectioned [vox].model must survive the subsequent flat write",
        );
        assert_eq!(
            crate::env_parse::resolve_config_str("OPENROUTER_BASE_URL", "DEFAULT"),
            "https://gateway.example/api",
            "flat OPENROUTER_BASE_URL must persist",
        );

        // Restore env.
        unsafe {
            match prev_data_dir {
                Some(v) => std::env::set_var("VOX_DATA_DIR", v),
                None => std::env::remove_var("VOX_DATA_DIR"),
            }
            if let Some(v) = prev_or {
                std::env::set_var("OPENROUTER_BASE_URL", v);
            }
        }
    }
}
