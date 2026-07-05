use std::{env, fs, path::PathBuf};
use tauri::command;

fn config_path() -> PathBuf {
    if let Ok(p) = env::var("PRICE_WATCH_CONFIG") {
        return PathBuf::from(p);
    }
    // Default: <user config dir>/storage-tier/price-watch/price-watch.config.json.
    // Falls back to the current directory if the OS config dir cannot be resolved
    // (dirs::config_dir() only returns None on exotic platforms).
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("storage-tier")
        .join("price-watch")
        .join("price-watch.config.json")
}

#[command]
pub fn mercatus_load_config() -> Result<serde_json::Value, String> {
    let path = config_path();
    let text =
        fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}"))
}

#[command]
pub fn mercatus_save_config(config: serde_json::Value) -> Result<(), String> {
    let path = config_path();
    let text =
        serde_json::to_string_pretty(&config).map_err(|e| format!("JSON serialize error: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}
