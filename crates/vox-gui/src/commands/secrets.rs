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
