use secrecy::SecretString;

use crate::errors::SecretError;
use crate::spec::SecretId;
use crate::spec::SecretSpec;

pub trait SecretBackend: Send + Sync {
    fn resolve(
        &self,
        id: SecretId,
        spec: SecretSpec,
        profile: Option<&str>,
        caller_context: &str,
    ) -> Result<Option<SecretString>, SecretError>;
    fn write_audit_log(
        &self,
        secret_id: &str,
        status: &str,
        source: Option<&str>,
        profile: &str,
        caller_context: &str,
        detail: Option<&str>,
    ) -> Result<(), SecretError>;
}

pub struct NoopBackend;

impl SecretBackend for NoopBackend {
    fn resolve(
        &self,
        _id: SecretId,
        _spec: SecretSpec,
        _profile: Option<&str>,
        _caller_context: &str,
    ) -> Result<Option<SecretString>, SecretError> {
        Ok(None)
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

pub struct UnavailableBackend {
    pub reason: String,
}

impl SecretBackend for UnavailableBackend {
    fn resolve(
        &self,
        _id: SecretId,
        _spec: SecretSpec,
        _profile: Option<&str>,
        _caller_context: &str,
    ) -> Result<Option<SecretString>, SecretError> {
        Err(SecretError::BackendUnavailable(self.reason.clone()))
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

#[cfg(feature = "secrets-infisical")]
pub mod infisical;

#[cfg(feature = "secrets-vault")]
pub mod vault;

pub mod vox_vault;

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::errors::SecretError;
    use crate::spec::{SecretId, SecretSpec};

    fn dummy_spec() -> SecretSpec {
        *SecretId::OpenRouterApiKey.spec()
    }

    #[test]
    fn unavailable_backend_resolve_returns_err_with_reason() {
        let backend = UnavailableBackend {
            reason: "integration-test-reason".to_string(),
        };
        let result = SecretBackend::resolve(
            &backend,
            SecretId::OpenRouterApiKey,
            dummy_spec(),
            None,
            "test-context",
        );
        let err = result.expect_err("UnavailableBackend::resolve must return Err");
        match err {
            SecretError::BackendUnavailable(msg) => {
                assert!(
                    msg.contains("integration-test-reason"),
                    "reason must be propagated into error; got: {msg}"
                );
            }
            other => panic!("expected BackendUnavailable, got {:?}", other),
        }
    }

    #[test]
    fn unavailable_backend_resolve_propagates_different_reasons() {
        for reason in &["service down", "feature disabled", ""] {
            let backend = UnavailableBackend {
                reason: reason.to_string(),
            };
            let result = SecretBackend::resolve(
                &backend,
                SecretId::GeminiApiKey,
                *SecretId::GeminiApiKey.spec(),
                Some("prod"),
                "caller",
            );
            assert!(result.is_err(), "must always error for reason={:?}", reason);
            if let Err(SecretError::BackendUnavailable(msg)) = result {
                assert_eq!(&msg, reason);
            }
        }
    }

    #[test]
    fn unavailable_backend_write_audit_log_always_succeeds() {
        let backend = UnavailableBackend {
            reason: "whatever".to_string(),
        };
        let result = SecretBackend::write_audit_log(
            &backend,
            "OPENROUTER_API_KEY",
            "resolved",
            Some("env"),
            "dev",
            "caller",
            None,
        );
        assert!(
            result.is_ok(),
            "write_audit_log on UnavailableBackend must be Ok"
        );
    }
}
