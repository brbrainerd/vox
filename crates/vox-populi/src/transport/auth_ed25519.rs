//! Trust-ledger-backed verification for Ed25519-signed A2A envelopes (P5-T1b).
//!
//! Layered on top of `envelope.rs`:
//!
//! 1. Parse and self-verify (signature math).
//! 2. Resolve `sender_pubkey_hex` against the [`TrustedNodeRegistry`].
//! 3. Enforce clock-skew window.
//! 4. Return a [`NodeAuthContext`] that downstream policy code consults.
//!
//! Anti-goal: this module never accepts an unknown pubkey for any reason —
//! "reputation" cannot bypass the binary trust gate (SSOT §0).

use vox_identity::TrustedNodeRegistry;

use super::envelope::{EnvelopeVerifyError, SignedA2AEnvelope};

/// Auth context produced after a successful trust-ledger verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAuthContext {
    /// Canonical node id resolved from the trust ledger.
    pub node_id: String,
    /// Ed25519 pubkey hex that was verified.
    pub pubkey_hex: String,
}

/// Errors from [`verify_against_trust`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VerifyTrustError {
    /// The envelope failed signature or clock-skew verification.
    #[error("envelope cryptographic verification failed: {0}")]
    Envelope(#[from] EnvelopeVerifyError),
    /// The sender pubkey is not registered in the trust ledger.
    #[error("pubkey is not in the trust ledger")]
    UnknownPubkey,
}

/// Verify an envelope and admit only if the pubkey is in the trust ledger.
pub fn verify_against_trust(
    env: &SignedA2AEnvelope,
    registry: &TrustedNodeRegistry,
    clock_skew_ms: u64,
) -> Result<NodeAuthContext, VerifyTrustError> {
    let _payload = env.verify_self_signed()?;
    env.check_clock_skew(clock_skew_ms)?;
    let known = registry
        .lookup_by_pubkey_hex(&env.sender_pubkey_hex)
        .ok_or(VerifyTrustError::UnknownPubkey)?;
    Ok(NodeAuthContext {
        node_id: known.node_id().to_string(),
        pubkey_hex: env.sender_pubkey_hex.clone(),
    })
}
