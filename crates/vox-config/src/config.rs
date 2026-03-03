//! `VoxConfig` — Single Source of Truth for all Vox toolchain settings.
//!
//! Precedence (highest → lowest):
//!   ENV VARS > Vox.toml (workspace) > ~/.vox/config.toml (global) > compiled defaults
//!
//! CLI flags must be applied by the caller *after* calling `VoxConfig::load()`.
//! See: `docs/agents/config-hierarchy.md`

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Full Vox toolchain configuration.
///
/// Load via `VoxConfig::load()` which applies the full precedence chain.
/// Do not construct manually outside tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoxConfig {
    // ── Provider / Model ──────────────────────────────────────────────────
    pub model: String,
    pub openrouter_key: Option<String>,
    pub openai_key: Option<String>,
    pub gemini_key: Option<String>,
    pub anthropic_key: Option<String>,

    // ── Budget ───────────────────────────────────────────────────────────
    pub daily_budget_usd: f64,
    pub per_session_budget_usd: f64,

    // ── Data paths ────────────────────────────────────────────────────────
    pub data_dir: PathBuf,
    pub model_dir: PathBuf,

    // ── Training ─────────────────────────────────────────────────────────
    pub train_epochs: usize,
    pub train_batch_size: usize,

    // ── Orchestrator ─────────────────────────────────────────────────────
    pub mcp_binary: Option<PathBuf>,
    pub db_url: Option<String>,
}

impl Default for VoxConfig {
    fn default() -> Self {
        Self {
            model: "anthropic/claude-sonnet-4".to_string(),
            openrouter_key: None,
            openai_key: None,
            gemini_key: None,
            anthropic_key: None,
            daily_budget_usd: 5.0,
            per_session_budget_usd: 1.0,
            data_dir: PathBuf::from("target/dogfood"),
            model_dir: crate::paths::data_dir()
                .map(|d| d.join("models"))
                .unwrap_or_else(|| PathBuf::from(".vox/models")),
            train_epochs: 3,
            train_batch_size: 256,
            mcp_binary: None,
            db_url: None,
        }
    }
}

/// TOML file schema — partial subset of VoxConfig fields.
/// Fields you don't set will fall through to lower-precedence layers.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct VoxToml {
    vox: Option<VoxTomlSection>,
    train: Option<TrainTomlSection>,
    db: Option<DbTomlSection>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct VoxTomlSection {
    model: Option<String>,
    daily_budget_usd: Option<f64>,
    per_session_budget_usd: Option<f64>,
    mcp_binary: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TrainTomlSection {
    data_dir: Option<PathBuf>,
    model_dir: Option<PathBuf>,
    epochs: Option<usize>,
    batch_size: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct DbTomlSection {
    url: Option<String>,
}

impl VoxConfig {
    /// Load config applying the full precedence chain:
    /// ENV VARS > Vox.toml (workspace) > ~/.vox/config.toml (global) > defaults
    pub fn load() -> Self {
        let mut cfg = Self::default();

        // 3. Apply global user config (~/.vox/config.toml)
        if let Some(global_path) = global_config_path() {
            cfg.apply_toml_file(&global_path);
        }

        // 2. Apply workspace Vox.toml (overrides global)
        cfg.apply_toml_file(Path::new("Vox.toml"));

        // 1. Apply ENV VARS (highest precedence short of CLI flags)
        cfg.apply_env();

        cfg
    }

    /// Get the value of a config key by name. Used by `vox_get_config` MCP tool.
    pub fn get_key(&self, key: &str) -> Option<String> {
        match key {
            "model" => Some(self.model.clone()),
            "daily_budget_usd" => Some(self.daily_budget_usd.to_string()),
            "per_session_budget_usd" => Some(self.per_session_budget_usd.to_string()),
            "data_dir" => Some(self.data_dir.display().to_string()),
            "model_dir" => Some(self.model_dir.display().to_string()),
            "train_epochs" => Some(self.train_epochs.to_string()),
            "train_batch_size" => Some(self.train_batch_size.to_string()),
            "db_url" => self.db_url.clone(),
            "mcp_binary" => self.mcp_binary.as_ref().map(|p| p.display().to_string()),
            _ => None,
        }
    }

    /// Set a config key at runtime (does not persist — use `vox config set` for that).
    pub fn set_key(&mut self, key: &str, value: &str) -> bool {
        match key {
            "model" => self.model = value.to_string(),
            "daily_budget_usd" => {
                if let Ok(v) = value.parse() {
                    self.daily_budget_usd = v;
                }
            }
            "per_session_budget_usd" => {
                if let Ok(v) = value.parse() {
                    self.per_session_budget_usd = v;
                }
            }
            "db_url" => self.db_url = Some(value.to_string()),
            "data_dir" => self.data_dir = PathBuf::from(value),
            "model_dir" => self.model_dir = PathBuf::from(value),
            "train_epochs" => {
                if let Ok(v) = value.parse() {
                    self.train_epochs = v;
                }
            }
            "train_batch_size" => {
                if let Ok(v) = value.parse() {
                    self.train_batch_size = v;
                }
            }
            _ => return false,
        }
        true
    }

    /// Returns all config keys and their current values for display/MCP.
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        for key in Self::known_keys() {
            if let Some(v) = self.get_key(key) {
                m.insert(key.to_string(), v);
            }
        }
        m
    }

    /// All supported config key names.
    pub fn known_keys() -> &'static [&'static str] {
        &[
            "model",
            "daily_budget_usd",
            "per_session_budget_usd",
            "data_dir",
            "model_dir",
            "train_epochs",
            "train_batch_size",
            "db_url",
            "mcp_binary",
        ]
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn apply_toml_file(&mut self, path: &Path) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(parsed) = toml::from_str::<VoxToml>(&text) else {
            return;
        };

        if let Some(vox) = parsed.vox {
            if let Some(v) = vox.model {
                self.model = v;
            }
            if let Some(v) = vox.daily_budget_usd {
                self.daily_budget_usd = v;
            }
            if let Some(v) = vox.per_session_budget_usd {
                self.per_session_budget_usd = v;
            }
            if let Some(v) = vox.mcp_binary {
                self.mcp_binary = Some(v);
            }
        }

        if let Some(train) = parsed.train {
            if let Some(v) = train.data_dir {
                self.data_dir = v;
            }
            if let Some(v) = train.model_dir {
                self.model_dir = v;
            }
            if let Some(v) = train.epochs {
                self.train_epochs = v;
            }
            if let Some(v) = train.batch_size {
                self.train_batch_size = v;
            }
        }

        if let Some(db) = parsed.db {
            if let Some(v) = db.url {
                self.db_url = Some(v);
            }
        }
    }

    fn apply_env(&mut self) {
        if let Ok(v) = std::env::var("VOX_MODEL") {
            if !v.is_empty() {
                self.model = v;
            }
        }
        if let Ok(v) = std::env::var("VOX_BUDGET_USD") {
            if let Ok(f) = v.parse() {
                self.daily_budget_usd = f;
            }
        }
        if let Ok(v) = std::env::var("VOX_DATA_DIR") {
            if !v.is_empty() {
                self.data_dir = PathBuf::from(v);
            }
        }
        if let Ok(v) = std::env::var("VOX_DB_URL") {
            if !v.is_empty() {
                self.db_url = Some(v);
            }
        }
        if let Ok(v) = std::env::var("VOX_MCP_BINARY") {
            if !v.is_empty() {
                self.mcp_binary = Some(PathBuf::from(v));
            }
        }
        // API keys
        if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
            if !v.is_empty() {
                self.openrouter_key = Some(v);
            }
        }
        if let Ok(v) = std::env::var("OPENAI_API_KEY") {
            if !v.is_empty() {
                self.openai_key = Some(v);
            }
        }
        if let Ok(v) = std::env::var("GEMINI_API_KEY") {
            if !v.is_empty() {
                self.gemini_key = Some(v);
            }
        }
        if let Ok(v) = std::env::var("ANTHROPIC_API_KEY") {
            if !v.is_empty() {
                self.anthropic_key = Some(v);
            }
        }
    }
}

fn global_config_path() -> Option<PathBuf> {
    crate::paths::data_dir().map(|d| d.join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = VoxConfig::default();
        assert!(!cfg.model.is_empty());
        assert!(cfg.daily_budget_usd > 0.0);
        assert!(cfg.train_epochs > 0);
        assert!(cfg.train_batch_size > 0);
    }

    #[test]
    fn get_set_roundtrip() {
        let mut cfg = VoxConfig::default();
        assert!(cfg.set_key("model", "openai/gpt-4o"));
        assert_eq!(cfg.get_key("model").as_deref(), Some("openai/gpt-4o"));
    }

    #[test]
    fn set_unknown_key_returns_false() {
        let mut cfg = VoxConfig::default();
        assert!(!cfg.set_key("nonexistent", "value"));
    }

    #[test]
    fn to_map_contains_all_known_keys_that_have_values() {
        let cfg = VoxConfig::default();
        let map = cfg.to_map();
        // model, daily_budget_usd, etc. must all be present
        assert!(map.contains_key("model"));
        assert!(map.contains_key("daily_budget_usd"));
        assert!(map.contains_key("train_epochs"));
    }
}
