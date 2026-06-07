//! Typed environment readers with defaults.
//!
//! For non-secret configuration: timeouts, operator flags, feature gates, routing preferences,
//! and other numeric or boolean tuning knobs. Values are resolved from environment variables,
//! `~/.vox/config.toml`, and compiled-in defaults (in that order).
//! For API keys and other sensitive values, use `vox_secrets::resolve_secret(...)` — this
//! module deliberately does not handle secrets.

use std::time::Duration;

use crate::toml_config;

#[must_use]
pub fn parse_u64_opt(raw: Option<&str>, default: u64) -> u64 {
    raw.and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}

/// Resolve a non-secret string config value with layered precedence:
/// 1. env var (highest — CI/override)
/// 2. `~/.vox/config.toml`
/// 3. compiled default
///
/// Suitable for operator-tuning strings (base URLs, log levels, feature flags).
/// For secrets such as API keys or tokens, use `vox_secrets::resolve_secret` instead.
#[must_use]
pub fn resolve_config_str(name: &str, default: &str) -> String {
    if let Ok(v) = std::env::var(name)
        && !v.trim().is_empty()
    {
        return v;
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(s) = v.as_str() {
            return s.to_string();
        } else if let Some(i) = v.as_integer() {
            return i.to_string();
        } else if let Some(f) = v.as_float() {
            return f.to_string();
        } else if let Some(b) = v.as_bool() {
            return b.to_string();
        }
    }
    default.to_string()
}

/// Resolve a u64 config value using layered precedence.
#[must_use]
pub fn resolve_config_u64(name: &str, default: u64) -> u64 {
    if let Ok(v) = std::env::var(name)
        && let Ok(parsed) = v.trim().parse::<u64>()
    {
        return parsed;
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(i) = v.as_integer() {
            if i >= 0 {
                return i as u64;
            }
        } else if let Some(s) = v.as_str()
            && let Ok(parsed) = s.trim().parse::<u64>()
        {
            return parsed;
        }
    }
    default
}

/// Resolve a usize config value using layered precedence.
#[must_use]
pub fn resolve_config_usize(name: &str, default: usize) -> usize {
    if let Ok(v) = std::env::var(name)
        && let Ok(parsed) = v.trim().parse::<usize>()
    {
        return parsed;
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(i) = v.as_integer() {
            if i >= 0 {
                return i as usize;
            }
        } else if let Some(s) = v.as_str()
            && let Ok(parsed) = s.trim().parse::<usize>()
        {
            return parsed;
        }
    }
    default
}

/// Resolve a bool config value using layered precedence.
#[must_use]
pub fn resolve_config_bool(name: &str, default: bool) -> bool {
    if let Ok(v) = std::env::var(name) {
        let t = v.trim().to_ascii_lowercase();
        if t == "1" || t == "true" || t == "yes" || t == "on" {
            return true;
        } else if t == "0" || t == "false" || t == "no" || t == "off" {
            return false;
        }
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(b) = v.as_bool() {
            return b;
        } else if let Some(s) = v.as_str() {
            let t = s.trim().to_ascii_lowercase();
            if t == "1" || t == "true" || t == "yes" || t == "on" {
                return true;
            } else if t == "0" || t == "false" || t == "no" || t == "off" {
                return false;
            }
        }
    }
    default
}

/// Resolve an f32 config value using layered precedence (env → config.toml → default).
#[must_use]
pub fn resolve_config_f32(name: &str, default: f32) -> f32 {
    resolve_config_opt_f32(name).unwrap_or(default)
}

/// Resolve an i32 config value using layered precedence (env → config.toml → default).
#[must_use]
pub fn resolve_config_i32(name: &str, default: i32) -> i32 {
    resolve_config_opt_i32(name).unwrap_or(default)
}

/// Resolve an optional f32 config value: env var → `~/.vox/config.toml` → `None`.
///
/// Accepts integer, float, or string-coerced TOML values.
#[must_use]
pub fn resolve_config_opt_f32(name: &str) -> Option<f32> {
    if let Ok(v) = std::env::var(name)
        && let Ok(parsed) = v.trim().parse::<f32>()
    {
        return Some(parsed);
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(f) = v.as_float() {
            return Some(f as f32);
        } else if let Some(i) = v.as_integer() {
            return Some(i as f32);
        } else if let Some(s) = v.as_str()
            && let Ok(parsed) = s.trim().parse::<f32>()
        {
            return Some(parsed);
        }
    }
    None
}

/// Resolve an optional i32 config value: env var → `~/.vox/config.toml` → `None`.
///
/// Accepts integer, float (truncated), or string-coerced TOML values.
#[must_use]
pub fn resolve_config_opt_i32(name: &str) -> Option<i32> {
    if let Ok(v) = std::env::var(name)
        && let Ok(parsed) = v.trim().parse::<i32>()
    {
        return Some(parsed);
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(i) = v.as_integer() {
            return i32::try_from(i).ok();
        } else if let Some(f) = v.as_float() {
            return Some(f as i32);
        } else if let Some(s) = v.as_str()
            && let Ok(parsed) = s.trim().parse::<i32>()
        {
            return Some(parsed);
        }
    }
    None
}

#[must_use]
pub fn env_u64(name: &str, default: u64) -> u64 {
    parse_u64_opt(std::env::var(name).ok().as_deref(), default)
}

#[must_use]
pub fn parse_usize_opt(raw: Option<&str>, default: usize) -> usize {
    raw.and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}

#[must_use]
pub fn env_usize(name: &str, default: usize) -> usize {
    parse_usize_opt(std::env::var(name).ok().as_deref(), default)
}

#[must_use]
pub fn env_duration_from_ms(name: &str, default_ms: u64) -> Duration {
    Duration::from_millis(env_u64(name, default_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_u64_trim_and_default() {
        assert_eq!(parse_u64_opt(None, 7), 7);
        assert_eq!(parse_u64_opt(Some(" 42 "), 7), 42);
        assert_eq!(parse_u64_opt(Some("nope"), 3), 3);
    }

    #[test]
    fn parse_usize_trim_and_default() {
        assert_eq!(parse_usize_opt(Some(" 9 "), 1), 9);
    }

    use crate::toml_config;
    use crate::toml_config::test_support::{CONFIG_TEST_LOCK as ENV_LOCK, HomeGuard};

    #[test]
    #[allow(unsafe_code)] // serialized with ENV_LOCK
    fn resolve_f32_env_then_toml_then_default() {
        let _g = ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        let key = "VOX_TEST_RESOLVE_F32";
        unsafe {
            std::env::remove_var(key);
        }
        let _ = toml_config::unset_user_config_value(key);

        // Neither set → default.
        assert!((resolve_config_f32(key, 0.25) - 0.25).abs() < f32::EPSILON);
        assert_eq!(resolve_config_opt_f32(key), None);

        // config.toml set → used.
        toml_config::set_user_config_value(key, "0.5").expect("set");
        assert!((resolve_config_f32(key, 0.25) - 0.5).abs() < f32::EPSILON);
        assert_eq!(resolve_config_opt_f32(key), Some(0.5));

        // env set → wins over config.toml.
        unsafe {
            std::env::set_var(key, "0.9");
        }
        assert!((resolve_config_f32(key, 0.25) - 0.9).abs() < f32::EPSILON);

        unsafe {
            std::env::remove_var(key);
        }
        let _ = toml_config::unset_user_config_value(key);
    }

    #[test]
    #[allow(unsafe_code)] // serialized with ENV_LOCK
    fn resolve_i32_env_then_toml_then_default() {
        let _g = ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        let key = "VOX_TEST_RESOLVE_I32";
        unsafe {
            std::env::remove_var(key);
        }
        let _ = toml_config::unset_user_config_value(key);

        assert_eq!(resolve_config_i32(key, 7), 7);
        assert_eq!(resolve_config_opt_i32(key), None);

        toml_config::set_user_config_value(key, "4096").expect("set");
        assert_eq!(resolve_config_i32(key, 7), 4096);
        assert_eq!(resolve_config_opt_i32(key), Some(4096));

        unsafe {
            std::env::set_var(key, "8192");
        }
        assert_eq!(resolve_config_i32(key, 7), 8192);

        unsafe {
            std::env::remove_var(key);
        }
        let _ = toml_config::unset_user_config_value(key);
    }
}
