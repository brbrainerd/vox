use rand::RngCore;
use vox_crypto::{VerifyingKey, verify};

pub fn generate_challenge() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

pub fn verify_challenge_response(
    verifying_key: &VerifyingKey,
    nonce: &[u8; 32],
    signature: &[u8; 64],
) -> bool {
    // Basic verification of the nonce
    // In a real protocol, the message might include a timestamp or context
    verify(verifying_key, nonce, signature)
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;
    use vox_crypto::{generate_signing_keypair, sign, to_verifying_key};

    #[test]
    fn generate_challenge_is_32_bytes() {
        let nonce = generate_challenge();
        assert_eq!(nonce.len(), 32);
    }

    #[test]
    fn generate_challenge_produces_distinct_nonces() {
        let a = generate_challenge();
        let b = generate_challenge();
        assert_ne!(a, b, "two consecutive challenges should differ");
    }

    #[test]
    fn verify_challenge_response_accepts_valid_signature() {
        let (sk, vk) = generate_signing_keypair();
        let nonce = generate_challenge();
        let sig = sign(&sk, &nonce);
        assert!(verify_challenge_response(&vk, &nonce, &sig));
    }

    #[test]
    fn verify_challenge_response_rejects_wrong_nonce() {
        let (sk, vk) = generate_signing_keypair();
        let nonce = generate_challenge();
        let sig = sign(&sk, &nonce);
        let mut bad_nonce = nonce;
        bad_nonce[0] ^= 0xff;
        assert!(!verify_challenge_response(&vk, &bad_nonce, &sig));
    }

    #[test]
    fn verify_challenge_response_rejects_wrong_key() {
        let (sk, _vk) = generate_signing_keypair();
        let (_sk2, vk2) = generate_signing_keypair();
        let nonce = generate_challenge();
        let sig = sign(&sk, &nonce);
        assert!(!verify_challenge_response(&vk2, &nonce, &sig));
    }
}
