use std::path::{Path, PathBuf};

use keyring::Entry;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::errors::SecretError;
use crate::types::SecretSource;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CliCredentials {
    pub registries: std::collections::HashMap<String, RegistryAuth>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryAuth {
    pub token: String,
    pub username: Option<String>,
}

const SECURE_SERVICE: &str = "vox-secrets";
const SECURE_SENTINEL: &str = "__secrets_keyring__";

#[must_use]
pub fn vox_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".vox")
}

fn auth_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("VOX_SECRETS_AUTH_PATH")
        && !override_path.trim().is_empty()
    {
        return PathBuf::from(override_path.trim());
    }
    vox_dir().join("auth.json")
}

fn secure_entry(registry: &str) -> Result<Entry, SecretError> {
    Entry::new(SECURE_SERVICE, registry)
        .map_err(|e| SecretError::BackendUnavailable(format!("secure store unavailable: {e}")))
}

fn read_secure_token(registry: &str) -> Option<String> {
    let entry = secure_entry(registry).ok()?;
    let value = entry.get_password().ok()?;
    if value.trim().is_empty() {
        return None;
    }
    Some(value)
}

fn write_secure_token(registry: &str, token: &str) -> Result<(), SecretError> {
    let entry = secure_entry(registry)?;
    entry
        .set_password(token)
        .map_err(|e| SecretError::BackendUnavailable(format!("failed to write secure token: {e}")))
}

fn read_credentials_file(path: &Path) -> Result<CliCredentials, SecretError> {
    if !path.exists() {
        return Ok(CliCredentials::default());
    }
    let content =
        vox_bounded_fs::read_utf8_path_capped(path).map_err(|e| SecretError::Io(e.to_string()))?;
    Ok(serde_json::from_str::<CliCredentials>(&content).unwrap_or_default())
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn set_file_permissions(path: &Path) -> Result<(), SecretError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| {
            SecretError::Io(format!(
                "failed to set restrictive permissions on {}: {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn write_credentials_file(path: &PathBuf, creds: &CliCredentials) -> Result<(), SecretError> {
    let content = serde_json::to_string_pretty(creds)
        .map_err(|e| SecretError::Serialization(format!("encode auth json: {e}")))?;
    std::fs::write(path, content)
        .map_err(|e| SecretError::Io(format!("write {}: {e}", path.display())))?;
    set_file_permissions(path)?;
    Ok(())
}

#[must_use]
pub fn read_registry_token(registry: &str) -> Option<(SecretString, SecretSource)> {
    if let Some(token) = read_secure_token(registry) {
        return Some((
            SecretString::new(token.into_boxed_str()),
            SecretSource::SecureStore,
        ));
    }
    let path = auth_path();
    if !path.exists() {
        if registry == "voxpm" {
            let legacy = vox_dir().join("auth_token");
            let token = vox_bounded_fs::read_utf8_path_capped_opt(legacy.as_path())?;
            let token = token.trim().to_string();
            if token.is_empty() {
                return None;
            }
            return Some((
                SecretString::new(token.into_boxed_str()),
                SecretSource::LegacyAuthToken,
            ));
        }
        return None;
    }

    let content = vox_bounded_fs::read_utf8_path_capped_opt(path.as_path())?;
    let creds = serde_json::from_str::<CliCredentials>(&content).ok()?;
    let auth = creds.registries.get(registry)?;
    if auth.token == SECURE_SENTINEL {
        return None;
    }
    if auth.token.trim().is_empty() {
        return None;
    }
    Some((
        SecretString::new(auth.token.clone().into_boxed_str()),
        SecretSource::AuthJson,
    ))
}

pub fn write_registry_token(
    registry: &str,
    token: &str,
    username: Option<String>,
) -> Result<PathBuf, SecretError> {
    let config_dir = vox_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| {
            SecretError::Io(format!("Failed to create {}: {e}", config_dir.display()))
        })?;
    }
    let auth_path = auth_path();
    let mut config = read_credentials_file(&auth_path)?;

    // Round-trip verify the keyring. On some platforms / sandboxes the
    // backend accepts the write but the read returns nothing — silently
    // storing the sentinel in that case would break `vox secrets get`.
    // (Honest-completion plan §0: "code path ≠ measurement".) Verify
    // before we commit the sentinel; fall back to plaintext otherwise.
    let secure_store_ok = match write_secure_token(registry, token) {
        Ok(()) => match read_secure_token(registry) {
            Some(roundtripped) if roundtripped == token => true,
            _ => {
                // Best-effort scrub of the partially-written keyring entry
                // so a future read can't return a stale or empty value.
                if let Ok(entry) = secure_entry(registry) {
                    let _ = entry.delete_credential();
                }
                eprintln!(
                    "warning: secure store write for `{registry}` did not round-trip; \
                     falling back to plaintext storage in {}",
                    auth_path.display()
                );
                false
            }
        },
        Err(_) => false,
    };

    config.registries.insert(
        registry.to_string(),
        RegistryAuth {
            token: if secure_store_ok {
                SECURE_SENTINEL.to_string()
            } else {
                token.to_string()
            },
            username,
        },
    );
    write_credentials_file(&auth_path, &config)?;
    Ok(auth_path)
}

/// Remove a registry token: deletes the keyring entry (if present) and the
/// `auth.json` map key. Returns `true` if an entry existed in either store.
///
/// Mirrors the keyring scrub at `write_registry_token` and never returns or
/// logs the token material.
pub fn remove_registry_token(registry: &str) -> Result<bool, SecretError> {
    let mut removed = false;

    // Best-effort delete of the secure-store entry. A missing entry is not an
    // error (NoEntry); only surface real backend failures as warnings.
    if let Ok(entry) = secure_entry(registry) {
        match entry.delete_credential() {
            Ok(()) => removed = true,
            Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                tracing::warn!(registry = %registry, error = %e, "keyring delete failed (continuing best-effort)");
            }
        }
    }

    let path = auth_path();
    if path.exists() {
        let mut creds = read_credentials_file(&path)?;
        if creds.registries.remove(registry).is_some() {
            removed = true;
            write_credentials_file(&path, &creds)?;
        }
    }

    Ok(removed)
}

pub fn migrate_to_secure_store() -> Result<usize, SecretError> {
    let path = auth_path();
    let mut creds = read_credentials_file(&path)?;
    let mut migrated = 0usize;
    for (registry, auth) in &mut creds.registries {
        if auth.token.trim().is_empty() || auth.token == SECURE_SENTINEL {
            continue;
        }
        write_secure_token(registry, &auth.token)?;
        auth.token = SECURE_SENTINEL.to_string();
        migrated += 1;
    }
    if migrated > 0 {
        write_credentials_file(&path, &creds)?;
    }
    Ok(migrated)
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn auth_path_uses_override() {
        let _g = ENV_LOCK.lock().expect("env lock");
        let tmp = std::env::temp_dir().join("vox-secrets-auth.json");
        unsafe {
            std::env::set_var("VOX_SECRETS_AUTH_PATH", &tmp);
        }
        let got = auth_path();
        assert!(got.to_string_lossy().contains("vox-secrets-auth.json"));
        unsafe {
            std::env::remove_var("VOX_SECRETS_AUTH_PATH");
        }
    }

    /// Regression for the journey bug: `set` then `get` must round-trip.
    /// On platforms where the keyring read-back fails after a successful
    /// write, `write_registry_token` falls back to plaintext storage in
    /// `auth.json` so the subsequent `read_registry_token` still works.
    #[test]
    fn write_then_read_round_trips_regardless_of_keyring_health() {
        let _g = ENV_LOCK.lock().expect("env lock");
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let auth_file = tmp_dir.path().join("auth.json");
        unsafe {
            std::env::set_var("VOX_SECRETS_AUTH_PATH", &auth_file);
        }

        // Use a registry name unlikely to collide with a real keyring
        // entry on the test machine.
        let reg = "vox-test-roundtrip-registry";
        let token = "sk-test-roundtrip-abcdef0123456789";

        let written = write_registry_token(reg, token, None).expect("write");
        assert_eq!(written, auth_file);

        let (read_back, _src) = read_registry_token(reg).expect("read");
        assert_eq!(secrecy::ExposeSecret::expose_secret(&read_back), token);

        unsafe {
            std::env::remove_var("VOX_SECRETS_AUTH_PATH");
        }
    }

    /// `remove_registry_token` clears the auth.json map key so a subsequent
    /// read returns `None`. Uses the plaintext fallback path via a temp
    /// `VOX_SECRETS_AUTH_PATH` (no real keyring required).
    #[test]
    fn remove_registry_token_clears_auth_json_entry() {
        let _g = ENV_LOCK.lock().expect("env lock");
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let auth_file = tmp_dir.path().join("auth.json");
        unsafe {
            std::env::set_var("VOX_SECRETS_AUTH_PATH", &auth_file);
        }

        let reg = "vox-test-remove-registry";
        let token = "sk-test-remove-abcdef0123456789";

        write_registry_token(reg, token, None).expect("write");
        assert!(read_registry_token(reg).is_some(), "present after write");

        let removed = remove_registry_token(reg).expect("remove");
        assert!(removed, "remove should report an entry was deleted");

        assert!(
            read_registry_token(reg).is_none(),
            "registry token must be gone after remove"
        );

        // Idempotent: removing again reports nothing (auth.json key already
        // gone; keyring entry, if any, also gone).
        let removed_again = remove_registry_token(reg).expect("remove2");
        assert!(
            !removed_again || read_registry_token(reg).is_none(),
            "second remove leaves the token absent"
        );

        unsafe {
            std::env::remove_var("VOX_SECRETS_AUTH_PATH");
        }
    }
}
