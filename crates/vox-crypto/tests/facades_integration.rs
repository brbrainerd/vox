//! Cross-facade crypto behaviors (no I/O, no network).

use vox_crypto::{
    SymKey, compliance_hash, decrypt, encrypt, fast_hash, generate_encryption_keypair,
    generate_signing_keypair, seal, secure_hash, sign, unseal, verify,
};

#[test]
fn hash_facades_are_deterministic_for_fixed_input() {
    let data = b"vox-crypto integration";
    assert_eq!(secure_hash(data), secure_hash(data));
    assert_eq!(compliance_hash(data), compliance_hash(data));
    assert_eq!(fast_hash(data), fast_hash(data));
}

#[test]
fn chacha_encrypt_roundtrip_with_fixed_key() {
    let key = SymKey([7u8; 32]);
    let pt = b"classified memo";
    let ct = encrypt(&key, pt).expect("encrypt");
    let round = decrypt(&key, &ct).expect("decrypt");
    assert_eq!(round.as_slice(), pt.as_slice());
}

#[test]
fn ed25519_sign_verify_roundtrip() {
    let (sk, vk) = generate_signing_keypair();
    let msg = b"payload";
    let sig = sign(&sk, msg);
    assert!(verify(&vk, msg, &sig));
    assert!(!verify(&vk, b"tampered", &sig));
}

#[test]
fn x25519_seal_unseal_roundtrip_no_network() {
    let (secret, public) = generate_encryption_keypair();
    let plaintext = b"sealed for recipient";
    let boxed = seal(&public, plaintext).expect("seal");
    let opened = unseal(&secret, &boxed).expect("unseal");
    assert_eq!(opened.as_slice(), plaintext.as_slice());
}
