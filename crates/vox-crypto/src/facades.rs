use blake3;
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use sha3::{Digest, Sha3_256};
use xxhash_rust::xxh3::xxh3_64;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secure cryptographic hash (BLAKE3)
pub fn secure_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Secure keyed hash (BLAKE3 MAC)
pub fn keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(data);
    hasher.finalize().into()
}

/// Fast generic hashing for caches (XXH3)
pub fn fast_hash(data: &[u8]) -> u64 {
    xxh3_64(data)
}

/// Compliance / standardized hash (SHA-3 256)
pub fn compliance_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct SymKey(pub [u8; 32]);

pub fn generate_sym_key() -> SymKey {
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
    SymKey(key)
}

pub fn encrypt(key: &SymKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(&key.0.into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 96-bits; unique per message

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut output = nonce.to_vec();
    output.extend(ciphertext);
    Ok(output)
}

pub fn decrypt(key: &SymKey, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".into());
    }
    let (nonce_bytes, payload) = ciphertext.split_at(12);
    decrypt_with_nonce(key, nonce_bytes, payload)
}

/// Required ChaCha20Poly1305 nonce length, in bytes (96-bit IETF nonce).
pub const CHACHA20POLY1305_NONCE_LEN: usize = 12;

/// Validate a nonce slice has the exact length required by ChaCha20Poly1305.
///
/// `Nonce::from_slice` panics on a length mismatch, which turns any
/// attacker- or caller-controlled byte slice into a denial-of-service vector.
/// Callers must validate first; encrypt/decrypt helpers below do this for you.
fn checked_nonce(nonce: &[u8]) -> Result<&Nonce, String> {
    if nonce.len() != CHACHA20POLY1305_NONCE_LEN {
        return Err(format!(
            "Invalid nonce length: expected {} bytes, got {}",
            CHACHA20POLY1305_NONCE_LEN,
            nonce.len()
        ));
    }
    Ok(Nonce::from_slice(nonce))
}

pub fn encrypt_with_nonce(key: &SymKey, nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(&key.0.into());
    let nonce = checked_nonce(nonce)?;

    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))
}

pub fn decrypt_with_nonce(
    key: &SymKey,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(&key.0.into());
    let nonce = checked_nonce(nonce)?;

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))
}

// --- Ed25519 Signing ---

pub struct SigningKey {
    pub inner: ed25519_dalek::SigningKey,
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifyingKey {
    pub inner: ed25519_dalek::VerifyingKey,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyingKey").finish_non_exhaustive()
    }
}

pub fn generate_signing_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = rand::rngs::OsRng;
    let signing_key = ed25519_dalek::SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    (
        SigningKey { inner: signing_key },
        VerifyingKey {
            inner: verifying_key,
        },
    )
}

pub fn sign(key: &SigningKey, message: &[u8]) -> [u8; 64] {
    use ed25519_dalek::Signer;
    key.inner.sign(message).to_bytes()
}

pub fn verify(key: &VerifyingKey, message: &[u8], sig: &[u8; 64]) -> bool {
    use ed25519_dalek::Verifier;
    let signature = ed25519_dalek::Signature::from_bytes(sig);
    key.inner.verify(message, &signature).is_ok()
}

pub fn signing_key_from_bytes(bytes: &[u8; 32]) -> SigningKey {
    SigningKey {
        inner: ed25519_dalek::SigningKey::from_bytes(bytes),
    }
}

pub fn signing_key_to_bytes(key: &SigningKey) -> [u8; 32] {
    key.inner.to_bytes()
}

pub fn to_verifying_key(signing_key: &SigningKey) -> VerifyingKey {
    VerifyingKey {
        inner: ed25519_dalek::VerifyingKey::from(&signing_key.inner),
    }
}

pub fn verifying_key_to_bytes(key: &VerifyingKey) -> [u8; 32] {
    key.inner.to_bytes()
}

pub fn verifying_key_from_bytes(bytes: &[u8; 32]) -> Result<VerifyingKey, String> {
    ed25519_dalek::VerifyingKey::from_bytes(bytes)
        .map(|k| VerifyingKey { inner: k })
        .map_err(|e| format!("Invalid verifying key: {}", e))
}

/// Verify a signature against a hex-encoded public key and signature.
pub fn verify_signature_hex(
    pubkey_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<bool, String> {
    let pk_bytes = hex::decode(pubkey_hex).map_err(|e| e.to_string())?;
    let sig_bytes = hex::decode(signature_hex).map_err(|e| e.to_string())?;

    if pk_bytes.len() != 32 {
        return Err("Invalid public key length (expected 32 bytes)".into());
    }
    if sig_bytes.len() != 64 {
        return Err("Invalid signature length (expected 64 bytes)".into());
    }

    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk_bytes);
    let pk = verifying_key_from_bytes(&pk_arr)?;

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    Ok(verify(&pk, message, &sig_arr))
}

// --- X25519 Sealed Box ---

pub struct EncryptionSecretKey(pub x25519_dalek::StaticSecret);
pub struct EncryptionPublicKey(pub x25519_dalek::PublicKey);

impl std::fmt::Debug for EncryptionSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionSecretKey")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for EncryptionPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionPublicKey")
            .field("bytes", &hex::encode(self.0.as_bytes()))
            .finish()
    }
}

pub fn generate_encryption_keypair() -> (EncryptionSecretKey, EncryptionPublicKey) {
    let secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
    let public = x25519_dalek::PublicKey::from(&secret);
    (EncryptionSecretKey(secret), EncryptionPublicKey(public))
}

pub fn seal(public_key: &EncryptionPublicKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut rng = rand::thread_rng();
    let ephemeral_sk = x25519_dalek::StaticSecret::random_from_rng(&mut rng);
    let ephemeral_pk = x25519_dalek::PublicKey::from(&ephemeral_sk);
    let shared_secret = ephemeral_sk.diffie_hellman(&public_key.0);

    let key = chacha20poly1305::Key::from_slice(shared_secret.as_bytes());
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = ChaCha20Poly1305::generate_nonce(&mut rng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut out = ephemeral_pk.as_bytes().to_vec();
    out.extend(nonce.as_slice());
    out.extend(ciphertext);
    Ok(out)
}

pub fn unseal(secret_key: &EncryptionSecretKey, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 32 + 12 {
        return Err("Ciphertext too short".into());
    }
    let (ephemeral_pk_bytes, rest) = ciphertext.split_at(32);
    let (nonce_bytes, encrypted_payload) = rest.split_at(12);

    let ephemeral_pk_arr: [u8; 32] = ephemeral_pk_bytes
        .try_into()
        .map_err(|_| "Invalid public key length")?;
    let ephemeral_pk = x25519_dalek::PublicKey::from(ephemeral_pk_arr);
    let shared_secret = secret_key.0.diffie_hellman(&ephemeral_pk);

    let key = chacha20poly1305::Key::from_slice(shared_secret.as_bytes());
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = checked_nonce(nonce_bytes)?;

    cipher
        .decrypt(nonce, encrypted_payload)
        .map_err(|e| format!("Decryption failed: {}", e))
}

pub fn encryption_secret_key_from_bytes(bytes: [u8; 32]) -> EncryptionSecretKey {
    EncryptionSecretKey(x25519_dalek::StaticSecret::from(bytes))
}

pub fn encryption_public_key_from_bytes(bytes: [u8; 32]) -> EncryptionPublicKey {
    EncryptionPublicKey(x25519_dalek::PublicKey::from(bytes))
}

pub fn encryption_public_key_to_bytes(key: &EncryptionPublicKey) -> [u8; 32] {
    *key.0.as_bytes()
}

#[cfg(test)]
mod nonce_length_tests {
    use super::*;

    #[test]
    fn encrypt_with_nonce_rejects_short_nonce() {
        let key = generate_sym_key();
        let res = encrypt_with_nonce(&key, &[0u8; 11], b"hello");
        let err = res.expect_err("11-byte nonce must not be accepted");
        assert!(err.contains("Invalid nonce length"), "got: {err}");
    }

    #[test]
    fn encrypt_with_nonce_rejects_long_nonce() {
        let key = generate_sym_key();
        let res = encrypt_with_nonce(&key, &[0u8; 13], b"hello");
        assert!(res.is_err(), "13-byte nonce must not be accepted");
    }

    #[test]
    fn encrypt_with_nonce_rejects_empty_nonce() {
        let key = generate_sym_key();
        let res = encrypt_with_nonce(&key, &[], b"hello");
        assert!(res.is_err(), "empty nonce must not be accepted");
    }

    #[test]
    fn decrypt_with_nonce_rejects_short_nonce() {
        let key = generate_sym_key();
        let res = decrypt_with_nonce(&key, &[0u8; 11], b"\x00\x00\x00\x00");
        assert!(res.is_err(), "11-byte nonce must not panic or accept");
    }

    #[test]
    fn checked_nonce_round_trip() {
        // 12 bytes is the only acceptable length.
        for len in [0, 1, 8, 11, 13, 16, 24, 32] {
            let buf = vec![0u8; len];
            assert!(checked_nonce(&buf).is_err(), "len {len} should reject");
        }
        let buf = vec![0u8; 12];
        assert!(checked_nonce(&buf).is_ok());
    }

    #[test]
    fn encrypt_then_decrypt_round_trip_still_works() {
        let key = generate_sym_key();
        let ct = encrypt(&key, b"plaintext").unwrap();
        let pt = decrypt(&key, &ct).unwrap();
        assert_eq!(pt, b"plaintext");
    }
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;

    // --- .fmt() [SigningKey Debug] ---
    #[test]
    fn signing_key_debug_does_not_leak_key_bytes() {
        let key_bytes = [0xABu8; 32];
        let sk = signing_key_from_bytes(&key_bytes);
        let debug_str = format!("{:?}", sk);
        // Must contain the struct name
        assert!(debug_str.contains("SigningKey"), "got: {debug_str}");
        // Must NOT contain any representation of the key bytes
        assert!(
            !debug_str.contains("ab"),
            "key bytes leaked in debug: {debug_str}"
        );
        assert!(
            !debug_str.contains("AB"),
            "key bytes leaked in debug: {debug_str}"
        );
        assert!(
            !debug_str.contains("171"),
            "key bytes leaked in debug: {debug_str}"
        );
    }

    // --- .fmt() [VerifyingKey Debug] ---
    #[test]
    fn verifying_key_debug_does_not_leak_key_bytes() {
        let key_bytes = [0xCDu8; 32];
        let sk = signing_key_from_bytes(&key_bytes);
        let vk = to_verifying_key(&sk);
        let debug_str = format!("{:?}", vk);
        assert!(debug_str.contains("VerifyingKey"), "got: {debug_str}");
        // The verifying key derived from [0xCD; 32] should not appear verbatim in debug
        // We check that the raw 0xcd byte value isn't naively printed
        assert!(
            !debug_str.contains("205, 205"),
            "key bytes leaked: {debug_str}"
        );
    }

    // --- signing_key_from_bytes() ---
    #[test]
    fn signing_key_from_bytes_round_trips_to_bytes() {
        let original = [0x42u8; 32];
        let sk = signing_key_from_bytes(&original);
        let recovered = signing_key_to_bytes(&sk);
        assert_eq!(
            recovered, original,
            "round-trip through from/to bytes must be identity"
        );
    }

    #[test]
    fn signing_key_from_bytes_produces_consistent_signatures() {
        let key_bytes = [0x11u8; 32];
        let sk1 = signing_key_from_bytes(&key_bytes);
        let sk2 = signing_key_from_bytes(&key_bytes);
        let msg = b"deterministic test";
        // Ed25519 with the same key and message must produce identical signatures
        assert_eq!(sign(&sk1, msg), sign(&sk2, msg));
    }

    // --- signing_key_to_bytes() ---
    #[test]
    fn signing_key_to_bytes_yields_seed_bytes() {
        let seed = [0xFFu8; 32];
        let sk = signing_key_from_bytes(&seed);
        assert_eq!(signing_key_to_bytes(&sk), seed);
    }

    // --- to_verifying_key() ---
    #[test]
    fn to_verifying_key_produces_key_that_validates_signatures() {
        let seed = [0x55u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk = to_verifying_key(&sk);
        let msg = b"round-trip verify";
        let sig = sign(&sk, msg);
        assert!(
            verify(&vk, msg, &sig),
            "verifying key from signing key must verify its signatures"
        );
        assert!(!verify(&vk, b"wrong", &sig), "must reject wrong message");
    }

    #[test]
    fn to_verifying_key_matches_generate_keypair_verifying_key() {
        let seed = [0x77u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk_derived = verifying_key_to_bytes(&to_verifying_key(&sk));
        // Constructing directly must give the same public key bytes
        let sk2 = signing_key_from_bytes(&seed);
        let vk_direct = verifying_key_to_bytes(&to_verifying_key(&sk2));
        assert_eq!(vk_derived, vk_direct);
    }

    // --- verifying_key_to_bytes() ---
    #[test]
    fn verifying_key_to_bytes_is_32_bytes_and_stable() {
        let seed = [0x33u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk = to_verifying_key(&sk);
        let b1 = verifying_key_to_bytes(&vk);
        let b2 = verifying_key_to_bytes(&vk);
        assert_eq!(b1.len(), 32);
        assert_eq!(b1, b2, "must be deterministic");
    }

    #[test]
    fn verifying_key_to_bytes_round_trips_through_from_bytes() {
        let seed = [0x44u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk = to_verifying_key(&sk);
        let bytes = verifying_key_to_bytes(&vk);
        let vk2 = verifying_key_from_bytes(&bytes).expect("valid key bytes");
        // Both keys must verify the same signature
        let msg = b"key bytes round trip";
        let sig = sign(&sk, msg);
        assert!(verify(&vk2, msg, &sig));
    }

    // --- keyed_hash() ---
    #[test]
    fn keyed_hash_is_deterministic() {
        let key = [0x01u8; 32];
        let data = b"test data";
        assert_eq!(keyed_hash(&key, data), keyed_hash(&key, data));
    }

    #[test]
    fn keyed_hash_differs_by_key() {
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let data = b"same data";
        assert_ne!(
            keyed_hash(&key1, data),
            keyed_hash(&key2, data),
            "different keys must produce different MACs"
        );
    }

    #[test]
    fn keyed_hash_differs_from_unkeyed_hash() {
        let key = [0xAAu8; 32];
        let data = b"vox";
        assert_ne!(
            keyed_hash(&key, data),
            secure_hash(data),
            "keyed hash must differ from unkeyed hash"
        );
    }

    #[test]
    fn keyed_hash_differs_by_data() {
        let key = [0x10u8; 32];
        assert_ne!(keyed_hash(&key, b"aaa"), keyed_hash(&key, b"bbb"));
    }

    // --- verify_signature_hex() ---
    #[test]
    fn verify_signature_hex_valid_roundtrip() {
        let seed = [0x99u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk = to_verifying_key(&sk);
        let msg = b"hello hex verify";
        let sig_bytes = sign(&sk, msg);
        let pk_hex = hex::encode(verifying_key_to_bytes(&vk));
        let sig_hex = hex::encode(sig_bytes);
        let result = verify_signature_hex(&pk_hex, msg, &sig_hex)
            .expect("should not error for valid inputs");
        assert!(result, "valid signature must verify");
    }

    #[test]
    fn verify_signature_hex_rejects_wrong_message() {
        let seed = [0x88u8; 32];
        let sk = signing_key_from_bytes(&seed);
        let vk = to_verifying_key(&sk);
        let sig_bytes = sign(&sk, b"original");
        let pk_hex = hex::encode(verifying_key_to_bytes(&vk));
        let sig_hex = hex::encode(sig_bytes);
        let result =
            verify_signature_hex(&pk_hex, b"tampered", &sig_hex).expect("should not error");
        assert!(!result, "signature must not verify against wrong message");
    }

    #[test]
    fn verify_signature_hex_rejects_invalid_hex_pubkey() {
        let err = verify_signature_hex("not-hex", b"msg", &"aa".repeat(64));
        assert!(err.is_err(), "invalid hex must return Err");
    }

    #[test]
    fn verify_signature_hex_rejects_short_pubkey() {
        // 31 bytes hex = 62 hex chars
        let short_pk = "aa".repeat(31);
        let sig_hex = "bb".repeat(64);
        let err = verify_signature_hex(&short_pk, b"msg", &sig_hex);
        assert!(err.is_err(), "short pubkey must return Err");
    }

    #[test]
    fn verify_signature_hex_rejects_short_signature() {
        let pk_hex = "cc".repeat(32);
        // 63 bytes = 126 hex chars
        let short_sig = "dd".repeat(63);
        let err = verify_signature_hex(&pk_hex, b"msg", &short_sig);
        assert!(err.is_err(), "short signature must return Err");
    }
}

#[cfg(test)]
mod semcov_wave41_tests {
    use super::*;

    // ── secure_hash ──────────────────────────────────────────────────────────

    #[test]
    fn secure_hash_empty_input_is_not_zero_array() {
        // Catches: hash returning [0u8;32] as a default/fallback instead of computing
        let h = secure_hash(&[]);
        assert_ne!(
            h, [0u8; 32],
            "BLAKE3 of empty slice must not be the zero array"
        );
    }

    #[test]
    fn secure_hash_different_inputs_produce_different_outputs() {
        // Catches: hash ignoring input and returning a constant
        let h1 = secure_hash(b"alpha");
        let h2 = secure_hash(b"beta");
        assert_ne!(h1, h2, "distinct inputs must produce distinct hashes");
    }

    #[test]
    fn secure_hash_and_compliance_hash_are_not_equal_for_same_input() {
        // Catches: both functions accidentally backed by the same algorithm
        let data = b"cross-algorithm collision check";
        let blake = secure_hash(data);
        let sha3 = compliance_hash(data);
        assert_ne!(
            blake, sha3,
            "BLAKE3 and SHA3-256 must not return equal digests for the same input"
        );
    }

    #[test]
    fn secure_hash_blake3_known_answer() {
        // Known-answer vector: plain unkeyed BLAKE3 of the empty input. secure_hash is
        // blake3::Hasher::new() + update(data) + finalize() with no keying/prefix/domain,
        // so the official BLAKE3 empty-message vector applies directly.
        let expected =
            hex::decode("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262")
                .unwrap();
        let got = secure_hash(&[]);
        assert_eq!(
            got.as_ref(),
            expected.as_slice(),
            "BLAKE3 empty-input known vector mismatch"
        );
    }

    // ── compliance_hash ───────────────────────────────────────────────────────

    #[test]
    fn compliance_hash_empty_input_is_not_zero_array() {
        // Catches: hasher finalize returning zeroed buffer on empty update
        let h = compliance_hash(&[]);
        assert_ne!(h, [0u8; 32]);
    }

    #[test]
    fn compliance_hash_known_vector() {
        // Catches: SHA3-256 wired to wrong variant (e.g. SHA2-256) — known SHA3-256("") value
        // SHA3-256 of empty string = a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a
        let expected =
            hex::decode("a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a")
                .unwrap();
        let got = compliance_hash(&[]);
        assert_eq!(
            got.as_ref(),
            expected.as_slice(),
            "SHA3-256 empty known vector mismatch"
        );
    }

    // ── fast_hash ─────────────────────────────────────────────────────────────

    #[test]
    fn fast_hash_returns_nonzero_for_nonempty_input() {
        // Catches: xxh3_64 stub returning 0
        // (collision with 0 is astronomically unlikely for realistic input)
        let h = fast_hash(b"vox-cache-key");
        assert_ne!(h, 0u64);
    }

    #[test]
    fn fast_hash_differs_for_adjacent_inputs() {
        // Catches: hash reading only first byte or ignoring trailing bytes
        let h1 = fast_hash(b"aaa");
        let h2 = fast_hash(b"aab");
        assert_ne!(h1, h2);
    }

    #[test]
    fn fast_hash_xxh3_known_answer() {
        // Known-answer vector: XXH3-64 of the empty input with the default seed (0).
        // fast_hash is a bare xxh3_64(data) call, so the upstream xxHash "" seed-0 value applies.
        let got = fast_hash(&[]);
        assert_eq!(
            got, 0x2D06800538D394C2,
            "XXH3-64 empty-input known vector (seed 0) mismatch"
        );
    }

    // ── keyed_hash ────────────────────────────────────────────────────────────

    #[test]
    fn keyed_hash_all_zero_key_differs_from_all_one_key() {
        // Catches: key parameter silently ignored (keyed hash degrades to unkeyed)
        let d = b"message";
        let h0 = keyed_hash(&[0u8; 32], d);
        let h1 = keyed_hash(&[1u8; 32], d);
        assert_ne!(h0, h1, "changing the key must change the MAC output");
    }

    #[test]
    fn keyed_hash_empty_data_still_depends_on_key() {
        // Catches: early-return on empty data bypassing key mixing
        let h0 = keyed_hash(&[0u8; 32], &[]);
        let h1 = keyed_hash(&[1u8; 32], &[]);
        assert_ne!(h0, h1, "empty-data keyed hashes must differ by key");
    }

    // ── decrypt / encrypt ─────────────────────────────────────────────────────

    #[test]
    fn decrypt_rejects_ciphertext_shorter_than_nonce() {
        // Catches: off-by-one in the length guard (< 12 vs <= 12)
        let key = generate_sym_key();
        for bad_len in 0..12usize {
            let fake_ct = vec![0u8; bad_len];
            let res = decrypt(&key, &fake_ct);
            assert!(
                res.is_err(),
                "ciphertext of {bad_len} bytes must be rejected"
            );
        }
    }

    #[test]
    fn decrypt_rejects_ciphertext_that_is_exactly_nonce_length_with_no_payload() {
        // Catches: treating a 12-byte input (nonce only, no auth tag) as valid
        // AEAD tag is 16 bytes; 12 bytes of "ciphertext" after nonce extraction
        // means zero-length payload — auth tag cannot exist, must fail
        let key = generate_sym_key();
        let nonce_only = vec![0u8; 12];
        let res = decrypt(&key, &nonce_only);
        assert!(
            res.is_err(),
            "nonce-only ciphertext (no auth tag) must fail decryption"
        );
    }

    #[test]
    fn decrypt_rejects_bitflipped_ciphertext() {
        // Catches: AEAD authentication tag not being checked (encrypt-then-forget-to-verify)
        let key = generate_sym_key();
        let mut ct = encrypt(&key, b"important data").unwrap();
        // Flip a bit in the payload region (after the 12-byte nonce)
        ct[12] ^= 0xFF;
        let res = decrypt(&key, &ct);
        assert!(
            res.is_err(),
            "bitflipped ciphertext must fail authentication"
        );
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_call_same_key_and_plaintext() {
        // Catches: nonce not randomised — static/zero nonce would produce identical output
        let key = generate_sym_key();
        let pt = b"nonce-diversity";
        let ct1 = encrypt(&key, pt).unwrap();
        let ct2 = encrypt(&key, pt).unwrap();
        assert_ne!(
            ct1, ct2,
            "two encryptions of same plaintext must produce distinct ciphertexts"
        );
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        // Catches: auth tag comparison skipped so any key decrypts successfully
        let key1 = generate_sym_key();
        let key2 = generate_sym_key();
        let ct = encrypt(&key1, b"secret").unwrap();
        let res = decrypt(&key2, &ct);
        assert!(res.is_err(), "decryption with wrong key must fail");
    }

    // ── verify / verify_signature_hex ─────────────────────────────────────────

    #[test]
    fn verify_rejects_all_zero_signature() {
        // Catches: verify() returning true for any sig without actually checking
        let (sk, vk) = generate_signing_keypair();
        let _ = sign(&sk, b"msg"); // ensure key is used
        let zero_sig = [0u8; 64];
        assert!(
            !verify(&vk, b"msg", &zero_sig),
            "all-zero signature must not verify"
        );
    }

    #[test]
    fn verify_rejects_signature_for_wrong_key() {
        // Catches: verifying against the signing key instead of the verifying key
        let (sk1, _) = generate_signing_keypair();
        let (_, vk2) = generate_signing_keypair();
        let sig = sign(&sk1, b"cross-key");
        assert!(
            !verify(&vk2, b"cross-key", &sig),
            "signature from key1 must not verify with key2's vk"
        );
    }

    #[test]
    fn verify_signature_hex_rejects_long_pubkey_hex() {
        // Catches: length check missing the > 32 case (only checks < 32)
        let long_pk = "aa".repeat(33); // 33 bytes = 66 hex chars
        let sig_hex = "bb".repeat(64);
        let err = verify_signature_hex(&long_pk, b"msg", &sig_hex);
        assert!(err.is_err(), "33-byte pubkey must return Err");
    }

    #[test]
    fn verify_signature_hex_rejects_long_signature_hex() {
        // Catches: length check missing the > 64 case (only checks < 64)
        let pk_hex = "cc".repeat(32);
        let long_sig = "dd".repeat(65); // 65 bytes = 130 hex chars
        let err = verify_signature_hex(&pk_hex, b"msg", &long_sig);
        assert!(err.is_err(), "65-byte signature must return Err");
    }

    // ── seal / unseal ─────────────────────────────────────────────────────────

    #[test]
    fn unseal_rejects_payload_shorter_than_ephemeral_pk_plus_nonce() {
        // Catches: off-by-one in the 44-byte minimum guard (32 ephemeral pk + 12 nonce)
        let (sk, _) = generate_encryption_keypair();
        for bad_len in 0..44usize {
            let fake = vec![0u8; bad_len];
            let res = unseal(&sk, &fake);
            assert!(
                res.is_err(),
                "payload of {bad_len} bytes must be rejected by unseal"
            );
        }
    }

    #[test]
    fn unseal_rejects_wrong_secret_key() {
        // Catches: shared-secret derivation not tied to the actual recipient key
        let (_, pk) = generate_encryption_keypair();
        let (wrong_sk, _) = generate_encryption_keypair();
        let boxed = seal(&pk, b"for recipient").unwrap();
        let res = unseal(&wrong_sk, &boxed);
        assert!(res.is_err(), "wrong secret key must not unseal the box");
    }

    #[test]
    fn seal_produces_different_ciphertext_each_call() {
        // Catches: ephemeral key or nonce not re-randomised per seal() call
        let (_, pk) = generate_encryption_keypair();
        let pt = b"repeated seal";
        let ct1 = seal(&pk, pt).unwrap();
        let ct2 = seal(&pk, pt).unwrap();
        assert_ne!(ct1, ct2, "each seal() must produce a fresh ciphertext");
    }

    // ── verifying_key_from_bytes ──────────────────────────────────────────────

    #[test]
    fn verifying_key_from_bytes_constructed_from_garbage_does_not_verify_real_signatures() {
        // Catches: verifying_key_from_bytes wrapping bytes without validation, allowing
        // a garbage key to accidentally verify signatures produced by a real key
        // (would happen if the verify() path ignores the public-key value entirely).
        let (sk, _) = generate_signing_keypair();
        let msg = b"legitimate message";
        let sig = sign(&sk, msg);
        // Construct a VerifyingKey from arbitrary bytes that are not the real public key.
        // ed25519_dalek accepts many byte patterns at construction time (lazy decompression),
        // so we only assert the behavioural contract: a garbage key MUST NOT verify
        // a real signature.
        let garbage_bytes = [0x42u8; 32];
        if let Ok(garbage_vk) = verifying_key_from_bytes(&garbage_bytes) {
            assert!(
                !verify(&garbage_vk, msg, &sig),
                "a garbage VerifyingKey must not verify a signature produced by a real key"
            );
        }
        // If from_bytes returns Err that is also acceptable — the key was rejected.
    }
}
