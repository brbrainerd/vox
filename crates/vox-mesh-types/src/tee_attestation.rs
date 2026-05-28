//! TEE attestation envelope (P6-T5).
//!
//! Defines the wire types for attaching a Trusted Execution Environment
//! quote to a `TaskResult`. The verifier trait is intentionally stubbed —
//! real Firecracker/Kata bindings are deferred to v1.x.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Quote types
// ---------------------------------------------------------------------------

/// Kind of TEE that produced the quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeQuoteKind {
    /// Intel TDX (Trust Domain Extensions) attestation quote.
    IntelTdx,
    /// AMD SEV-SNP (Secure Encrypted Virtualisation - Secure Nested Paging).
    AmdSevSnp,
    /// AWS Nitro Enclaves attestation document.
    AwsNitro,
    /// Firecracker micro-VM measurement (custom, not a hardware TEE).
    FirecrackerMeasurement,
    /// Stub / test quote (never accepted by a real verifier).
    Stub,
}

/// A TEE attestation quote attached to a task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeQuote {
    /// Kind of TEE that produced this quote.
    pub kind: TeeQuoteKind,
    /// Base64-encoded raw quote bytes (vendor-specific format).
    pub quote_b64: String,
    /// BLAKE3 hex digest of the workload measurement (PCR-equivalent).
    pub measurement_blake3_hex: String,
    /// ISO-8601 timestamp from the TEE platform clock (may not match wall clock).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_timestamp: Option<String>,
    /// Optional nonce used to freshness-bind the quote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_hex: Option<String>,
}

// ---------------------------------------------------------------------------
// Verifier trait
// ---------------------------------------------------------------------------

/// Errors returned by a `TeeVerifier`.
#[derive(Debug, thiserror::Error)]
pub enum TeeVerifyError {
    /// The verifier does not support this TEE kind.
    #[error("TEE kind {0:?} is not supported by this verifier")]
    Unsupported(TeeQuoteKind),
    /// The quote bytes could not be decoded or parsed.
    #[error("quote decode error: {0}")]
    DecodeFailed(String),
    /// The measurement does not match the expected reference value.
    #[error("measurement mismatch: expected {expected}, got {actual}")]
    MeasurementMismatch { expected: String, actual: String },
    /// Cryptographic signature verification failed.
    #[error("quote signature verification failed")]
    SignatureInvalid,
    /// The quote is stale (too far outside the expected time window).
    #[error("quote is stale")]
    Stale,
    /// Catch-all for platform-specific errors.
    #[error("platform error: {0}")]
    Platform(String),
}

/// Trait for TEE quote verifiers.
///
/// Implementations are platform-specific and loaded as plugins at runtime.
/// The only implementation in this phase is `StubTeeVerifier`, which always
/// returns `Err(TeeVerifyError::Unsupported)`.
pub trait TeeVerifier: Send + Sync {
    /// Verify a TEE quote.
    ///
    /// Returns `Ok(())` on success. Returns `Err` for any verification failure
    /// (signature, measurement, staleness, or unsupported kind).
    fn verify(&self, quote: &TeeQuote) -> Result<(), TeeVerifyError>;
}

// ---------------------------------------------------------------------------
// Stub verifier (shipped; real verifiers are in v1.x plugins)
// ---------------------------------------------------------------------------

/// Stub `TeeVerifier` that always returns `Err(Unsupported)`.
///
/// This ships as the default verifier for Phase 6. Real TEE-backed verifiers
/// (Intel TDX, AMD SEV-SNP, AWS Nitro) are deferred to v1.x plugin crates.
#[derive(Debug, Default)]
pub struct StubTeeVerifier;

impl TeeVerifier for StubTeeVerifier {
    fn verify(&self, quote: &TeeQuote) -> Result<(), TeeVerifyError> {
        Err(TeeVerifyError::Unsupported(quote.kind.clone()))
    }
}
