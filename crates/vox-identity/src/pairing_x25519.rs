//! Per-pairing X25519 key derivation (P5-T10).
//!
//! Each node generates a fresh X25519 key for every unique pairing relationship.
//! The resulting shared secret is used as a JWE key-encryption key so only
//! the paired counterpart can decrypt result envelopes.

use vox_crypto::{
    EncryptionPublicKey, EncryptionSecretKey, encryption_public_key_from_bytes,
    encryption_public_key_to_bytes, encryption_secret_key_from_bytes, generate_encryption_keypair,
};

/// A per-pairing X25519 keypair for ECDH-based JWE key derivation.
pub struct PairingKey {
    /// The 32-byte X25519 public key to share with the remote peer.
    pub pubkey_bytes: [u8; 32],
    sk: EncryptionSecretKey,
}

impl PairingKey {
    /// Generate a fresh random X25519 keypair for a new pairing.
    pub fn generate() -> Self {
        let (sk, pk) = generate_encryption_keypair();
        let pubkey_bytes = encryption_public_key_to_bytes(&pk);
        Self { pubkey_bytes, sk }
    }

    /// Reconstruct a `PairingKey` from stored secret-key bytes.
    ///
    /// The caller is responsible for keeping `sk_bytes` secret.
    pub fn from_bytes(sk_bytes: [u8; 32], pubkey_bytes: [u8; 32]) -> Self {
        Self {
            pubkey_bytes,
            sk: encryption_secret_key_from_bytes(sk_bytes),
        }
    }

    /// Derive the 32-byte shared secret via X25519 ECDH with `their_pubkey`.
    ///
    /// Both parties compute the same secret when each uses their own private key
    /// and the other's public key.
    pub fn shared_secret(&self, their_pubkey: &[u8; 32]) -> [u8; 32] {
        let their_pk: EncryptionPublicKey = encryption_public_key_from_bytes(*their_pubkey);
        // x25519_dalek DH is accessed through the EncryptionSecretKey wrapper.
        // We derive via the same Diffie-Hellman path used by vox_crypto::seal().
        self.sk.0.diffie_hellman(&their_pk.0).to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecdh_shared_secret_is_symmetric() {
        let alice = PairingKey::generate();
        let bob = PairingKey::generate();

        let alice_secret = alice.shared_secret(&bob.pubkey_bytes);
        let bob_secret = bob.shared_secret(&alice.pubkey_bytes);

        assert_eq!(alice_secret, bob_secret, "ECDH must be symmetric");
    }

    #[test]
    fn different_pairings_produce_different_secrets() {
        let alice = PairingKey::generate();
        let bob = PairingKey::generate();
        let carol = PairingKey::generate();

        let ab = alice.shared_secret(&bob.pubkey_bytes);
        let ac = alice.shared_secret(&carol.pubkey_bytes);
        assert_ne!(ab, ac, "distinct pairings must yield distinct secrets");
    }

    #[test]
    fn pubkey_bytes_are_32_bytes() {
        let key = PairingKey::generate();
        assert_eq!(key.pubkey_bytes.len(), 32);
    }
}
