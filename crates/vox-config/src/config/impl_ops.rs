//! Load / merge / persist logic for [`VoxConfig`](super::vox_config::VoxConfig).
//!
//! **Figment pilot (narrow):** precedence remains explicit (`defaults` → global TOML → repo `Vox.toml` → env)
//! in [`VoxConfig::load`] / [`VoxConfig::load_from_repo_root`]. A future step can centralize the same
//! ordering via [`figment`](https://docs.rs/figment) without changing field semantics — keep call sites stable until tests cover merge parity.

use std::path::{Path, PathBuf};

use super::gamify_web::{BuildTarget, GamifyMode, WebRunMode};
use super::persist::{global_config_path, save_merged_global_config};
use super::toml_schema::VoxToml;
use super::vox_config::VoxConfig;

impl VoxConfig {
    /// Load config applying the full precedence chain:
    /// ENV VARS > Vox.toml (workspace) > ~/.vox/config.toml (global) > defaults
    pub fn load() -> Self {
        let mut cfg = Self::default();

        if let Some(global_path) = global_config_path() {
            cfg.apply_toml_file(&global_path);
        }

        cfg.apply_toml_file(Path::new("Vox.toml"));
        cfg.apply_env();

        cfg
    }

    /// Load like [`Self::load`], but apply `repo_root.join("Vox.toml")` before the process-CWD `Vox.toml`.
    pub fn load_from_repo_root(repo_root: &Path) -> Self {
        let mut cfg = Self::default();
        if let Some(global_path) = global_config_path() {
            cfg.apply_toml_file(&global_path);
        }
        cfg.apply_toml_file(&repo_root.join("Vox.toml"));
        cfg.apply_toml_file(Path::new("Vox.toml"));
        cfg.apply_env();
        cfg
    }

    /// Get the value of a config key by name. Used by the `vox_config_get` MCP tool (alias `vox_get_config`).
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
            "gamify_enabled" | "gamify.enabled" => Some(self.gamify_enabled.to_string()),
            "gamify_mode" | "gamify.mode" => Some(self.gamify_mode.as_config_str().to_string()),
            "web_run_mode" | "web.run_mode" => Some(self.web_run_mode.as_config_str().to_string()),
            "web_tanstack_start" | "web.tanstack_start" => {
                Some(self.web_tanstack_start.to_string())
            }
            _ => None,
        }
    }

    /// Set a config key at runtime (does not persist by itself — call [`Self::save`] after mutating,
    /// or use `vox config set` in the CLI).
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
            "gamify_enabled" | "gamify.enabled" => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" => self.gamify_enabled = true,
                "false" | "0" | "no" => self.gamify_enabled = false,
                _ => return false,
            },
            "gamify_mode" | "gamify.mode" => {
                self.gamify_mode = match value.to_lowercase().as_str() {
                    "balanced" => GamifyMode::Balanced,
                    "serious" => GamifyMode::Serious,
                    "learning" => GamifyMode::Learning,
                    _ => return false,
                };
            }
            "web.run_mode" | "web_run_mode" => {
                self.web_run_mode = match value.to_lowercase().as_str() {
                    "app" => WebRunMode::App,
                    "script" => WebRunMode::Script,
                    "auto" => WebRunMode::Auto,
                    _ => return false,
                };
            }
            "web.tanstack_start" | "web_tanstack_start" => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" => self.web_tanstack_start = true,
                "false" | "0" | "no" => self.web_tanstack_start = false,
                _ => return false,
            },
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
            "gamify_enabled",
            "gamify_mode",
            "gamify.enabled",
            "gamify.mode",
            "web.run_mode",
            "web_run_mode",
            "web.tanstack_start",
            "web_tanstack_start",
        ]
    }

    /// Writes `[vox]`, `[train]`, and optionally `[db].url` to `~/.vox/config.toml`.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = global_config_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "cannot resolve ~/.vox data directory",
            ));
        };
        save_merged_global_config(&path, self)
    }

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
            if let Some(v) = vox.gamify_enabled {
                self.gamify_enabled = v;
            }
            if let Some(v) = vox.gamify_mode {
                self.gamify_mode = v;
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

        if let Some(db) = parsed.db
            && let Some(v) = db.url
        {
            self.db_url = Some(v);
        }

        if let Some(web) = parsed.web {
            if let Some(v) = web.run_mode {
                self.web_run_mode = v;
            }
            if let Some(v) = web.tanstack_start {
                self.web_tanstack_start = v;
            }
        }

        if let Some(build) = parsed.build
            && let Some(v) = build.target
        {
            self.build_target = v;
        }
    }

    fn apply_env(&mut self) {
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxModel).expose() {
            self.model = v.to_string();
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBudgetUsd).expose()
            && let Ok(f) = v.parse()
        {
            self.daily_budget_usd = f;
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxDataDir).expose() {
            self.data_dir = PathBuf::from(v.to_string());
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxDbUrl).expose() {
            self.db_url = Some(v.to_string());
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpBinary).expose() {
            self.mcp_binary = Some(PathBuf::from(v.to_string()));
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGamifyEnabled).expose()
        {
            let low = v.to_lowercase();
            self.gamify_enabled = matches!(low.as_str(), "1" | "true" | "yes");
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGamifyMode).expose()
        {
            self.gamify_mode = match v.to_lowercase().as_str() {
                "serious" => GamifyMode::Serious,
                "learning" => GamifyMode::Learning,
                _ => GamifyMode::Balanced,
            };
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxWebRunMode).expose()
        {
            self.web_run_mode = match v.to_lowercase().as_str() {
                "app" => WebRunMode::App,
                "script" => WebRunMode::Script,
                _ => WebRunMode::Auto,
            };
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxWebTanstackStart).expose()
        {
            let low = v.to_lowercase();
            self.web_tanstack_start = matches!(low.as_str(), "1" | "true" | "yes");
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey).expose()
        {
            self.openrouter_key = Some(v.to_string());
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiApiKey).expose() {
            self.openai_key = Some(v.to_string());
        }
        if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiApiKey).expose() {
            self.gemini_key = Some(v.to_string());
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicApiKey).expose()
        {
            self.anthropic_key = Some(v.to_string());
        }

        // Non-secret build override (documented on `BuildTarget` in `gamify_web.rs`).
        // Must run after `Vox.toml` merges so env beats manifest when both are set.
        Self::merge_build_target_from_env_var(self);
    }

    /// Override `build_target` from `VOX_BUILD_TARGET` (`fullstack` | `server` | `client`).
    pub(crate) fn merge_build_target_from_env_var(cfg: &mut VoxConfig) {
        let Ok(raw) = std::env::var("VOX_BUILD_TARGET") else {
            return;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(t) = trimmed.parse::<BuildTarget>() {
            cfg.build_target = t;
        }
    }
}

#[cfg(test)]
fn merge_vox_toml_path_for_test(cfg: &mut VoxConfig, path: &Path) {
    cfg.apply_toml_file(path);
}

#[cfg(test)]
// Rust 2024 made `std::env::set_var` / `remove_var` `unsafe`. The
// `vox_build_target_env_overrides_toml_after_merge` test serializes via
// `VOX_BUILD_TARGET_ENV_MUTEX` (declared in this module).
#[allow(unsafe_code)]
mod tests {
    use super::super::gamify_web::BuildTarget;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn save_merges_and_preserves_unknown_and_optional_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let initial = r#"[registry]
url = "https://reg.example"

[vox]
model = "m1"
daily_budget_usd = 3.0
per_session_budget_usd = 1.5
gamify_enabled = true
gamify_mode = "balanced"
mcp_binary = "/keep/mcp"
future_vox = "pv"

[train]
data_dir = "d1"
model_dir = "md1"
epochs = 2
batch_size = 8
legacy_train = true

[db]
url = "https://db.example"
db_extra = "de"
"#;
        std::fs::write(&path, initial).expect("write");

        let cfg = VoxConfig {
            model: "m1".into(),
            daily_budget_usd: 3.0,
            per_session_budget_usd: 1.5,
            gamify_enabled: true,
            gamify_mode: GamifyMode::Learning,
            data_dir: PathBuf::from("d1"),
            model_dir: PathBuf::from("md1"),
            train_epochs: 2,
            train_batch_size: 8,
            mcp_binary: None,
            db_url: None,
            ..VoxConfig::default()
        };

        super::super::persist::save_merged_global_config(&path, &cfg).expect("save");

        let text = std::fs::read_to_string(&path).expect("read");
        let parsed: toml::Value = toml::from_str(&text).expect("parse");

        let root = parsed.as_table().expect("root table");
        assert_eq!(
            root.get("registry")
                .and_then(|v| v.get("url"))
                .and_then(toml::Value::as_str),
            Some("https://reg.example")
        );

        let vox = root
            .get("vox")
            .and_then(toml::Value::as_table)
            .expect("vox");
        assert_eq!(
            vox.get("future_vox").and_then(toml::Value::as_str),
            Some("pv")
        );
        assert_eq!(
            vox.get("mcp_binary").and_then(toml::Value::as_str),
            Some("/keep/mcp")
        );
        assert_eq!(
            vox.get("gamify_mode").and_then(toml::Value::as_str),
            Some("learning")
        );

        let train = root
            .get("train")
            .and_then(toml::Value::as_table)
            .expect("train");
        assert_eq!(
            train.get("legacy_train").and_then(toml::Value::as_bool),
            Some(true)
        );

        let db = root.get("db").and_then(toml::Value::as_table).expect("db");
        assert_eq!(
            db.get("url").and_then(toml::Value::as_str),
            Some("https://db.example")
        );
        assert_eq!(db.get("db_extra").and_then(toml::Value::as_str), Some("de"));
    }

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = VoxConfig::default();
        assert!(!cfg.model.is_empty());
        assert!(cfg.daily_budget_usd > 0.0);
        assert!(cfg.train_epochs > 0);
        assert!(cfg.train_batch_size > 0);
        assert_eq!(cfg.web_run_mode, WebRunMode::Auto);
        assert!(!cfg.web_tanstack_start);
    }

    #[test]
    fn reads_web_run_mode_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[web]\nrun_mode = \"app\"\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert_eq!(cfg.web_run_mode, WebRunMode::App);
    }

    #[test]
    fn web_run_mode_set_key_roundtrip() {
        let mut cfg = VoxConfig::default();
        assert!(cfg.set_key("web.run_mode", "script"));
        assert_eq!(cfg.web_run_mode, WebRunMode::Script);
        assert_eq!(cfg.get_key("web.run_mode").as_deref(), Some("script"));
    }

    #[test]
    fn reads_web_tanstack_start_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[web]\ntanstack_start = true\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert!(cfg.web_tanstack_start);
    }

    #[test]
    fn web_tanstack_start_set_key_roundtrip() {
        let mut cfg = VoxConfig::default();
        assert!(cfg.set_key("web.tanstack_start", "true"));
        assert!(cfg.web_tanstack_start);
        assert_eq!(cfg.get_key("web.tanstack_start").as_deref(), Some("true"));
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

    // ── BuildTarget tests ────────────────────────────────────────────────────

    #[test]
    fn build_target_from_str_parses_all_variants() {
        assert_eq!(
            "fullstack".parse::<BuildTarget>().unwrap(),
            BuildTarget::Fullstack
        );
        assert_eq!(
            "server".parse::<BuildTarget>().unwrap(),
            BuildTarget::Server
        );
        assert_eq!(
            "client".parse::<BuildTarget>().unwrap(),
            BuildTarget::Client
        );
        // case-insensitive
        assert_eq!(
            "SERVER".parse::<BuildTarget>().unwrap(),
            BuildTarget::Server
        );
        assert_eq!(
            "  Fullstack ".parse::<BuildTarget>().unwrap(),
            BuildTarget::Fullstack
        );
    }

    #[test]
    fn build_target_from_str_unknown_is_none() {
        assert!("".parse::<BuildTarget>().is_err());
        assert!("ios".parse::<BuildTarget>().is_err());
        assert!("backend".parse::<BuildTarget>().is_err());
    }

    #[test]
    fn build_target_default_is_fullstack() {
        assert_eq!(BuildTarget::default(), BuildTarget::Fullstack);
    }

    #[test]
    fn reads_build_target_server_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[build]\ntarget = \"server\"\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert_eq!(cfg.build_target, BuildTarget::Server);
    }

    #[test]
    fn reads_build_target_fullstack_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[build]\ntarget = \"fullstack\"\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert_eq!(cfg.build_target, BuildTarget::Fullstack);
    }

    #[test]
    fn build_target_defaults_to_fullstack_when_build_section_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[web]\nrun_mode = \"app\"\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert_eq!(cfg.build_target, BuildTarget::Fullstack);
    }

    static VOX_BUILD_TARGET_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn vox_build_target_env_overrides_toml_after_merge() {
        let _lock = VOX_BUILD_TARGET_ENV_MUTEX.lock().expect("serial env test");
        let prev = std::env::var("VOX_BUILD_TARGET").ok();
        unsafe {
            std::env::set_var("VOX_BUILD_TARGET", "client");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        std::fs::write(&p, "[build]\ntarget = \"server\"\n").expect("write");
        let mut cfg = VoxConfig::default();
        merge_vox_toml_path_for_test(&mut cfg, &p);
        assert_eq!(cfg.build_target, BuildTarget::Server);
        VoxConfig::merge_build_target_from_env_var(&mut cfg);
        assert_eq!(cfg.build_target, BuildTarget::Client);
        match prev {
            Some(v) => unsafe {
                std::env::set_var("VOX_BUILD_TARGET", v);
            },
            None => unsafe {
                std::env::remove_var("VOX_BUILD_TARGET");
            },
        }
    }

    #[test]
    fn to_map_contains_all_known_keys_that_have_values() {
        let cfg = VoxConfig::default();
        let map = cfg.to_map();
        assert!(map.contains_key("model"));
        assert!(map.contains_key("daily_budget_usd"));
        assert!(map.contains_key("train_epochs"));
    }
}
