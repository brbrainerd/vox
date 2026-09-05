//! JSON Web Encryption (JWE) engine for secure Agent-to-Agent (A2A) payload proxying over the Mesh.
//! Implements a subset of RFC 7516 (dir + A256GCM) for zero-knowledge transit of Vault secrets.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;

/// Hardcoded Base64URL-encoded header for `{"alg":"dir","enc":"A256GCM"}`
const JWE_HEADER_B64: &str = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0";

#[derive(Debug, thiserror::Error)]
pub enum JweError {
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Invalid JWE format")]
    InvalidFormat,
    #[error("Decoding error: {0}")]
    Decode(String),
    #[error("Missing key logic")]
    MissingKey,
}

/// Encrypt string payload into a Compact JWE string using `dir` (direct) routing and AES-256-GCM.
///
/// `symmetric_key` must be exactly 32 bytes (256 bits).
pub fn encrypt_jwe_compact(payload: &[u8], symmetric_key: &[u8; 32]) -> Result<String, JweError> {
    let cipher = Aes256Gcm::new(symmetric_key.into());

    let mut iv = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv); // 96-bits

    // In JWE using A256GCM, the Additional Authenticated Data (AAD) is the ASCII representation
    // of the Base64URL-encoded header string.
    let aad = JWE_HEADER_B64.as_bytes();

    let payload_to_encrypt = Payload { msg: payload, aad };

    let encrypted = cipher
        .encrypt(nonce, payload_to_encrypt)
        .map_err(|e| JweError::Crypto(e.to_string()))?;

    // The Rust `aes_gcm` crate appends the 16-byte authentication tag to the ciphertext.
    if encrypted.len() < 16 {
        return Err(JweError::Crypto("Ciphertext too small for tag".to_string()));
    }

    let ct_len = encrypted.len() - 16;
    let ciphertext = &encrypted[..ct_len];
    let tag = &encrypted[ct_len..];

    let iv_b64 = URL_SAFE_NO_PAD.encode(iv);
    let ct_b64 = URL_SAFE_NO_PAD.encode(ciphertext);
    let tag_b64 = URL_SAFE_NO_PAD.encode(tag);

    // Format: BASE64URL(UTF8(JWE Protected Header)) || '.' ||
    //         BASE64URL(JWE Encrypted Key) || '.' ||
    //         BASE64URL(JWE Initialization Vector) || '.' ||
    //         BASE64URL(JWE Ciphertext) || '.' ||
    //         BASE64URL(JWE Authentication Tag)
    //
    // For 'dir', the Encrypted Key is empty.
    Ok(format!("{JWE_HEADER_B64}..{iv_b64}.{ct_b64}.{tag_b64}"))
}

/// Decrypt a Compact JWE string using `dir` (direct) routing and AES-256-GCM.
pub fn decrypt_jwe_compact(
    jwe_string: &str,
    symmetric_key: &[u8; 32],
) -> Result<Vec<u8>, JweError> {
    let parts: Vec<&str> = jwe_string.split('.').collect();
    if parts.len() != 5 {
        return Err(JweError::InvalidFormat);
    }

    let header_b64 = parts[0];
    // parts[1] is the empty Encrypted Key
    let iv_b64 = parts[2];
    let ct_b64 = parts[3];
    let tag_b64 = parts[4];

    if header_b64 != JWE_HEADER_B64 {
        return Err(JweError::InvalidFormat); // Only strictly accepting our predefined header
    }

    let iv = URL_SAFE_NO_PAD
        .decode(iv_b64)
        .map_err(|e| JweError::Decode(e.to_string()))?;
    if iv.len() != 12 {
        return Err(JweError::InvalidFormat);
    }

    let ciphertext = URL_SAFE_NO_PAD
        .decode(ct_b64)
        .map_err(|e| JweError::Decode(e.to_string()))?;
    let tag = URL_SAFE_NO_PAD
        .decode(tag_b64)
        .map_err(|e| JweError::Decode(e.to_string()))?;

    if tag.len() != 16 {
        return Err(JweError::InvalidFormat);
    }

    // `aes_gcm` expects the payload to match (ciphertext || tag)
    let mut encrypted_combined = Vec::with_capacity(ciphertext.len() + tag.len());
    encrypted_combined.extend_from_slice(&ciphertext);
    encrypted_combined.extend_from_slice(&tag);

    let cipher = Aes256Gcm::new(symmetric_key.into());
    let nonce = Nonce::from_slice(&iv);
    let aad = header_b64.as_bytes();

    let payload = Payload {
        msg: &encrypted_combined,
        aad,
    };

    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|e| JweError::Crypto(e.to_string()))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwe_compact_round_trip() {
        let key = [0x42; 32];
        let payload = b"Hello, Mesh! This is a secret API key envelope.";

        // Encrypt
        let jwe_str = encrypt_jwe_compact(payload.as_ref(), &key).expect("Encryption failed");
        assert!(jwe_str.starts_with("eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..")); // empty encrypted key
        assert_eq!(jwe_str.split('.').count(), 5);

        // Decrypt
        let decrypted = decrypt_jwe_compact(&jwe_str, &key).expect("Decryption failed");
        assert_eq!(payload.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_jwe_invalid_format() {
        let key = [0x42; 32];
        assert!(matches!(
            decrypt_jwe_compact("invalid.jwe", &key),
            Err(JweError::InvalidFormat)
        ));
    }
}
