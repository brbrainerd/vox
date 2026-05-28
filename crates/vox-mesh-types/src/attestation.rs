use serde::{Deserialize, Serialize};

/// Signed result attestation produced by a worker for each completed task.
///
/// The worker signs a canonical JSON payload containing all fields except
/// `signature_b64` itself, using a per-job ephemeral Ed25519 key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Task identifier this attestation covers.
    pub task_id: String,
    /// BLAKE3 hex digest of the raw task input payload.
    pub input_hash_blake3_hex: String,
    /// BLAKE3 hex digest of the raw output payload.
    pub output_hash_blake3_hex: String,
    /// Measured GPU wall-clock time in fractional seconds.
    pub gpu_seconds: f64,
    /// BLAKE3 hex digest of the full execution trace, if available.
    pub trace_blake3_hex: Option<String>,
    /// Hex-encoded compressed Ed25519 public key of the ephemeral signing key.
    pub ephemeral_pubkey_hex: String,
    /// Base64-encoded Ed25519 signature over the canonical signed payload.
    pub signature_b64: String,
    /// Unix epoch milliseconds when this attestation was produced.
    pub signed_at_unix_ms: u64,
    /// Optional TEE attestation quote (P6-T5). Present when the task ran inside a
    /// TEE-capable sandbox (Intel TDX, AMD SEV-SNP, AWS Nitro, Firecracker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tee_quote: Option<crate::tee_attestation::TeeQuote>,
    /// Optional BLAKE3 hex digest of the replay proof (P6-T5).
    /// Allows a verifier to replay the computation and confirm the output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_proof_blake3_hex: Option<String>,
    /// Optional base64-encoded kudos signature over the attestation (P6-T5).
    /// Signed by the node's long-lived identity key, not the ephemeral key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kudos_signature_b64: Option<String>,
}
