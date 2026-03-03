//! Webhook signature generation and verification using HMAC-SHA3-256.

use data_encoding::HEXLOWER;
use sha3::{Digest, Sha3_256};

use crate::WebhookError;

/// A webhook signature — an HMAC-SHA3-256 hex digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookSignature(pub String);

impl std::fmt::Display for WebhookSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sha3={}", self.0)
    }
}

/// Sign a payload with a secret key using HMAC-SHA3-256.
pub fn sign_payload(secret: &str, payload: &[u8]) -> WebhookSignature {
    // Simple HMAC construction: H(key XOR opad || H(key XOR ipad || message))
    let key = secret.as_bytes();
    let block_size = 136usize; // SHA3-256 block = 1088 bits = 136 bytes

    let mut padded_key = [0u8; 136];
    let key_to_use = if key.len() > block_size {
        // Hash long keys
        let mut h = Sha3_256::new();
        h.update(key);
        let hashed = h.finalize();
        padded_key[..32].copy_from_slice(&hashed);
        &padded_key[..block_size]
    } else {
        padded_key[..key.len()].copy_from_slice(key);
        &padded_key[..block_size]
    };

    let mut ipad_key = [0u8; 136];
    let mut opad_key = [0u8; 136];
    for i in 0..block_size {
        ipad_key[i] = key_to_use[i] ^ 0x36;
        opad_key[i] = key_to_use[i] ^ 0x5c;
    }

    // Inner hash
    let mut inner = Sha3_256::new();
    inner.update(ipad_key);
    inner.update(payload);
    let inner_hash = inner.finalize();

    // Outer hash
    let mut outer = Sha3_256::new();
    outer.update(opad_key);
    outer.update(inner_hash);
    let result = outer.finalize();

    WebhookSignature(HEXLOWER.encode(&result))
}

/// Verify a payload against a signature string (e.g. "sha3=abc123...").
pub fn verify_payload(secret: &str, payload: &[u8], signature: &str) -> Result<(), WebhookError> {
    let expected = sign_payload(secret, payload);
    let provided = signature.trim_start_matches("sha3=");
    // Constant-time compare
    if constant_time_eq(expected.0.as_bytes(), provided.as_bytes()) {
        Ok(())
    } else {
        Err(WebhookError::InvalidSignature)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let secret = "my-webhook-secret";
        let payload = b"hello world";
        let sig = sign_payload(secret, payload);
        assert!(verify_payload(secret, payload, &sig.to_string()).is_ok());
    }

    #[test]
    fn wrong_secret_fails_verification() {
        let sig = sign_payload("correct-secret", b"data");
        let result = verify_payload("wrong-secret", b"data", &sig.to_string());
        assert!(result.is_err());
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let sig = sign_payload("secret", b"original");
        let result = verify_payload("secret", b"tampered", &sig.to_string());
        assert!(result.is_err());
    }

    #[test]
    fn signature_is_deterministic() {
        let sig1 = sign_payload("s", b"p");
        let sig2 = sign_payload("s", b"p");
        assert_eq!(sig1, sig2);
    }
}
