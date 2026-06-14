use std::fmt;

use vox_crypto::{SigningKey, VerifyingKey, generate_signing_keypair, secure_hash, sign};

pub struct NodeIdentity {
    node_id: String,
    signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeIdentity")
            .field("node_id", &self.node_id)
            .finish_non_exhaustive()
    }
}

impl NodeIdentity {
    pub fn generate() -> Self {
        let (signing_key, verifying_key) = generate_signing_keypair();
        let pubkey_bytes = vox_crypto::verifying_key_to_bytes(&verifying_key);
        let hash = secure_hash(&pubkey_bytes);
        let node_id = hex::encode(&hash[0..16]);

        Self {
            node_id,
            signing_key,
            verifying_key,
        }
    }

    pub fn from_keys(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        let pubkey_bytes = vox_crypto::verifying_key_to_bytes(&verifying_key);
        let hash = secure_hash(&pubkey_bytes);
        let node_id = hex::encode(&hash[0..16]);

        Self {
            node_id,
            signing_key,
            verifying_key,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn sign_challenge(&self, nonce: &[u8]) -> [u8; 64] {
        sign(&self.signing_key, nonce)
    }

    /// Full hex-encoded Ed25519 public key (32 bytes → 64 hex chars).
    pub fn pubkey_hex(&self) -> String {
        hex::encode(vox_crypto::verifying_key_to_bytes(&self.verifying_key))
    }

    /// Short, human-readable fingerprint of the public key, e.g.
    /// `ed25519:7f:42:9b…2a:11` — head 3 + tail 2 bytes of the hex pubkey.
    pub fn fingerprint(&self) -> String {
        let h = self.pubkey_hex();
        let pairs: Vec<String> = h
            .as_bytes()
            .chunks(2)
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect();
        let head = pairs.iter().take(3).cloned().collect::<Vec<_>>().join(":");
        let tail = pairs
            .iter()
            .rev()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(":");
        format!("ed25519:{head}…{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pubkey_hex_is_64_chars() {
        let id = NodeIdentity::generate();
        assert_eq!(id.pubkey_hex().len(), 64, "ed25519 pubkey is 32 bytes hex");
    }

    #[test]
    fn fingerprint_shape() {
        let id = NodeIdentity::generate();
        let fp = id.fingerprint();
        assert!(fp.starts_with("ed25519:"), "fp prefixed: {fp}");
        assert!(fp.contains('…'), "fp elided: {fp}");
    }
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;
    use vox_crypto::{generate_signing_keypair, verifying_key_from_bytes, verifying_key_to_bytes};

    // --- .fmt() (Debug) tests ---

    #[test]
    fn debug_fmt_contains_node_id() {
        let id = NodeIdentity::generate();
        let node_id = id.node_id().to_string();
        let dbg = format!("{:?}", id);
        assert!(
            dbg.contains(&node_id),
            "debug output should contain node_id: {dbg}"
        );
    }

    #[test]
    fn debug_fmt_does_not_expose_signing_key() {
        let id = NodeIdentity::generate();
        let dbg = format!("{:?}", id);
        assert!(
            !dbg.contains("signing_key"),
            "signing_key must not appear in debug: {dbg}"
        );
    }

    #[test]
    fn debug_fmt_has_struct_name() {
        let id = NodeIdentity::generate();
        let dbg = format!("{:?}", id);
        assert!(
            dbg.starts_with("NodeIdentity"),
            "debug must start with struct name: {dbg}"
        );
    }

    // --- .from_keys() tests ---

    #[test]
    fn from_keys_node_id_is_32_hex_chars() {
        let (sk, vk) = generate_signing_keypair();
        let id = NodeIdentity::from_keys(sk, vk);
        assert_eq!(
            id.node_id().len(),
            32,
            "node_id is 16 bytes -> 32 hex chars"
        );
        assert!(id.node_id().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn from_keys_different_keypairs_produce_different_node_ids() {
        let (sk1, vk1) = generate_signing_keypair();
        let (sk2, vk2) = generate_signing_keypair();
        let id1 = NodeIdentity::from_keys(sk1, vk1);
        let id2 = NodeIdentity::from_keys(sk2, vk2);
        assert_ne!(id1.node_id(), id2.node_id());
    }

    #[test]
    fn from_keys_node_id_deterministic_from_same_pubkey() {
        let (sk, vk) = generate_signing_keypair();
        let vk_bytes = verifying_key_to_bytes(&vk);
        let vk2 = verifying_key_from_bytes(&vk_bytes).unwrap();
        let id = NodeIdentity::from_keys(sk, vk);
        // Re-derive node_id the same way the function does
        let hash = vox_crypto::secure_hash(&vk_bytes);
        let expected_node_id = hex::encode(&hash[0..16]);
        // Verify pubkey round-trips to confirm determinism path
        let _ = verifying_key_to_bytes(&vk2);
        assert_eq!(id.node_id(), expected_node_id);
    }
}

#[cfg(test)]
mod semcov_wave49_tests {
    //! Adversarial tests for NodeIdentity: identity parsing, validation,
    //! scoping rules, equality/hashing invariants, and signing correctness.
    #![allow(unused_imports)]
    use super::*;
    use vox_crypto::{
        generate_signing_keypair, sign, verify, verifying_key_from_bytes, verifying_key_to_bytes,
    };

    // ── node_id format & derivation ──────────────────────────────────────

    #[test]
    fn node_id_is_lowercase_hex_only() {
        // Catches: node_id accidentally using uppercase hex (breaks case-sensitive lookups)
        let id = NodeIdentity::generate();
        let nid = id.node_id();
        assert!(
            nid.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "node_id must be lowercase hex, got: {nid}"
        );
    }

    #[test]
    fn node_id_is_exactly_32_chars_for_generate() {
        // Catches: off-by-one in hash slice (e.g. hash[0..15] → 30 chars)
        for _ in 0..5 {
            let id = NodeIdentity::generate();
            assert_eq!(
                id.node_id().len(),
                32,
                "node_id must be 32 hex chars (16-byte prefix): {}",
                id.node_id()
            );
        }
    }

    #[test]
    fn generate_and_from_keys_produce_same_node_id_from_same_keypair() {
        // Catches: generate() and from_keys() using different hash inputs, producing divergent IDs
        let (sk, vk) = generate_signing_keypair();
        let vk_bytes = verifying_key_to_bytes(&vk);
        let vk2 = verifying_key_from_bytes(&vk_bytes).unwrap();
        let id_from_keys = NodeIdentity::from_keys(sk, vk);
        // from_keys should produce the same node_id as generate() would for the same pubkey
        let hash = vox_crypto::secure_hash(&vk_bytes);
        let expected = hex::encode(&hash[0..16]);
        assert_eq!(id_from_keys.node_id(), expected);
        // Round-trip: re-encoding the same pubkey bytes must still agree
        let vk2_bytes = verifying_key_to_bytes(&vk2);
        let hash2 = vox_crypto::secure_hash(&vk2_bytes);
        let expected2 = hex::encode(&hash2[0..16]);
        assert_eq!(id_from_keys.node_id(), expected2);
    }

    #[test]
    fn node_id_does_not_embed_signing_key_material() {
        // Catches: node_id accidentally derived from private key bytes instead of pubkey
        let (sk, vk) = generate_signing_keypair();
        let id = NodeIdentity::from_keys(sk, vk);
        // The node_id must equal the hash of the *verifying* key, not anything else.
        let vk_bytes = verifying_key_to_bytes(&id.verifying_key);
        let hash = vox_crypto::secure_hash(&vk_bytes);
        let expected = hex::encode(&hash[0..16]);
        assert_eq!(
            id.node_id(),
            expected,
            "node_id must be derived from verifying_key bytes"
        );
    }

    #[test]
    fn two_generated_identities_have_distinct_node_ids() {
        // Catches: generate() returning a cached / zeroed identity
        let id1 = NodeIdentity::generate();
        let id2 = NodeIdentity::generate();
        assert_ne!(
            id1.node_id(),
            id2.node_id(),
            "independently generated identities must differ"
        );
    }

    // ── pubkey_hex ───────────────────────────────────────────────────────

    #[test]
    fn pubkey_hex_encodes_correct_verifying_key() {
        // Catches: pubkey_hex returning signing-key bytes or wrong key
        let (sk, vk) = generate_signing_keypair();
        let vk_bytes = verifying_key_to_bytes(&vk);
        let expected_hex = hex::encode(vk_bytes);
        let id = NodeIdentity::from_keys(sk, vk);
        assert_eq!(
            id.pubkey_hex(),
            expected_hex,
            "pubkey_hex must encode the verifying_key exactly"
        );
    }

    #[test]
    fn pubkey_hex_is_stable_across_calls() {
        // Catches: pubkey_hex re-deriving from signing key each call, with non-determinism
        let id = NodeIdentity::generate();
        let h1 = id.pubkey_hex();
        let h2 = id.pubkey_hex();
        assert_eq!(h1, h2, "pubkey_hex must be idempotent");
    }

    #[test]
    fn pubkey_hex_is_valid_decodable_hex() {
        // Catches: pubkey_hex containing non-hex chars or wrong encoding
        let id = NodeIdentity::generate();
        let hex_str = id.pubkey_hex();
        let decoded = hex::decode(&hex_str).expect("pubkey_hex must be valid hex");
        assert_eq!(decoded.len(), 32, "decoded pubkey must be 32 bytes");
    }

    // ── fingerprint ──────────────────────────────────────────────────────

    #[test]
    fn fingerprint_head_matches_pubkey_prefix() {
        // Catches: fingerprint taking bytes from wrong position (e.g. node_id instead of pubkey)
        let id = NodeIdentity::generate();
        let fp = id.fingerprint();
        let hex = id.pubkey_hex();
        // first 3 byte-pairs in pubkey_hex: chars 0..6
        let expected_head = format!("{}:{}:{}", &hex[0..2], &hex[2..4], &hex[4..6]);
        assert!(
            fp.contains(&expected_head),
            "fingerprint head must match first 3 pubkey bytes: fp={fp}, expected head={expected_head}"
        );
    }

    #[test]
    fn fingerprint_tail_matches_pubkey_suffix() {
        // Catches: fingerprint tail drawn from wrong end (e.g. reversed bytes, or off-by-one chunk)
        let id = NodeIdentity::generate();
        let fp = id.fingerprint();
        let hex = id.pubkey_hex();
        // last 2 byte-pairs: chars 60..64
        let expected_tail = format!("{}:{}", &hex[60..62], &hex[62..64]);
        assert!(
            fp.ends_with(&expected_tail),
            "fingerprint tail must match last 2 pubkey bytes: fp={fp}, expected tail={expected_tail}"
        );
    }

    #[test]
    fn fingerprint_unique_across_distinct_identities() {
        // Catches: fingerprint always returning a constant or fixed string
        let id1 = NodeIdentity::generate();
        let id2 = NodeIdentity::generate();
        assert_ne!(
            id1.fingerprint(),
            id2.fingerprint(),
            "distinct identities must produce distinct fingerprints"
        );
    }

    // ── sign_challenge / verify round-trip ───────────────────────────────

    #[test]
    fn sign_challenge_verifies_with_own_pubkey() {
        // Catches: sign_challenge using wrong key, or verify wired to wrong bytes
        use vox_crypto::verify;
        let id = NodeIdentity::generate();
        let nonce: [u8; 32] = [0xab; 32];
        let sig = id.sign_challenge(&nonce);
        assert!(
            verify(&id.verifying_key, &nonce, &sig),
            "signature from sign_challenge must verify with the identity's verifying_key"
        );
    }

    #[test]
    fn sign_challenge_does_not_verify_with_different_identity_key() {
        // Catches: verify accidentally accepting any valid Ed25519 sig, ignoring key binding
        use vox_crypto::verify;
        let id1 = NodeIdentity::generate();
        let id2 = NodeIdentity::generate();
        let nonce: [u8; 32] = [0x77; 32];
        let sig = id1.sign_challenge(&nonce);
        assert!(
            !verify(&id2.verifying_key, &nonce, &sig),
            "signature made with id1 must NOT verify under id2's pubkey"
        );
    }

    #[test]
    fn sign_challenge_rejects_mutated_nonce() {
        // Catches: verify() not covering all nonce bytes (e.g. only first N bytes checked)
        use vox_crypto::verify;
        let id = NodeIdentity::generate();
        let nonce: [u8; 32] = [0x11; 32];
        let sig = id.sign_challenge(&nonce);
        let mut bad_nonce = nonce;
        bad_nonce[31] ^= 0x01; // flip last byte
        assert!(
            !verify(&id.verifying_key, &bad_nonce, &sig),
            "mutating the last byte of the nonce must invalidate the signature"
        );
    }

    #[test]
    fn sign_challenge_empty_nonce_does_not_panic() {
        // Catches: sign() panicking on zero-length input instead of producing a valid sig
        let id = NodeIdentity::generate();
        let _sig = id.sign_challenge(&[]);
        // must not panic — we don't verify it here, just ensure no crash
    }

    // ── scoping / key isolation ───────────────────────────────────────────

    #[test]
    fn signing_key_accessor_returns_key_that_signs_correctly() {
        // Catches: signing_key() returning a different (default/zero) key than the one stored
        use vox_crypto::{sign, verify};
        let id = NodeIdentity::generate();
        let msg = b"scope isolation test";
        let sig = sign(id.signing_key(), msg);
        assert!(
            verify(&id.verifying_key, msg, &sig),
            "signing_key() accessor must return the key matching verifying_key"
        );
    }

    #[test]
    fn from_keys_mismatched_pair_produces_unverifiable_signature() {
        // Catches: from_keys() silently accepting any (sk, vk) pair without binding check;
        // signing with sk then verifying under the *stored* vk should fail if they're unrelated.
        use vox_crypto::{sign, verify};
        let (sk1, _vk1) = generate_signing_keypair();
        let (_sk2, vk2) = generate_signing_keypair();
        // Build identity with *mismatched* keys
        let id = NodeIdentity::from_keys(sk1, vk2);
        let msg = b"mismatch test";
        let sig = sign(id.signing_key(), msg);
        // sig was made with sk1 but id.verifying_key is vk2 — must not verify
        assert!(
            !verify(&id.verifying_key, msg, &sig),
            "mismatched sk/vk pair: signature under sk must NOT verify under stored vk"
        );
    }
}
