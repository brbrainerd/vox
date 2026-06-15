//! Tauri commands for the "Runtime" Settings surface — view/edit core user
//! config persisted to `~/.vox/config.toml`.
//!
//! The field catalog is a **view over the `vox-llm-config` SSOT**: `get_user_config`
//! iterates [`vox_config::vox_llm_config::gui_fields`] (every non-secret LLM/AI key)
//! rather than a hand-maintained list, so the GUI can never drift from the registry.
//! Two persistence tiers are routed by each key's `Persistence`:
//!  * **VoxConfig-tier** sectioned fields (`[vox]`/`[train]`/`[db]`) via
//!    [`vox_config::VoxConfig::load`] / `save`.
//!  * **Flat** top-level keys via [`vox_config::toml_config::set_user_config_value`].
//!
//! CACHE-COHERENCE: `VoxConfig::save()` does a direct `fs::write` that bypasses the
//! flat config cache. After any VoxConfig-tier write we call
//! [`vox_config::toml_config::reload_user_config`] so a subsequent flat write reads the
//! sectioned tables back and never clobbers them.

use serde::Serialize;
use tauri::command;
use vox_config::vox_llm_config::{self, Kind, LlmConfigKey, Persistence};

/// One editable config field as presented to the Runtime settings UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfigFieldDto {
    pub key: String,
    pub label: String,
    pub hint: String,
    /// "General" | "Models & endpoints" | "Tuning" | "Training"
    pub group: String,
    /// "string" | "float" | "int" | "path" | "enum" | "bool"
    pub kind: String,
    /// Allowed values when `kind == "enum"`.
    pub options: Vec<String>,
    pub default: String,
    pub current_value: String,
}

/// Look up the registry key, mapping "not found" to an error string for IPC.
fn key_for(key: &str) -> Result<&'static LlmConfigKey, String> {
    vox_llm_config::get(key).ok_or_else(|| format!("unknown config key: {key}"))
}

/// Read the VoxConfig-tier field `key` from `cfg` as a string.
fn voxconfig_value(cfg: &vox_config::VoxConfig, key: &str) -> String {
    match key {
        "model" => cfg.model.clone(),
        "daily_budget_usd" => cfg.daily_budget_usd.to_string(),
        "per_session_budget_usd" => cfg.per_session_budget_usd.to_string(),
        "data_dir" => cfg.data_dir.to_string_lossy().into_owned(),
        "db_url" => cfg.db_url.clone().unwrap_or_default(),
        "train_epochs" => cfg.train_epochs.to_string(),
        "train_batch_size" => cfg.train_batch_size.to_string(),
        _ => String::new(),
    }
}

/// Display default for a key: VoxConfig-tier defaults come from `VoxConfig::default()`
/// (so they never drift from the struct); flat keys use the registry literal default.
fn default_value(spec: &LlmConfigKey) -> String {
    match spec.persistence {
        Persistence::VoxConfig => voxconfig_value(&vox_config::VoxConfig::default(), spec.env),
        Persistence::FlatToml | Persistence::EnvOnly => spec.default.to_string(),
    }
}

/// Generic flat resolver for keys without a dedicated accessor: env > config.toml > default.
fn generic_flat_value(key: &str, default: &str) -> String {
    if let Ok(v) = std::env::var(key) {
        if !v.is_empty() {
            return v;
        }
    }
    let cfg = vox_config::toml_config::load_user_config();
    if let Some(v) = cfg.values.get(key) {
        return v
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| v.to_string());
    }
    default.to_string()
}

/// Current effective value of a flat key (env > config.toml > default), rendered for
/// display. Keys with dedicated accessors use them (precedence + URL sanitization);
/// the rest fall through to [`generic_flat_value`].
fn flat_effective_value(spec: &LlmConfigKey) -> String {
    match spec.env {
        "vox_populi::inference_PROFILE" => {
            // Re-render the resolved enum to its canonical slug.
            match vox_config::inference_profile_from_env() {
                vox_config::InferenceProfile::DesktopOllama => "desktop_ollama",
                vox_config::InferenceProfile::CloudOpenAiCompatible => "cloud_openai_compatible",
                vox_config::InferenceProfile::MobileLitert => "mobile_litert",
                vox_config::InferenceProfile::MobileCoreml => "mobile_coreml",
                vox_config::InferenceProfile::LanGateway => "lan_gateway",
            }
            .to_string()
        }
        "OPENROUTER_BASE_URL" => vox_config::openrouter_base_url(),
        "VOX_OPENAI_BASE_URL" => vox_config::openai_compatible_base_url(),
        "POPULI_URL" => vox_config::local_ollama_populi_base_url(),
        "OLLAMA_TUNING_TEMPERATURE" => vox_config::ollama_tuning_temperature()
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "OLLAMA_TUNING_TOP_P" => vox_config::ollama_tuning_top_p()
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "OLLAMA_TUNING_NUM_CTX" => vox_config::ollama_tuning_num_ctx()
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "OPENAI_TUNING_TEMPERATURE" => vox_config::openai_tuning_temperature()
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "OPENAI_TUNING_TOP_P" => vox_config::openai_tuning_top_p()
            .map(|v| v.to_string())
            .unwrap_or_default(),
        other => generic_flat_value(other, spec.default),
    }
}

/// Build the full catalog from the registry, filling `current_value` from the
/// effective config. Secret keys are excluded (managed via the Keys & Secrets tab).
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over the vox-llm-config view; routing + cache-coherence covered by vox-config tests
pub fn get_user_config() -> Vec<UserConfigFieldDto> {
    let cfg = vox_config::VoxConfig::load();
    vox_llm_config::gui_fields()
        .into_iter()
        .filter_map(|f| {
            let spec = vox_llm_config::get(f.key)?;
            let current_value = match spec.persistence {
                Persistence::VoxConfig => voxconfig_value(&cfg, spec.env),
                Persistence::FlatToml | Persistence::EnvOnly => flat_effective_value(spec),
            };
            Some(UserConfigFieldDto {
                key: f.key.to_string(),
                label: f.label.to_string(),
                hint: f.hint.to_string(),
                group: f.group.to_string(),
                kind: f.kind.to_string(),
                options: f.options.iter().map(|s| (*s).to_string()).collect(),
                default: default_value(spec),
                current_value,
            })
        })
        .collect()
}

/// Validate `value` against the registry key's `kind`, returning the trimmed value.
fn validate(spec: &LlmConfigKey, value: &str) -> Result<String, String> {
    let v = value.trim();
    match spec.kind {
        Kind::Float => {
            let f = v
                .parse::<f64>()
                .map_err(|_| format!("{} must be a number", spec.label))?;
            if !f.is_finite() {
                return Err(format!("{} must be a finite number", spec.label));
            }
            if f < 0.0 {
                return Err(format!("{} must not be negative", spec.label));
            }
            Ok(v.to_string())
        }
        Kind::Int => {
            let i = v
                .parse::<i64>()
                .map_err(|_| format!("{} must be an integer", spec.label))?;
            if i < 0 {
                return Err(format!("{} must not be negative", spec.label));
            }
            Ok(v.to_string())
        }
        Kind::Bool => match v {
            "true" | "false" => Ok(v.to_string()),
            _ => Err(format!("{} must be true or false", spec.label)),
        },
        Kind::Enum => {
            if spec.options.contains(&v) {
                Ok(v.to_string())
            } else {
                Err(format!("{} is not a valid {} value", v, spec.label))
            }
        }
        Kind::String | Kind::Url | Kind::Path => {
            // Non-empty on set; clear optional keys (e.g. db_url) via reset, not an empty set.
            if v.is_empty() {
                return Err(format!("{} must not be empty", spec.label));
            }
            Ok(v.to_string())
        }
    }
}

/// Persist one config field. Routes by persistence tier; refreshes the flat cache
/// after any VoxConfig-tier write to keep both writers coherent.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_config; routing + cache-coherence covered by vox-config tests
pub fn set_user_config(key: String, value: String) -> Result<(), String> {
    let spec = key_for(&key)?;
    let stored = validate(spec, &value)?;

    match spec.persistence {
        Persistence::VoxConfig => {
            // Use the persisted-global config as the save base, NOT `load()`: `save()`
            // writes every field, so an env-folded base would bake transient overrides
            // (VOX_BUDGET_USD / VOX_DATA_DIR / …) permanently into config.toml.
            let mut cfg = vox_config::VoxConfig::load_persisted_global();
            apply_voxconfig_field(&mut cfg, &key, &stored)?;
            cfg.save().map_err(|e| e.to_string())?;
            // VoxConfig::save bypasses the flat cache — refresh it so any later flat
            // write read-modify-writes against the sectioned tables we just wrote.
            vox_config::toml_config::reload_user_config();
        }
        Persistence::FlatToml | Persistence::EnvOnly => {
            vox_config::toml_config::set_user_config_value(&key, &stored)?;
        }
    }
    Ok(())
}

/// Reset one field to its default: unset flat keys; restore VoxConfig-tier fields to
/// `VoxConfig::default()` and re-save.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_config; routing + cache-coherence covered by vox-config tests
pub fn reset_user_config(key: String) -> Result<(), String> {
    let spec = key_for(&key)?;
    match spec.persistence {
        Persistence::VoxConfig => {
            // Persisted-global base (see `set_user_config`): avoid re-persisting env overrides.
            let mut cfg = vox_config::VoxConfig::load_persisted_global();
            let default = vox_config::VoxConfig::default();
            let default_str = voxconfig_value(&default, &key);
            // For db_url the default is "no override"; clear it.
            if key == "db_url" {
                cfg.db_url = None;
            } else {
                apply_voxconfig_field(&mut cfg, &key, &default_str)?;
            }
            cfg.save().map_err(|e| e.to_string())?;
            vox_config::toml_config::reload_user_config();
        }
        Persistence::FlatToml | Persistence::EnvOnly => {
            vox_config::toml_config::unset_user_config_value(&key)?;
        }
    }
    Ok(())
}

/// Mutate one VoxConfig-tier field from its validated string form.
fn apply_voxconfig_field(
    cfg: &mut vox_config::VoxConfig,
    key: &str,
    value: &str,
) -> Result<(), String> {
    match key {
        "model" => cfg.model = value.to_string(),
        "daily_budget_usd" => {
            cfg.daily_budget_usd = value.parse().map_err(|_| "invalid number".to_string())?;
        }
        "per_session_budget_usd" => {
            cfg.per_session_budget_usd = value.parse().map_err(|_| "invalid number".to_string())?;
        }
        "data_dir" => cfg.data_dir = std::path::PathBuf::from(value),
        "db_url" => cfg.db_url = Some(value.to_string()),
        "train_epochs" => {
            cfg.train_epochs = value.parse().map_err(|_| "invalid integer".to_string())?;
        }
        "train_batch_size" => {
            cfg.train_batch_size = value.parse().map_err(|_| "invalid integer".to_string())?;
        }
        _ => return Err(format!("not a VoxConfig field: {key}")),
    }
    Ok(())
}

/// Event name the frontend subscribes to for reactive Runtime-settings refresh.
pub const LLM_CONFIG_CHANGED_EVENT: &str = "vox://llm-config-changed";

/// Payload for [`LLM_CONFIG_CHANGED_EVENT`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfigChanged {
    pub rev: u64,
    pub keys: Vec<String>,
}

/// Spawn once at GUI startup: forward `vox-config` snapshot bumps to the webview as
/// [`LLM_CONFIG_CHANGED_EVENT`], so the Runtime settings surface refreshes reactively
/// when config changes — whether from this GUI, an env reload, or mesh sync.
pub fn spawn_llm_config_bridge(app: tauri::AppHandle) {
    use tauri::Emitter;
    vox_config::snapshot::on_change(move |change| {
        let _ = app.emit(
            LLM_CONFIG_CHANGED_EVENT,
            LlmConfigChanged {
                rev: change.rev,
                keys: change.changed.clone(),
            },
        );
    });
}

/// Recorded LLM spend (actuals) vs the configured budget caps, for the Runtime settings
/// surface. Spend comes from the single SSOT aggregate (`VoxDb::llm_spend_summary`); the
/// caps come from `VoxConfig` — same sources the rest of the system routes/charges on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmSpendDto {
    pub session_usd: f64,
    pub day_usd: f64,
    pub total_usd: f64,
    pub daily_budget_usd: f64,
    pub per_session_budget_usd: f64,
}

/// Read recorded LLM spend (session/day/total) + the budget caps. `session_id` scopes the
/// per-session figure. Returns zeros for spend if the store is unavailable (caps still shown).
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over VoxDb::llm_spend_summary + VoxConfig caps
pub async fn get_llm_spend(session_id: Option<String>) -> Result<LlmSpendDto, String> {
    let cfg = vox_config::VoxConfig::load();
    let spend = match vox_db::VoxDb::connect_canonical().await {
        Ok(db) => db
            .llm_spend_summary(session_id.as_deref())
            .await
            .unwrap_or_default(),
        // No store yet (fresh install / not connected) — surface caps with zero spend.
        Err(_) => Default::default(),
    };
    Ok(LlmSpendDto {
        session_usd: spend.session_usd,
        day_usd: spend.day_usd,
        total_usd: spend.total_usd,
        daily_budget_usd: cfg.daily_budget_usd,
        per_session_budget_usd: cfg.per_session_budget_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(key: &str) -> &'static LlmConfigKey {
        vox_llm_config::get(key).expect("registry key")
    }

    #[test]
    fn spend_dto_serializes_camel_case() {
        let d = LlmSpendDto {
            session_usd: 0.03,
            day_usd: 0.1,
            total_usd: 1.2,
            daily_budget_usd: 5.0,
            per_session_budget_usd: 1.0,
        };
        let j = serde_json::to_string(&d).expect("serialize");
        assert!(j.contains("\"sessionUsd\":0.03"), "{j}");
        assert!(j.contains("\"dailyBudgetUsd\":5.0"), "{j}");
    }

    #[test]
    fn change_event_payload_serializes_camel_case() {
        let p = LlmConfigChanged {
            rev: 3,
            keys: vec!["OPENROUTER_BASE_URL".to_string()],
        };
        let j = serde_json::to_string(&p).expect("serialize");
        assert!(j.contains("\"rev\":3"), "rev field present: {j}");
        assert!(
            j.contains("OPENROUTER_BASE_URL"),
            "changed key present: {j}"
        );
    }

    #[test]
    fn catalog_is_registry_non_secret_view() {
        let cat = get_user_config();
        let reg = vox_llm_config::gui_fields();
        assert_eq!(
            cat.len(),
            reg.len(),
            "GUI catalog must equal registry gui_fields"
        );
        // Secrets must never surface here.
        assert!(
            cat.iter().all(|f| f.key != "OPENROUTER_API_KEY"),
            "secret API keys must not appear in the Runtime catalog"
        );
        // A previously-hidden key is now surfaced.
        assert!(
            cat.iter().any(|f| f.key == "ANTHROPIC_TUNING_TEMPERATURE"),
            "registry-driven catalog should surface formerly-hidden tuning keys"
        );
    }

    #[test]
    fn float_rejects_nan_inf_and_negative() {
        let s = spec("daily_budget_usd");
        assert!(validate(s, "nan").is_err());
        assert!(validate(s, "inf").is_err());
        assert!(validate(s, "-5").is_err());
        assert!(validate(s, "1e999").is_err());
    }

    #[test]
    fn float_accepts_valid_nonnegative() {
        let s = spec("daily_budget_usd");
        assert_eq!(validate(s, " 2.5 ").as_deref(), Ok("2.5"));
        assert_eq!(validate(s, "0").as_deref(), Ok("0"));
    }

    #[test]
    fn int_rejects_negative() {
        let s = spec("train_epochs");
        assert!(validate(s, "-3").is_err());
        assert_eq!(validate(s, "4").as_deref(), Ok("4"));
    }

    #[test]
    fn enum_validates_against_options() {
        let s = spec("vox_populi::inference_PROFILE");
        assert!(validate(s, "desktop_ollama").is_ok());
        assert!(validate(s, "not_a_profile").is_err());
    }

    #[test]
    fn unknown_key_is_rejected() {
        assert!(key_for("NOT_A_REAL_KEY").is_err());
    }
}
