//! Ed25519 signing and verification for [`PreregistrationV1`].
//!
//! # Signing flow
//! 1. Set `prereg.signed_at` to current Unix timestamp.
//! 2. Set `prereg.signing_key` to the hex-encoded Ed25519 verifying key.
//! 3. Set `prereg.id` to the Trusty URI of the (now fully-populated) canonical JSON.
//! 4. Sign the canonical JSON bytes with [`vox_crypto::facades::sign`].
//! 5. Return the 64-byte signature as hex.
//!
//! # Verification flow
//! 1. Decode `signature_hex` → 64-byte array.
//! 2. Decode `prereg.signing_key` → [`vox_crypto::facades::VerifyingKey`].
//! 3. Compute canonical JSON of the prereg (same as during signing).
//! 4. Call [`vox_crypto::facades::verify`]; return error if it returns false.

use crate::preregistration::trusty_uri::{canonical_json, compute_trusty_uri};
use thiserror::Error;
use vox_crypto::facades::{
    SigningKey, sign, to_verifying_key, verify, verifying_key_from_bytes, verifying_key_to_bytes,
};
use vox_research_events::preregistration::PreregistrationV1;

/// A hex-encoded Ed25519 signature (128 hex characters = 64 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub String);

/// Errors returned by [`sign_prereg`].
#[derive(Debug, Error)]
pub enum SignError {
    #[error("serialization failed: {0}")]
    Serialization(String),
}

/// Errors returned by [`verify_prereg`].
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid signature hex: {0}")]
    BadSignatureHex(String),
    #[error("signature has wrong length: expected 64 bytes, got {0}")]
    BadSignatureLength(usize),
    #[error("invalid signing key hex in prereg: {0}")]
    BadKeyHex(String),
    #[error("invalid signing key bytes: {0}")]
    BadKeyBytes(String),
    #[error("signature verification failed")]
    InvalidSignature,
}

/// Sign `prereg` in place, returning the hex-encoded signature.
///
/// Sets `prereg.signed_at`, `prereg.signing_key`, and `prereg.id` as side-effects.
pub fn sign_prereg(
    prereg: &mut PreregistrationV1,
    signing_key: &SigningKey,
) -> Result<Signature, SignError> {
    // Capture current Unix timestamp
    prereg.signed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Embed the verifying key (hex-encoded) so verifiers don't need a separate key store
    let vk = to_verifying_key(signing_key);
    prereg.signing_key = hex::encode(verifying_key_to_bytes(&vk));

    // Compute Trusty URI now that all fields are set
    prereg.id = compute_trusty_uri(prereg);

    // Sign the canonical JSON bytes
    let canonical = canonical_json(prereg);
    let sig_bytes = sign(signing_key, canonical.as_bytes());
    Ok(Signature(hex::encode(sig_bytes)))
}

/// Verify that `signature_hex` is a valid Ed25519 signature over the canonical JSON of `prereg`.
pub fn verify_prereg(prereg: &PreregistrationV1, signature_hex: &str) -> Result<(), VerifyError> {
    // Decode signature
    let sig_bytes =
        hex::decode(signature_hex).map_err(|e| VerifyError::BadSignatureHex(e.to_string()))?;
    if sig_bytes.len() != 64 {
        return Err(VerifyError::BadSignatureLength(sig_bytes.len()));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    // Decode verifying key from prereg.signing_key
    let pk_bytes =
        hex::decode(&prereg.signing_key).map_err(|e| VerifyError::BadKeyHex(e.to_string()))?;
    if pk_bytes.len() != 32 {
        return Err(VerifyError::BadKeyHex(format!(
            "expected 32 bytes, got {}",
            pk_bytes.len()
        )));
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk_bytes);
    let vk = verifying_key_from_bytes(&pk_arr).map_err(VerifyError::BadKeyBytes)?;

    // Verify over canonical JSON
    let canonical = canonical_json(prereg);
    if verify(&vk, canonical.as_bytes(), &sig_arr) {
        Ok(())
    } else {
        Err(VerifyError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "tool-call malformation rate increased after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:deadbeef".to_string(),
                eval_set_swhid: "swh:1:dir:cafebabe".to_string(),
                inspect_task_id: Some("task-malform-01".to_string()),
            },
            metric: MetricSpec {
                name: "malformation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule {
                max_n: 500,
                alpha: None,
                threshold: Some(0.95),
            },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 20.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: Some("abc1234".to_string()),
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        assert!(!prereg.id.is_empty(), "id must be set after signing");
        assert!(
            !prereg.signing_key.is_empty(),
            "signing_key must be set after signing"
        );
        assert!(prereg.signed_at > 0, "signed_at must be set after signing");
        verify_prereg(&prereg, &sig.0).expect("verification must succeed");
    }

    #[test]
    fn tamper_detection_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        prereg.hypothesis = "TAMPERED hypothesis".to_string();
        let result = verify_prereg(&prereg, &sig.0);
        assert!(result.is_err(), "verification must fail after tampering");
    }

    #[test]
    fn wrong_signature_hex_fails_verify() {
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "ff".repeat(64);
        let result = verify_prereg(&prereg, &bad_sig);
        assert!(
            result.is_err(),
            "verification must fail with wrong signature"
        );
    }
}
