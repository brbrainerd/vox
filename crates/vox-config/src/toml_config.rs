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

/// Persist a key-value pair to `~/.vox/config.toml` atomically.
pub fn set_user_config_value(key: &str, value: &str) -> Result<(), String> {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let mut guard = cache.lock().expect("config cache mutex poisoned");

    guard
        .values
        .insert(key.to_string(), toml::Value::String(value.to_string()));

    let toml_str =
        toml::to_string(&*guard).map_err(|e| format!("Failed to serialize config: {e}"))?;

    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }

    fs::write(&path, toml_str).map_err(|e| format!("Failed to write config file: {e}"))?;

    Ok(())
}

/// Remove a key from `~/.vox/config.toml`.
pub fn unset_user_config_value(key: &str) -> Result<bool, String> {
    let cache = CONFIG_CACHE.get_or_init(initialize_cache);
    let mut guard = cache.lock().expect("config cache mutex poisoned");

    if guard.values.remove(key).is_some() {
        let toml_str =
            toml::to_string(&*guard).map_err(|e| format!("Failed to serialize config: {e}"))?;

        let path = get_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
        }

        fs::write(&path, toml_str).map_err(|e| format!("Failed to write config file: {e}"))?;

        Ok(true)
    } else {
        Ok(false)
    }
}
