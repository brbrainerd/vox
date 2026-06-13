//! Tauri commands for the "Keys & Secrets" GUI surface.
//!
//! SECURITY INVARIANT (non-negotiable): a secret value, once submitted, is
//! NEVER returned to the UI. The DTO carries only presence + a redacted
//! `head4…tail2` preview. Neither [`vox_secrets::ResolvedSecret::expose`] nor
//! `vox_secrets::get_registry_token` is reachable from any command here —
//! `set_secret` / `remove_secret` return a bare `is_present` boolean only.

use serde::Serialize;
use tauri::command;
use vox_secrets::SecretSpec;

/// Redaction-safe status row for one managed secret. Mirrors
/// [`vox_secrets::SecretStatusRow`]; every field is non-sensitive.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatusDto {
    pub id: String,
    pub canonical_env: String,
    pub scope_description: String,
    pub taxonomy_slug: String,
    pub auth_registry: Option<String>,
    pub required: bool,
    pub is_present: bool,
    pub status: String,
    /// `head4…tail2 (redacted)` preview or `(missing)` — never the raw value.
    pub redacted: String,
    pub source: Option<String>,
    pub remediation: String,
}

impl From<vox_secrets::SecretStatusRow> for SecretStatusDto {
    fn from(row: vox_secrets::SecretStatusRow) -> Self {
        Self {
            id: row.id,
            canonical_env: row.canonical_env.to_string(),
            scope_description: row.scope_description.to_string(),
            taxonomy_slug: row.taxonomy_slug.to_string(),
            auth_registry: row.auth_registry.map(str::to_string),
            required: row.required,
            is_present: row.is_present,
            status: row.status,
            redacted: row.redacted,
            source: row.source,
            remediation: row.remediation.to_string(),
        }
    }
}

/// List every real (non config-only) managed secret with presence + redacted
/// preview. No raw values are ever included.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets/vox_vault backends; redaction + routing covered by vox-secrets tests
pub fn list_secret_status() -> Vec<SecretStatusDto> {
    vox_secrets::list_secret_status()
        .into_iter()
        .map(SecretStatusDto::from)
        .collect()
}

/// Resolve the spec whose canonical env name matches `key`. The GUI sends the
/// `canonical_env` shown in the table.
fn spec_for_key(key: &str) -> Result<&'static SecretSpec, String> {
    vox_secrets::all_specs()
        .into_iter()
        .find(|s| s.canonical_env == key)
        .ok_or_else(|| format!("unknown secret key: {key}"))
}

/// Write a secret value. Routes auth.json registry-token keys to the auth
/// store, everything else to the Clavis vault. Returns the resulting
/// `is_present` (always `true` on success) — NEVER the value, which is
/// dropped as soon as it is persisted.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets/vox_vault backends; redaction + routing covered by vox-secrets tests
pub fn set_secret(key: String, value: String) -> Result<bool, String> {
    if value.is_empty() {
        return Err("secret value must not be empty".to_string());
    }
    let spec = spec_for_key(&key)?;

    if let Some(registry) = spec.auth_registry {
        vox_secrets::set_registry_token(registry, &value, None).map_err(|e| e.to_string())?;
    } else {
        let backend_key = spec.backend_key.unwrap_or(spec.canonical_env);
        let backend =
            vox_secrets::backend::vox_vault::VoxCloudBackend::new().map_err(|e| e.to_string())?;
        backend
            .write_secret(backend_key, &value)
            .map_err(|e| e.to_string())?;
    }

    // Re-resolve presence only; the value is never read back to the UI.
    Ok(vox_secrets::resolve_secret(spec.id).is_present())
}

/// Non-sensitive backend/profile status header for the Keys & Secrets surface.
/// Mirrors the data behind the CLI `vox secrets backend-status` command. Carries
/// only the backend mode selector, active resolution profile, and (if the backend
/// is unreachable) a non-sensitive availability detail — NEVER any secret value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsBackendStatusDto {
    /// `Debug` form of [`vox_secrets::BackendMode`] (e.g. `Auto`, `VoxCloud`).
    pub backend_mode: String,
    /// Active resolution profile selector (`dev` / `ci` / `prod` / `hardcut`).
    pub profile: String,
    /// `true` when the active profile is strict (CI/prod/hardcut).
    pub strict: bool,
    /// `true` when the backend resolves; `false` if a spec reported unavailable.
    pub available: bool,
    /// Non-sensitive availability detail when `available == false`.
    pub detail: Option<String>,
}

/// Report the active secrets backend mode + resolution profile (non-sensitive).
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets backend-status; non-sensitive fields only
pub fn secrets_backend_status() -> SecretsBackendStatusDto {
    let mode = vox_secrets::BackendMode::from_env();
    let profile = vox_secrets::active_resolve_profile();
    let detail = vox_secrets::backend_unavailable_detail();
    SecretsBackendStatusDto {
        backend_mode: format!("{mode:?}"),
        profile: profile.as_str().to_string(),
        strict: profile.is_strict(),
        available: detail.is_none(),
        detail,
    }
}

/// One managed secret recognised during an `.env` import. SECURITY: name +
/// redacted preview only — never the raw value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEnvEntryDto {
    pub source_key: String,
    pub canonical_env: String,
    pub redacted: String,
}

/// Result of an `.env` import (dry-run preview or applied write).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEnvResultDto {
    /// `true` if values were written to the vault; `false` for a dry-run preview.
    pub applied: bool,
    /// Number of managed secrets recognised / imported.
    pub count: usize,
    /// Recognised managed secrets (names + redacted preview only).
    pub entries: Vec<ImportEnvEntryDto>,
}

/// Import managed secrets from a `.env` file. When `apply` is `false` (dry-run)
/// the result lists only the KEY NAMES (+ redacted preview) that WOULD import —
/// no values are stored or returned. When `apply` is `true` the values are
/// written to the vault and only the redaction-safe entries + count come back.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets::import_env_from_path; redaction covered by vox-secrets tests
pub fn import_env(path: Option<String>, apply: bool) -> Result<ImportEnvResultDto, String> {
    let p = path
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(".env"));
    let result = vox_secrets::import_env_from_path(&p, apply).map_err(|e| e.to_string())?;
    Ok(ImportEnvResultDto {
        applied: result.applied,
        count: result.count(),
        entries: result
            .entries
            .into_iter()
            .map(|e| ImportEnvEntryDto {
                source_key: e.source_key,
                canonical_env: e.canonical_env.to_string(),
                redacted: e.redacted,
            })
            .collect(),
    })
}

/// Migrate plaintext `auth.json` registry tokens into the secure store. Returns
/// the number of entries moved. No token material is returned.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets::migrate_auth_store_to_secure_store
pub fn migrate_auth_store() -> Result<usize, String> {
    vox_secrets::migrate_auth_store_to_secure_store().map_err(|e| e.to_string())
}

/// Remove a secret. Routes auth.json registry-token keys to
/// `remove_registry_token`, everything else to the vault `delete_secret`.
/// Returns `is_present` (`false` on success) — never any value.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_secrets/vox_vault backends; redaction + routing covered by vox-secrets tests
pub fn remove_secret(key: String) -> Result<bool, String> {
    let spec = spec_for_key(&key)?;

    if let Some(registry) = spec.auth_registry {
        vox_secrets::remove_registry_token(registry).map_err(|e| e.to_string())?;
    } else {
        let backend_key = spec.backend_key.unwrap_or(spec.canonical_env);
        let backend =
            vox_secrets::backend::vox_vault::VoxCloudBackend::new().map_err(|e| e.to_string())?;
        backend
            .delete_secret(backend_key)
            .map_err(|e| e.to_string())?;
    }

    Ok(vox_secrets::resolve_secret(spec.id).is_present())
}
