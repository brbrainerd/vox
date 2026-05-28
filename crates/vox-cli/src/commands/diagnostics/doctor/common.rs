//! Shared helpers and [`Check`] for `vox doctor`.

use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

use vox_bounded_fs::read_utf8_path_capped_async;

/// One environment check row for human or JSON output.
#[derive(Debug, Serialize)]
pub(crate) struct Check {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

#[cfg_attr(not(feature = "codex"), allow(dead_code))]
impl Check {
    /// Create a passing check.
    pub(crate) fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pass: true,
            detail: detail.into(),
        }
    }

    /// Create a failing check.
    pub(crate) fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pass: false,
            detail: detail.into(),
        }
    }

    /// Create a check with an explicit pass/fail predicate.
    pub(crate) fn new(name: impl Into<String>, pass: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pass,
            detail: detail.into(),
        }
    }
}

/// Delegates to [`crate::fs_utils::user_home_dir`] (`vox_config::paths::user_home_dir`).
pub(crate) fn user_home_dir() -> Option<PathBuf> {
    Some(crate::fs_utils::user_home_dir())
}

pub(crate) fn vox_dot_dir() -> PathBuf {
    user_home_dir()
        .map(|h| h.join(".vox"))
        .unwrap_or_else(|| PathBuf::from(".vox"))
}

pub(crate) fn redact_key(k: &str) -> String {
    let t = k.trim();
    if t.is_empty() {
        return "(empty)".to_string();
    }
    if t.len() <= 6 {
        "***".to_string()
    } else {
        format!("{}…{} (redacted)", &t[..4], &t[t.len() - 2..])
    }
}

pub(crate) async fn auth_registry_token(registry: &str) -> Option<String> {
    let auth_path = vox_dot_dir().join("auth.json");
    let content = read_utf8_path_capped_async(&auth_path).await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("registries")?
        .get(registry)?
        .get("token")?
        .as_str()
        .map(std::string::ToString::to_string)
}

fn resolved_google_key_sync() -> Option<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiApiKey)
        .expose()
        .map(std::string::ToString::to_string)
}

pub(crate) async fn resolved_google_key() -> Option<String> {
    if let Some(k) = resolved_google_key_sync() {
        return Some(k);
    }
    auth_registry_token("google").await
}

fn resolved_openrouter_key_sync() -> Option<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
        .expose()
        .map(std::string::ToString::to_string)
}

pub(crate) async fn resolved_openrouter_key() -> Option<String> {
    if let Some(k) = resolved_openrouter_key_sync() {
        return Some(k);
    }
    auth_registry_token("openrouter").await
}

#[derive(Debug, Deserialize)]
pub(crate) struct AuthRegistriesOnly {
    #[serde(default)]
    pub registries: std::collections::HashMap<String, serde_json::Value>,
}
