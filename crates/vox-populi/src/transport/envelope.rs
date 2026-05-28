//! Ed25519-signed A2A envelope. Replaces JWT-HS256 (forgeable by any
//! token-holder) per SSOT Phase 5 P5-T1.
//!
//! Wire shape (JSON):
//! ```json
//! {
//!   "version": 1,
//!   "message_type": "ack",
//!   "sender_pubkey_hex": "<64 hex>",
//!   "payload_b64": "<base64 std>",
//!   "signature_b64": "<base64 std>",
//!   "issued_at_unix_ms": 1234567890123
//! }
//! ```
//!
//! Signature input is the canonical concatenation:
//! `b"voxmesh.envelope.v1\0" || message_type || \0 || payload || \0 || issued_at_unix_ms_be8`.
//! Anti-replay is enforced by `issued_at_unix_ms` clock-skew bound at the
//! verifier (default ±300s) plus a bounded LRU of recent signatures.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use vox_crypto::{SigningKey, VerifyingKey, sign, verify_signature_hex, verifying_key_to_bytes};

/// Stable canonical-input prefix. Bumping invalidates all old signatures.
pub const ENVELOPE_DOMAIN: &[u8] = b"voxmesh.envelope.v1\0";

/// Ed25519-signed wire envelope for A2A control-plane messages (P5-T1a).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedA2AEnvelope {
    /// Envelope schema version; must be 1.
    pub version: u8,
    /// Logical message type (e.g. `"ack"`, `"job_submit"`).
    pub message_type: String,
    /// Hex-encoded Ed25519 public key of the sender (64 hex chars = 32 bytes).
    pub sender_pubkey_hex: String,
    /// Standard base64-encoded payload bytes.
    pub payload_b64: String,
    /// Standard base64-encoded 64-byte Ed25519 signature over the canonical input.
    pub signature_b64: String,
    /// Wall-clock time when the envelope was signed (Unix milliseconds).
    pub issued_at_unix_ms: u64,
}

/// Errors that can occur when verifying a [`SignedA2AEnvelope`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvelopeVerifyError {
    /// Envelope schema version is not supported by this implementation.
    #[error("unsupported envelope version: {0}")]
    UnsupportedVersion(u8),
    /// `sender_pubkey_hex` is not valid hex or not a valid Ed25519 public key.
    #[error("invalid pubkey hex")]
    InvalidPubkey,
    /// `signature_b64` is not valid standard base64 or not 64 bytes when decoded.
    #[error("invalid signature base64")]
    InvalidSignatureB64,
    /// `payload_b64` is not valid standard base64.
    #[error("invalid payload base64")]
    InvalidPayloadB64,
    /// Signature verification failed: payload, pubkey, or signature were tampered.
    #[error("signature does not verify")]
    SignatureMismatch,
    /// `issued_at_unix_ms` is outside the ±`skew_ms` window from now.
    #[error("issued_at out of clock skew window: drift={drift_ms}ms")]
    ClockSkew {
        /// Signed drift in milliseconds (positive = envelope is from the past).
        drift_ms: i64,
    },
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn canonical_input(message_type: &str, payload: &[u8], issued_at_unix_ms: u64) -> Vec<u8> {
    let mut buf =
        Vec::with_capacity(ENVELOPE_DOMAIN.len() + message_type.len() + 1 + payload.len() + 1 + 8);
    buf.extend_from_slice(ENVELOPE_DOMAIN);
    buf.extend_from_slice(message_type.as_bytes());
    buf.push(0u8);
    buf.extend_from_slice(payload);
    buf.push(0u8);
    buf.extend_from_slice(&issued_at_unix_ms.to_be_bytes());
    buf
}

impl SignedA2AEnvelope {
    pub fn sign(message_type: &str, payload: &[u8], sk: &SigningKey, vk: &VerifyingKey) -> Self {
        let issued_at_unix_ms = now_unix_ms();
        let input = canonical_input(message_type, payload, issued_at_unix_ms);
        let sig = sign(sk, &input);
        Self {
            version: 1,
            message_type: message_type.to_string(),
            sender_pubkey_hex: hex::encode(verifying_key_to_bytes(vk)),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
            signature_b64: base64::engine::general_purpose::STANDARD.encode(sig),
            issued_at_unix_ms,
        }
    }

    /// Self-contained verification: parses pubkey from envelope and verifies.
    /// Does **not** consult the trust ledger — use `auth_ed25519::verify_against_trust`
    /// for that.
    pub fn verify_self_signed(&self) -> Result<Vec<u8>, EnvelopeVerifyError> {
        if self.version != 1 {
            return Err(EnvelopeVerifyError::UnsupportedVersion(self.version));
        }
        let payload = base64::engine::general_purpose::STANDARD
            .decode(&self.payload_b64)
            .map_err(|_| EnvelopeVerifyError::InvalidPayloadB64)?;
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| EnvelopeVerifyError::InvalidSignatureB64)?;
        if sig_bytes.len() != 64 {
            return Err(EnvelopeVerifyError::InvalidSignatureB64);
        }
        let input = canonical_input(&self.message_type, &payload, self.issued_at_unix_ms);
        let ok = verify_signature_hex(&self.sender_pubkey_hex, &input, &hex::encode(&sig_bytes))
            .map_err(|_| EnvelopeVerifyError::InvalidPubkey)?;
        if !ok {
            return Err(EnvelopeVerifyError::SignatureMismatch);
        }
        Ok(payload)
    }

    /// Verify the issued-at fits within `±skew_ms` of now.
    pub fn check_clock_skew(&self, skew_ms: u64) -> Result<(), EnvelopeVerifyError> {
        let now = now_unix_ms() as i64;
        let drift = now - self.issued_at_unix_ms as i64;
        if drift.unsigned_abs() > skew_ms {
            return Err(EnvelopeVerifyError::ClockSkew { drift_ms: drift });
        }
        Ok(())
    }
}
