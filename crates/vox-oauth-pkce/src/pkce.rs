//! RFC 7636 PKCE code_verifier/code_challenge generation.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// A generated PKCE pair: the secret verifier (kept in-process) and its
/// S256 challenge (sent to the authorization server).
///
/// `Debug` is hand-written to **redact `verifier`** so the code verifier — a
/// bearer-adjacent OAuth secret — can never leak into logs/traces via
/// `tracing::debug!(?pair)`. `challenge` is not secret (it's sent to the
/// authorization server in the clear) and stays visible.
#[derive(Clone)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

impl std::fmt::Debug for PkcePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkcePair")
            .field(
                "verifier",
                &format!("[redacted len={}]", self.verifier.len()),
            )
            .field("challenge", &self.challenge)
            .finish()
    }
}

/// Generate a new PKCE pair using a 64-byte random verifier (RFC 7636 §4.1
/// requires 43-128 chars of unreserved base64url chars; 64 raw bytes ->
/// 86 base64url chars, comfortably in range) and its S256 challenge.
pub fn generate() -> PkcePair {
    let mut raw = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut raw);
    let verifier = URL_SAFE_NO_PAD.encode(raw);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    PkcePair {
        verifier,
        challenge,
    }
}

/// Generate a random `state` value (32 bytes, base64url) for CSRF binding.
pub fn generate_state() -> String {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    URL_SAFE_NO_PAD.encode(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_in_rfc7636_length_range() {
        let pair = generate();
        assert!(pair.verifier.len() >= 43 && pair.verifier.len() <= 128);
        assert!(
            pair.verifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "verifier must be base64url (unreserved chars only): {}",
            pair.verifier
        );
    }

    #[test]
    fn debug_redacts_verifier() {
        let pair = generate();
        let dbg = format!("{pair:?}");
        assert!(
            !dbg.contains(&pair.verifier),
            "verifier must never appear in Debug: {dbg}"
        );
        assert!(
            dbg.contains("[redacted"),
            "verifier should render redacted: {dbg}"
        );
        assert!(
            dbg.contains(&pair.challenge),
            "challenge is not secret and should stay visible: {dbg}"
        );
    }

    #[test]
    fn challenge_is_sha256_of_verifier() {
        let pair = generate();
        let mut hasher = Sha256::new();
        hasher.update(pair.verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(pair.challenge, expected);
    }

    #[test]
    fn two_calls_produce_different_verifiers() {
        let a = generate();
        let b = generate();
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn state_is_nonempty_and_varies() {
        let a = generate_state();
        let b = generate_state();
        assert!(!a.is_empty());
        assert_ne!(a, b);
    }
}
