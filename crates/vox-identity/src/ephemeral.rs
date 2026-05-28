//! Per-job ephemeral Ed25519 subkey for result attestation (P5-T6).

use vox_crypto::{
    SigningKey, VerifyingKey, generate_signing_keypair, sign, to_verifying_key,
    verifying_key_to_bytes,
};

/// A short-lived Ed25519 keypair generated once per submitted job.
///
/// The public key is embedded in the signed `Attestation` envelope so verifiers
/// can check the result signature without trusting a long-lived node key.
pub struct EphemeralSigner {
    /// Hex-encoded compressed Ed25519 public key (32 bytes → 64 hex chars).
    pub pubkey_hex: String,
    sk: SigningKey,
}

impl EphemeralSigner {
    /// Generate a fresh ephemeral signing keypair.
    pub fn new() -> Self {
        let (sk, vk) = generate_signing_keypair();
        let pubkey_hex = hex::encode(verifying_key_to_bytes(&vk));
        Self { pubkey_hex, sk }
    }

    /// Sign `msg` and return the raw 64-byte Ed25519 signature.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        sign(&self.sk, msg)
    }

    /// Return the corresponding verifying key for this ephemeral keypair.
    pub fn verifying_key(&self) -> VerifyingKey {
        to_verifying_key(&self.sk)
    }
}

impl Default for EphemeralSigner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::{verify, verifying_key_from_bytes};

    #[test]
    fn sign_verify_round_trip() {
        let signer = EphemeralSigner::new();
        let msg = b"test attestation payload";
        let sig = signer.sign(msg);

        // Reconstruct the verifying key from the hex-encoded pubkey_hex.
        let pk_bytes: Vec<u8> = hex::decode(&signer.pubkey_hex).expect("hex decode");
        let pk_arr: [u8; 32] = pk_bytes.try_into().expect("32 bytes");
        let vk = verifying_key_from_bytes(&pk_arr).expect("valid key");

        assert!(verify(&vk, msg, &sig), "signature must verify");
    }

    #[test]
    fn different_signers_produce_different_pubkeys() {
        let a = EphemeralSigner::new();
        let b = EphemeralSigner::new();
        assert_ne!(
            a.pubkey_hex, b.pubkey_hex,
            "each signer must have unique pubkey"
        );
    }

    #[test]
    fn wrong_message_fails_verification() {
        let signer = EphemeralSigner::new();
        let sig = signer.sign(b"correct message");

        let pk_bytes: Vec<u8> = hex::decode(&signer.pubkey_hex).expect("hex decode");
        let pk_arr: [u8; 32] = pk_bytes.try_into().expect("32 bytes");
        let vk = verifying_key_from_bytes(&pk_arr).expect("valid key");

        assert!(
            !verify(&vk, b"tampered message", &sig),
            "tampered msg must not verify"
        );
    }
}
