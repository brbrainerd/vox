//! Tauri commands for the two highest-value STT/voice knobs in Settings:
//! ASR backend selection and dictation domain mode (general vs. code).
//!
//! `VOX_ORATIO_BACKEND` is a registered secret (`vox_secrets::SecretId::VoxOratioBackend`)
//! that `backend_dispatch::create_backend()` resolves via
//! `vox_secrets::resolve_secret(...).expose()` — reads here go through the
//! same resolver so the Settings UI shows the value actually in effect, not
//! a value from a different source. `VOX_ORATIO_DOMAIN_MODE` is a plain env
//! var read directly by `runtime_config.rs` (not a registered secret), so it
//! keeps the simpler env/flat-config lookup. Both writes persist to
//! `vox_config::toml_config` (so the choice survives restart, the same
//! mechanism `user_config.rs`'s `FlatToml`-tier keys use) *and* call
//! `std::env::set_var` so the change takes effect in the running process —
//! mirroring `commands/models.rs`'s established live-effect pattern.
//! These are not added to the LLM/AI-scoped `vox-llm-config` registry: that
//! registry's own header comment scopes it to "Band A" provider/model/tuning/
//! budget keys and explicitly excludes other config domains — STT is not
//! Band A.

use serde::Serialize;
use tauri::command;

/// One STT setting field, mirroring the shape of `UserConfigFieldDto` in
/// `user_config.rs` closely enough that `SettingsView.tsx` can reuse its
/// existing `Row` + enum-button rendering pattern.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttConfigFieldDto {
    pub key: String,
    pub label: String,
    pub hint: String,
    pub options: Vec<String>,
    pub current_value: String,
}

const BACKEND_KEY: &str = "VOX_ORATIO_BACKEND";
const BACKEND_OPTIONS: [&str; 3] = ["auto", "whisper", "sherpa"];
const DOMAIN_KEY: &str = "VOX_ORATIO_DOMAIN_MODE";
const DOMAIN_OPTIONS: [&str; 2] = ["general", "code"];

fn flat_config_fallback(key: &str, default: &str) -> String {
    let cfg = vox_config::toml_config::load_user_config();
    cfg.values
        .get(key)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| default.to_string())
}

/// Current effective backend value — resolved the same way
/// `backend_dispatch::create_backend()` resolves it, not from a separate
/// source the Settings UI would drift from.
fn current_backend_value() -> String {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioBackend)
        .expose()
        .map(str::to_string)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| flat_config_fallback(BACKEND_KEY, "auto"))
}

/// Current effective domain-mode value. `VOX_ORATIO_DOMAIN_MODE` is not a
/// registered secret — mirror `runtime_config.rs`'s own direct-env-var read
/// rather than routing through `vox_secrets`.
fn current_domain_value() -> String {
    if let Ok(v) = std::env::var(DOMAIN_KEY)
        && !v.is_empty()
    {
        return v;
    }
    flat_config_fallback(DOMAIN_KEY, "general")
}

/// Read the two STT settings for the Settings UI.
#[command]
pub fn get_stt_config() -> Vec<SttConfigFieldDto> {
    vec![
        SttConfigFieldDto {
            key: BACKEND_KEY.to_string(),
            label: "Voice dictation engine".to_string(),
            hint: "auto picks Parakeet (sherpa-onnx) when available, falling back to Whisper"
                .to_string(),
            options: BACKEND_OPTIONS.iter().map(|s| s.to_string()).collect(),
            current_value: current_backend_value(),
        },
        SttConfigFieldDto {
            key: DOMAIN_KEY.to_string(),
            label: "Dictation domain".to_string(),
            hint: "code enables symbol/casing expansion (\"open paren\" -> \"(\")".to_string(),
            options: DOMAIN_OPTIONS.iter().map(|s| s.to_string()).collect(),
            current_value: current_domain_value(),
        },
    ]
}

/// Persist one STT setting AND make it take effect immediately in the
/// running process. Both keys are plain enums validated against a fixed
/// option list.
#[command]
pub fn set_stt_config(key: String, value: String) -> Result<(), String> {
    let valid = match key.as_str() {
        BACKEND_KEY => BACKEND_OPTIONS.contains(&value.as_str()),
        DOMAIN_KEY => DOMAIN_OPTIONS.contains(&value.as_str()),
        _ => return Err(format!("unknown STT config key: {key}")),
    };
    if !valid {
        return Err(format!("{value} is not a valid value for {key}"));
    }
    // Persist for the next launch...
    vox_config::toml_config::set_user_config_value(&key, &value)?;
    // ...and take effect immediately: config persistence alone does nothing
    // until something re-reads it into env, and nothing re-hydrates the flat
    // file into process env at startup. Mirrors commands/models.rs's
    // established live-effect pattern.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(&key, &value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_stt_config_returns_both_keys_with_defaults() {
        let fields = get_stt_config();
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.key == BACKEND_KEY));
        assert!(fields.iter().any(|f| f.key == DOMAIN_KEY));
    }

    #[test]
    fn set_stt_config_rejects_unknown_key() {
        assert!(set_stt_config("NOT_A_KEY".to_string(), "x".to_string()).is_err());
    }

    #[test]
    fn set_stt_config_rejects_invalid_value() {
        assert!(set_stt_config(BACKEND_KEY.to_string(), "not_a_backend".to_string()).is_err());
    }

    #[test]
    fn set_stt_config_takes_effect_immediately_in_process_env() {
        // Regression test for the audit finding above this task: a Settings
        // write must be visible to the same process's runtime resolvers
        // without a restart, not just persisted to the flat config file.
        let original = std::env::var(DOMAIN_KEY).ok();
        set_stt_config(DOMAIN_KEY.to_string(), "code".to_string()).expect("valid set");
        assert_eq!(std::env::var(DOMAIN_KEY).as_deref(), Ok("code"));
        #[allow(unsafe_code)]
        unsafe {
            match &original {
                Some(v) => std::env::set_var(DOMAIN_KEY, v),
                None => std::env::remove_var(DOMAIN_KEY),
            }
        }
    }
}
