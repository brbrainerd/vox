//! Integration tests for OpFragmentEnvelope and FederationEnvelope (P6-T1).

use vox_mesh_types::op_fragment::{FederationSignature, OpFragmentEnvelope, OpFragmentKind};

/// Build a minimal envelope for testing.
fn make_envelope(kind: OpFragmentKind, payload: serde_json::Value) -> OpFragmentEnvelope {
    OpFragmentEnvelope {
        context: "https://www.w3.org/ns/activitystreams".to_string(),
        id: "urn:uuid:00000000-0000-0000-0000-000000000001".to_string(),
        kind,
        actor: "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK".to_string(),
        object: payload,
        signature: FederationSignature::placeholder(
            "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK#key-1",
        ),
    }
}

#[test]
fn round_trip_json() {
    let env = make_envelope(
        OpFragmentKind::TaskDispatched,
        serde_json::json!({ "task_id": "t-001", "priority": 128 }),
    );

    let json = serde_json::to_string(&env).expect("serialize");
    let decoded: OpFragmentEnvelope = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(decoded.id, env.id);
    assert_eq!(decoded.actor, env.actor);
    assert_eq!(decoded.kind, OpFragmentKind::TaskDispatched);
}

#[test]
fn canonical_bytes_exclude_signature() {
    let env = make_envelope(
        OpFragmentKind::KudosAward,
        serde_json::json!({ "amount": 10 }),
    );

    let bytes = env.canonical_signing_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // Signature block must be present but signature_b64 must be empty string.
    let sig_b64 = parsed["signature"]["signature_b64"]
        .as_str()
        .expect("signature_b64 is a string in canonical form");
    assert!(
        sig_b64.is_empty(),
        "canonical bytes blank out signature_b64"
    );
}

#[test]
fn canonical_bytes_are_deterministic() {
    let env = make_envelope(
        OpFragmentKind::TrustAnnouncement,
        serde_json::json!({ "epoch": 7, "digest": "deadbeef" }),
    );

    let a = env.canonical_signing_bytes();
    let b = env.canonical_signing_bytes();
    assert_eq!(a, b, "canonical bytes must be deterministic");
}

// `ed25519-dalek` is a workspace dev-dependency (see this crate's Cargo.toml),
// not a feature flag — always available in the test build.
#[test]
fn sign_and_verify_roundtrip() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    let sk = SigningKey::generate(&mut OsRng);
    let mut env = make_envelope(
        OpFragmentKind::TaskResult,
        serde_json::json!({ "task_id": "t-002", "success": true }),
    );

    let canonical = env.canonical_signing_bytes();
    let sig = sk.sign(&canonical);
    env.signature.signature_b64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sig.to_bytes());

    // Verify
    use ed25519_dalek::Verifier;
    let vk = sk.verifying_key();
    let sig_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &env.signature.signature_b64,
    )
    .unwrap();
    let sig_decoded = ed25519_dalek::Signature::from_bytes(&sig_bytes.try_into().unwrap());
    assert!(
        vk.verify(&env.canonical_signing_bytes(), &sig_decoded)
            .is_ok()
    );
}

#[test]
fn unknown_kind_deserializes() {
    let json = r#"{
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": "urn:uuid:00000000-0000-0000-0000-000000000002",
        "type": "SomeFutureKind",
        "actor": "did:key:z6Mk",
        "object": {},
        "signature": {
            "type": "Ed25519Signature2020",
            "created": "1970-01-01T00:00:00Z",
            "creator": "did:key:z6Mk#key-1",
            "signature_b64": ""
        }
    }"#;

    let env: OpFragmentEnvelope = serde_json::from_str(json).expect("unknown kind is tolerated");
    assert_eq!(env.kind, OpFragmentKind::Unknown);
}
