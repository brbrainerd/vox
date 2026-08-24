//! Stand-in for [`super::vox_vault`] when the `secrets-vox-vault` feature is OFF.
//!
//! SECURITY: this is deliberately NOT a silent no-op. Every entry point returns
//! [`SecretError::BackendUnavailable`], so `resolve_secret` reports
//! [`crate::ResolutionStatus::BackendUnavailable`] (with detail) exactly as it
//! does when a real vault fails to initialize — it never degrades to a quiet
//! "secret not found". Writes fail loudly instead of appearing to succeed.

use secrecy::SecretString;

use super::SecretBackend;
use crate::errors::SecretError;
use crate::spec::{SecretId, SecretSpec};

const DISABLED: &str = "vox-secrets was built without the `secrets-vox-vault` feature; \
     the Clavis vault backend is not linked into this build";

fn disabled<T>() -> Result<T, SecretError> {
    tracing::error!(target: "vox::secrets", "{DISABLED}");
    Err(SecretError::BackendUnavailable(DISABLED.to_string()))
}

pub const DEFAULT_HISTORY_DEPTH: u32 = 10;

/// Never constructible: [`VoxCloudBackend::new`] always fails in this build.
#[derive(Debug)]
pub struct VoxCloudBackend {
    _never: std::convert::Infallible,
}

impl VoxCloudBackend {
    /// Always fails — the vault backend is not compiled into this build.
    ///
    /// # Errors
    /// Always returns [`SecretError::BackendUnavailable`].
    pub fn new() -> Result<Self, SecretError> {
        disabled()
    }

    /// Always fails — see [`VoxCloudBackend::new`].
    ///
    /// # Errors
    /// Always returns [`SecretError::BackendUnavailable`].
    pub fn write_secret(&self, _key: &str, _plaintext: &str) -> Result<(), SecretError> {
        disabled()
    }

    /// Always fails — see [`VoxCloudBackend::new`].
    ///
    /// # Errors
    /// Always returns [`SecretError::BackendUnavailable`].
    #[allow(clippy::too_many_arguments)]
    pub fn write_secret_v2(
        &self,
        _secret_id: &str,
        _plaintext: &str,
        _profile: Option<&str>,
        _change_kind: &str,
        _detail: Option<&str>,
        _caller_context: &str,
        _history_depth: u32,
    ) -> Result<(), SecretError> {
        disabled()
    }
}

impl SecretBackend for VoxCloudBackend {
    fn resolve(
        &self,
        _id: SecretId,
        _spec: SecretSpec,
        _profile: Option<&str>,
        _caller_context: &str,
    ) -> Result<Option<SecretString>, SecretError> {
        disabled()
    }

    fn write_audit_log(
        &self,
        _secret_id: &str,
        _status: &str,
        _source: Option<&str>,
        _profile: &str,
        _caller_context: &str,
        _detail: Option<&str>,
    ) -> Result<(), SecretError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultHealth {
    pub vault_path_display: String,
    pub keyring_entry_present: bool,
    pub master_fingerprint: String,
    pub account_id: String,
    pub kek_ref: String,
    pub kek_version: i64,
    pub row_count: u64,
    pub can_decrypt: bool,
    pub decrypt_error: Option<String>,
}

/// Always fails — the vault backend is not compiled into this build.
///
/// # Errors
/// Always returns [`SecretError::BackendUnavailable`].
pub fn probe_vault_health(_backend: &VoxCloudBackend) -> Result<VaultHealth, SecretError> {
    disabled()
}

/// One-line summary for `vox secrets doctor` (no secret material).
#[must_use]
pub fn cloudless_vault_env_diagnostic() -> String {
    "mode=disabled; reason=built without the `secrets-vox-vault` cargo feature".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_fails_loudly_rather_than_returning_a_noop_backend() {
        let err = VoxCloudBackend::new().expect_err("vault must be unavailable, not a no-op");
        assert!(
            matches!(err, SecretError::BackendUnavailable(ref r) if r.contains("secrets-vox-vault")),
            "expected a named BackendUnavailable, got {err:?}"
        );
    }

    #[test]
    fn diagnostic_reports_disabled() {
        assert!(cloudless_vault_env_diagnostic().contains("disabled"));
    }
}
