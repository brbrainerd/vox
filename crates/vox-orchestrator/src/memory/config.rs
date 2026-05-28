use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Configuration for the persistent memory system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Tenant account identifier. Scopes `log_dir` and `memory_md_path` to a subdirectory
    /// so agents on different accounts cannot read each other's memory files.
    ///
    /// Sanitized to `[a-zA-Z0-9_-]{1,128}` at construction via [`MemoryConfig::for_account`].
    /// Default: `"global"` (backward-compatible single-account layout).
    #[serde(default = "default_account_id")]
    pub account_id: String,
    /// Directory for daily log files (e.g. `YYYY-MM-DD.md`).
    pub log_dir: PathBuf,
    /// Path to the long-term memory file (`MEMORY.md`).
    pub memory_md_path: PathBuf,
    /// Maximum number of days to retain daily logs. `0` = keep forever.
    pub log_retention_days: u64,
    /// Whether the memory system is enabled (default: true).
    pub enabled: bool,
}

fn default_account_id() -> String {
    "global".to_string()
}

/// Sanitize `account_id` to filesystem-safe characters.
///
/// Allows `[a-zA-Z0-9_-]`, truncates to 128 chars, replaces everything else with `_`.
/// Always returns a non-empty string (falls back to `"global"`).
fn sanitize_account_id(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(128)
        .collect();
    if s.is_empty() {
        "global".to_string()
    } else {
        s
    }
}

impl MemoryConfig {
    /// Construct a config scoped to `account_id` under `base_dir`.
    ///
    /// Layout:
    /// ```text
    /// {base_dir}/{account_id}/MEMORY.md
    /// {base_dir}/{account_id}/logs/YYYY-MM-DD.md
    /// ```
    pub fn for_account(account_id: impl Into<String>, base_dir: impl Into<PathBuf>) -> Self {
        let id = sanitize_account_id(&account_id.into());
        let base: PathBuf = base_dir.into();
        let account_dir = base.join(&id);
        Self {
            account_id: id,
            log_dir: account_dir.join("logs"),
            memory_md_path: account_dir.join("MEMORY.md"),
            log_retention_days: 30,
            enabled: true,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self::for_account("global", vox_config::paths::REPO_MEMORY_DIR)
    }
}
