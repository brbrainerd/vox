//! Ed25519 envelope round-trip and trust-gate tests (P5-T1a, P5-T1b, P5-T1c).

// Rust 2024: `std::env::set_var` / `remove_var` are `unsafe`. Each `unsafe`
// block below documents serialization assumptions (these tests do not run
// concurrently with anything else that touches the same env vars).
#![allow(unsafe_code)]

use base64::Engine as _;
use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_populi::transport::envelope::{EnvelopeVerifyError, SignedA2AEnvelope};

// ── P5-T1a: self-signed round-trip ────────────────────────────────────────────

#[test]
fn envelope_round_trip_verifies() {
    let (sk, vk) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let env = SignedA2AEnvelope::sign("ack", &payload, &sk, &vk);
    assert_eq!(env.message_type, "ack");
    assert_eq!(env.sender_pubkey_hex.len(), 64);
    assert!(env.verify_self_signed().is_ok());
}

#[test]
fn envelope_with_swapped_signature_is_rejected() {
    let (sk_a, vk_a) = generate_signing_keypair();
    let (sk_b, _vk_b) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk_a, &vk_a);
    let other = SignedA2AEnvelope::sign("ack", &payload, &sk_b, &vk_a);
    env.signature_b64 = other.signature_b64;
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}

#[test]
fn envelope_with_swapped_payload_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let payload = br#"{"hello":"world"}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk, &vk);
    env.payload_b64 = base64::engine::general_purpose::STANDARD.encode(b"{\"hello\":\"evil\"}");
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}

#[test]
fn pubkey_in_envelope_must_match_signer() {
    let (sk_a, vk_a) = generate_signing_keypair();
    let (_sk_b, vk_b) = generate_signing_keypair();
    let payload = br#"{}"#.to_vec();
    let mut env = SignedA2AEnvelope::sign("ack", &payload, &sk_a, &vk_a);
    env.sender_pubkey_hex = hex::encode(verifying_key_to_bytes(&vk_b));
    let err = env.verify_self_signed().unwrap_err();
    assert!(matches!(err, EnvelopeVerifyError::SignatureMismatch));
}

// ── P5-T1b: trust-ledger gate ─────────────────────────────────────────────────

#[test]
fn verify_against_trust_admits_known_pubkey() {
    use vox_identity::TrustedNodeRegistry;
    use vox_populi::transport::auth_ed25519::{VerifyTrustError, verify_against_trust};

    let (sk, vk) = generate_signing_keypair();
    let pubkey_hex = hex::encode(verifying_key_to_bytes(&vk));
    let mut reg = TrustedNodeRegistry::default();
    reg.upsert("node-A", &pubkey_hex);

    let env = SignedA2AEnvelope::sign("ack", b"{}", &sk, &vk);
    let ctx = verify_against_trust(&env, &reg, 300_000).expect("admit");
    assert_eq!(ctx.node_id, "node-A");
    let _: VerifyTrustError; // type must be reachable
}

#[test]
fn verify_against_trust_rejects_unknown_pubkey() {
    use vox_identity::TrustedNodeRegistry;
    use vox_populi::transport::auth_ed25519::{VerifyTrustError, verify_against_trust};

    let (sk, vk) = generate_signing_keypair();
    let reg = TrustedNodeRegistry::default();
    let env = SignedA2AEnvelope::sign("ack", b"{}", &sk, &vk);
    let err = verify_against_trust(&env, &reg, 300_000).unwrap_err();
    assert!(matches!(err, VerifyTrustError::UnknownPubkey));
}

// ── P5-T1c: AuthScheme default ────────────────────────────────────────────────

#[test]
fn auth_scheme_default_is_ed25519_envelope() {
    use vox_populi::transport::AuthScheme;
    let prior = std::env::var("VOX_MESH_AUTH_SCHEME").ok();
    unsafe {
        std::env::remove_var("VOX_MESH_AUTH_SCHEME");
    }
    let scheme = AuthScheme::from_env();
    assert_eq!(scheme, AuthScheme::Ed25519Envelope);
    if let Some(v) = prior {
        unsafe {
            std::env::set_var("VOX_MESH_AUTH_SCHEME", v);
        }
    }
}
