//! Upload target configuration: the canonical production OTLP endpoint and the
//! write-only ingest token.
//!
//! Pure (no `reqwest`, no network) so it compiles in `--no-default-features`
//! builds and is unit-testable without the `remote` feature. The async uploader
//! (`upload.rs`, feature `remote`) consumes this to know where to POST and which
//! bearer to attach.
//!
//! SECURITY: `ingest_token` is a **write-only anti-abuse key** (Sentry-DSN model),
//! NOT a confidentiality boundary. Privacy is enforced by client-side redaction
//! (project → redact, applied before the spool) and the server-side taxonomy
//! allowlist — never by this token.

/// Default production ingest endpoint — the Vox Foundation Coolify deployment.
pub const DEFAULT_OTLP_ENDPOINT: &str = "https://telemetry.voxlang.org/v1/logs";

/// Env var overriding [`DEFAULT_OTLP_ENDPOINT`] (e.g. the local mirror at
/// `http://localhost:4318/v1/logs`, or a staging host).
pub const OTLP_ENDPOINT_ENV: &str = "VOX_TELEMETRY_OTLP_ENDPOINT";

/// Env var carrying the write-only ingest anti-abuse key.
pub const INGEST_TOKEN_ENV: &str = "VOX_TELEMETRY_INGEST_TOKEN";

/// Where (and with what auth) the uploader POSTs redacted OTLP records.
#[derive(Debug, Clone)]
pub struct TelemetryUploadConfig {
    pub endpoint: String,
    pub ingest_token: Option<String>,
}

impl Default for TelemetryUploadConfig {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_OTLP_ENDPOINT.to_string(),
            ingest_token: None,
        }
    }
}

impl TelemetryUploadConfig {
    /// Resolve from the environment, defaulting to the production endpoint. The
    /// endpoint override and token env vars are both honored; an empty value is
    /// treated as unset.
    pub fn from_env() -> Self {
        let endpoint = std::env::var(OTLP_ENDPOINT_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_OTLP_ENDPOINT.to_string());
        let ingest_token = std::env::var(INGEST_TOKEN_ENV).ok().filter(|s| !s.is_empty());
        Self {
            endpoint,
            ingest_token,
        }
    }

    /// The `Authorization` header value (`Bearer <token>`) when a token is set;
    /// `None` ⇒ send no auth header (local-dev / unauthenticated mirror).
    pub fn authorization_header(&self) -> Option<String> {
        self.ingest_token.as_ref().map(|t| format!("Bearer {t}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_endpoint_is_production_telemetry_host() {
        assert_eq!(
            TelemetryUploadConfig::default().endpoint,
            "https://telemetry.voxlang.org/v1/logs"
        );
    }

    #[test]
    fn ingest_token_threads_into_authorization_header() {
        let cfg = TelemetryUploadConfig {
            endpoint: "x".into(),
            ingest_token: Some("k".into()),
        };
        assert_eq!(cfg.authorization_header().as_deref(), Some("Bearer k"));
    }

    #[test]
    fn no_token_means_no_authorization_header() {
        let cfg = TelemetryUploadConfig {
            endpoint: "x".into(),
            ingest_token: None,
        };
        assert_eq!(cfg.authorization_header(), None);
    }
}
