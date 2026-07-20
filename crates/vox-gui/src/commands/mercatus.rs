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

/// The first-run state: no config saved yet. Not an error — a fresh
/// install/worktree has nothing at `config_path()`, and reporting that as a
/// raw OS "file not found" (previously surfaced verbatim to the user) reads
/// as a broken app rather than an empty watchlist waiting to be filled in.
fn empty_config() -> serde_json::Value {
    serde_json::json!({ "_meta": { "sources": {} }, "watchlist": [] })
}

#[command]
pub fn mercatus_load_config() -> Result<serde_json::Value, String> {
    let path = config_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(empty_config());
        }
        Err(e) => return Err(format!("Cannot read {}: {e}", path.display())),
    };
    serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_returns_empty_watchlist_on_first_run_not_an_error() {
        // config_path() is env-var-overridable; point it at a file that
        // genuinely does not exist rather than touching the real user config dir.
        let dir =
            std::env::temp_dir().join(format!("vox-gui-mercatus-test-{}", std::process::id()));
        // SAFETY: test-only, single-threaded within this test's scope.
        unsafe {
            std::env::set_var("PRICE_WATCH_CONFIG", dir.join("does-not-exist.json"));
        }
        let result = mercatus_load_config();
        unsafe {
            std::env::remove_var("PRICE_WATCH_CONFIG");
        }
        assert_eq!(result, Ok(empty_config()));
    }
}

#[command]
pub fn mercatus_save_config(config: serde_json::Value) -> Result<(), String> {
    let path = config_path();
    let text =
        serde_json::to_string_pretty(&config).map_err(|e| format!("JSON serialize error: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}
