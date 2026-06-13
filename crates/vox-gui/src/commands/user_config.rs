//! Tauri commands for the "Runtime" Settings surface — view/edit core user
//! config persisted to `~/.vox/config.toml`.
//!
//! Two config tiers are exposed through one flat catalog:
//!  * **VoxConfig-tier** sectioned fields (`[vox]`/`[train]`/`[db]`) routed through
//!    [`vox_config::VoxConfig::load`] / `save`.
//!  * **inference-tier** flat top-level keys routed through
//!    [`vox_config::toml_config::set_user_config_value`].
//!
//! CACHE-COHERENCE: `VoxConfig::save()` does a direct `fs::write` that bypasses the
//! flat config cache. After any VoxConfig-tier write we call
//! [`vox_config::toml_config::reload_user_config`] so a subsequent flat write reads the
//! sectioned tables back and never clobbers them. The flat writers themselves
//! read-modify-write the file fresh, so neither tier clobbers the other.

use serde::Serialize;
use tauri::command;

/// One editable config field as presented to the Runtime settings UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfigFieldDto {
    pub key: String,
    pub label: String,
    pub hint: String,
    /// "General" | "Models & endpoints" | "Tuning" | "Training"
    pub group: String,
    /// "string" | "float" | "int" | "path" | "enum"
    pub kind: String,
    /// Allowed values when `kind == "enum"`.
    pub options: Vec<String>,
    pub default: String,
    pub current_value: String,
}

/// Which persistence tier a key belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    /// Sectioned `VoxConfig` field routed through `VoxConfig::load()`/`save()`.
    VoxConfig,
    /// Flat top-level key routed through `set_user_config_value`.
    Flat,
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    String,
    Float,
    Int,
    Path,
    Enum,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Float => "float",
            Kind::Int => "int",
            Kind::Path => "path",
            Kind::Enum => "enum",
        }
    }
}

/// Static metadata for one field (everything except `current_value`).
struct FieldSpec {
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    group: &'static str,
    kind: Kind,
    options: &'static [&'static str],
    tier: Tier,
}

const PROFILE_OPTIONS: &[&str] = &[
    "desktop_ollama",
    "cloud_openai_compatible",
    "mobile_litert",
    "mobile_coreml",
    "lan_gateway",
];

/// The full catalog. `default` values are computed at read time so they track the
/// live `VoxConfig::default()` / accessor defaults.
const FIELDS: &[FieldSpec] = &[
    // ---- General (VoxConfig-tier) -------------------------------------------------
    FieldSpec {
        key: "model",
        label: "Default model",
        hint: "Model id used when no per-call override is set",
        group: "General",
        kind: Kind::String,
        options: &[],
        tier: Tier::VoxConfig,
    },
    FieldSpec {
        key: "daily_budget_usd",
        label: "Daily budget (USD)",
        hint: "Soft cap on spend per day",
        group: "General",
        kind: Kind::Float,
        options: &[],
        tier: Tier::VoxConfig,
    },
    FieldSpec {
        key: "per_session_budget_usd",
        label: "Per-session budget (USD)",
        hint: "Soft cap on spend per session",
        group: "General",
        kind: Kind::Float,
        options: &[],
        tier: Tier::VoxConfig,
    },
    FieldSpec {
        key: "data_dir",
        label: "Training data dir",
        hint: "Directory for MENS training data ([train].data_dir); does NOT relocate the app's runtime data",
        group: "General",
        kind: Kind::Path,
        options: &[],
        tier: Tier::VoxConfig,
    },
    FieldSpec {
        key: "db_url",
        label: "Database URL",
        hint: "Optional external database URL (blank = local default)",
        group: "General",
        kind: Kind::String,
        options: &[],
        tier: Tier::VoxConfig,
    },
    // ---- Models & endpoints (inference-tier flat keys) ----------------------------
    FieldSpec {
        key: "vox_populi::inference_PROFILE",
        label: "Inference profile",
        hint: "Where chat/completion traffic runs",
        group: "Models & endpoints",
        kind: Kind::Enum,
        options: PROFILE_OPTIONS,
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "OPENROUTER_BASE_URL",
        label: "OpenRouter base URL",
        hint: "OpenAI-compatible OpenRouter endpoint",
        group: "Models & endpoints",
        kind: Kind::String,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "VOX_OPENAI_BASE_URL",
        label: "OpenAI base URL",
        hint: "OpenAI-compatible cloud endpoint",
        group: "Models & endpoints",
        kind: Kind::String,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "POPULI_URL",
        label: "Local Ollama base URL",
        hint: "Local Ollama / Populi HTTP endpoint",
        group: "Models & endpoints",
        kind: Kind::String,
        options: &[],
        tier: Tier::Flat,
    },
    // ---- Tuning (inference-tier flat keys) ----------------------------------------
    FieldSpec {
        key: "OLLAMA_TUNING_TEMPERATURE",
        label: "Ollama temperature",
        hint: "Sampling temperature for local Ollama",
        group: "Tuning",
        kind: Kind::Float,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "OLLAMA_TUNING_TOP_P",
        label: "Ollama top-p",
        hint: "Nucleus sampling cutoff for local Ollama",
        group: "Tuning",
        kind: Kind::Float,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "OLLAMA_TUNING_NUM_CTX",
        label: "Ollama context window",
        hint: "num_ctx tokens for local Ollama",
        group: "Tuning",
        kind: Kind::Int,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "OPENAI_TUNING_TEMPERATURE",
        label: "OpenAI temperature",
        hint: "Sampling temperature for OpenAI-compatible cloud",
        group: "Tuning",
        kind: Kind::Float,
        options: &[],
        tier: Tier::Flat,
    },
    FieldSpec {
        key: "OPENAI_TUNING_TOP_P",
        label: "OpenAI top-p",
        hint: "Nucleus sampling cutoff for OpenAI-compatible cloud",
        group: "Tuning",
        kind: Kind::Float,
        options: &[],
        tier: Tier::Flat,
    },
    // ---- Training (VoxConfig-tier) ------------------------------------------------
    FieldSpec {
        key: "train_epochs",
        label: "Training epochs",
        hint: "Default epochs for MENS training runs",
        group: "Training",
        kind: Kind::Int,
        options: &[],
        tier: Tier::VoxConfig,
    },
    FieldSpec {
        key: "train_batch_size",
        label: "Training batch size",
        hint: "Default batch size for MENS training runs",
        group: "Training",
        kind: Kind::Int,
        options: &[],
        tier: Tier::VoxConfig,
    },
];

fn spec_for(key: &str) -> Result<&'static FieldSpec, String> {
    FIELDS
        .iter()
        .find(|f| f.key == key)
        .ok_or_else(|| format!("unknown config key: {key}"))
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

/// Default value for `key` as a display string (from `VoxConfig::default()` or the
/// inference accessor defaults).
fn default_value(key: &str) -> String {
    let d = vox_config::VoxConfig::default();
    match key {
        // VoxConfig-tier
        "model"
        | "daily_budget_usd"
        | "per_session_budget_usd"
        | "data_dir"
        | "db_url"
        | "train_epochs"
        | "train_batch_size" => voxconfig_value(&d, key),
        // inference-tier flat keys (defaults mirror the resolver fallbacks)
        "vox_populi::inference_PROFILE" => "desktop_ollama".to_string(),
        "OPENROUTER_BASE_URL" => "https://openrouter.ai/api".to_string(),
        "VOX_OPENAI_BASE_URL" => "https://api.openai.com/v1".to_string(),
        "POPULI_URL" => "http://localhost:11434".to_string(),
        // tuning defaults are "unset" → empty
        _ => String::new(),
    }
}

/// Current effective value of a flat inference-tier key (env > config.toml > default),
/// rendered for display.
fn flat_effective_value(key: &str) -> String {
    match key {
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
        _ => String::new(),
    }
}

/// Build the full catalog, filling `current_value` from the effective config.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_config; routing + cache-coherence covered by vox-config tests
pub fn get_user_config() -> Vec<UserConfigFieldDto> {
    let cfg = vox_config::VoxConfig::load();
    FIELDS
        .iter()
        .map(|f| {
            let current_value = match f.tier {
                Tier::VoxConfig => voxconfig_value(&cfg, f.key),
                Tier::Flat => flat_effective_value(f.key),
            };
            UserConfigFieldDto {
                key: f.key.to_string(),
                label: f.label.to_string(),
                hint: f.hint.to_string(),
                group: f.group.to_string(),
                kind: f.kind.as_str().to_string(),
                options: f.options.iter().map(|s| s.to_string()).collect(),
                default: default_value(f.key),
                current_value,
            }
        })
        .collect()
}

/// Validate `value` against the field's `kind`, returning the trimmed value to store.
fn validate(spec: &FieldSpec, value: &str) -> Result<String, String> {
    let v = value.trim();
    match spec.kind {
        Kind::Float => {
            if v.is_empty() {
                return Err(format!("{} must be a number", spec.label));
            }
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
            if v.is_empty() {
                return Err(format!("{} must be an integer", spec.label));
            }
            let i = v
                .parse::<i64>()
                .map_err(|_| format!("{} must be an integer", spec.label))?;
            if i < 0 {
                return Err(format!("{} must not be negative", spec.label));
            }
            Ok(v.to_string())
        }
        Kind::Enum => {
            if spec.options.contains(&v) {
                Ok(v.to_string())
            } else {
                Err(format!("{} is not a valid {} value", v, spec.label))
            }
        }
        Kind::String | Kind::Path => {
            // URL-ish endpoint keys must be non-empty when this tier requires a value;
            // db_url is genuinely optional and may be cleared via reset, so a non-empty
            // set is still required here (use reset to clear).
            if v.is_empty() {
                return Err(format!("{} must not be empty", spec.label));
            }
            Ok(v.to_string())
        }
    }
}

/// Persist one config field. Routes by tier; refreshes the flat cache after any
/// VoxConfig-tier write to keep both writers coherent.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_config; routing + cache-coherence covered by vox-config tests
pub fn set_user_config(key: String, value: String) -> Result<(), String> {
    let spec = spec_for(&key)?;
    let stored = validate(spec, &value)?;

    match spec.tier {
        Tier::VoxConfig => {
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
        Tier::Flat => {
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
    let spec = spec_for(&key)?;
    match spec.tier {
        Tier::VoxConfig => {
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
        Tier::Flat => {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn float_spec() -> &'static FieldSpec {
        spec_for("daily_budget_usd").expect("float field")
    }

    fn int_spec() -> &'static FieldSpec {
        spec_for("train_epochs").expect("int field")
    }

    #[test]
    fn float_rejects_nan_inf_and_negative() {
        let spec = float_spec();
        assert!(validate(spec, "nan").is_err());
        assert!(validate(spec, "inf").is_err());
        assert!(validate(spec, "-5").is_err());
        // 1e999 overflows f64 to +inf and must be rejected as non-finite.
        assert!(validate(spec, "1e999").is_err());
    }

    #[test]
    fn float_accepts_valid_nonnegative() {
        let spec = float_spec();
        assert_eq!(validate(spec, " 2.5 ").as_deref(), Ok("2.5"));
        assert_eq!(validate(spec, "0").as_deref(), Ok("0"));
    }

    #[test]
    fn int_rejects_negative() {
        let spec = int_spec();
        assert!(validate(spec, "-3").is_err());
        assert_eq!(validate(spec, "4").as_deref(), Ok("4"));
    }
}
