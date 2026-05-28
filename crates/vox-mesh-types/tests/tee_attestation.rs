//! Integration tests for TEE attestation envelope (P6-T5).

use vox_mesh_types::{
    Attestation,
    task::TaskResult,
    tee_attestation::{StubTeeVerifier, TeeQuote, TeeQuoteKind, TeeVerifier, TeeVerifyError},
};

fn make_tee_quote(kind: TeeQuoteKind) -> TeeQuote {
    TeeQuote {
        kind,
        quote_b64: "AAAAAAAAAA==".to_string(),
        measurement_blake3_hex: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
            .to_string(),
        platform_timestamp: Some("2026-05-10T00:00:00Z".to_string()),
        nonce_hex: Some("cafecafe".to_string()),
    }
}

fn make_attestation_with_tee(tee_quote: Option<TeeQuote>) -> Attestation {
    Attestation {
        task_id: "task-001".to_string(),
        input_hash_blake3_hex: "aabbcc".to_string(),
        output_hash_blake3_hex: "ddeeff".to_string(),
        gpu_seconds: 1.5,
        trace_blake3_hex: None,
        ephemeral_pubkey_hex: "0".repeat(64),
        signature_b64: "AAAA".to_string(),
        signed_at_unix_ms: 0,
        tee_quote,
        replay_proof_blake3_hex: None,
        kudos_signature_b64: None,
    }
}

#[test]
fn attestation_with_tee_quote_round_trips() {
    let attestation = make_attestation_with_tee(Some(make_tee_quote(TeeQuoteKind::Stub)));
    let json = serde_json::to_string(&attestation).expect("serialize");
    let decoded: Attestation = serde_json::from_str(&json).expect("deserialize");
    let q = decoded.tee_quote.expect("tee_quote present");
    assert_eq!(q.kind, TeeQuoteKind::Stub);
    assert_eq!(q.measurement_blake3_hex.len(), 64);
}

#[test]
fn attestation_without_tee_quote_round_trips() {
    let attestation = make_attestation_with_tee(None);
    let json = serde_json::to_string(&attestation).expect("serialize");
    // tee_quote should be absent from JSON (skip_serializing_if)
    assert!(!json.contains("tee_quote"));
    let decoded: Attestation = serde_json::from_str(&json).expect("deserialize");
    assert!(decoded.tee_quote.is_none());
}

#[test]
fn task_result_with_attestation_and_tee() {
    let attestation = make_attestation_with_tee(Some(make_tee_quote(TeeQuoteKind::IntelTdx)));
    let result = TaskResult {
        task_id: "task-002".to_string(),
        node_id: "node-001".to_string(),
        success: true,
        output_b64: "output".to_string(),
        duration_ms: 100,
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        attestation: Some(attestation),
    };
    let json = serde_json::to_string(&result).expect("serialize");
    let decoded: TaskResult = serde_json::from_str(&json).expect("deserialize");
    let att = decoded.attestation.expect("attestation present");
    let q = att.tee_quote.expect("tee_quote present");
    assert_eq!(q.kind, TeeQuoteKind::IntelTdx);
}

#[test]
fn stub_verifier_returns_unsupported() {
    let verifier = StubTeeVerifier;
    let quote = make_tee_quote(TeeQuoteKind::AmdSevSnp);
    let result = verifier.verify(&quote);
    assert!(result.is_err());
    match result.unwrap_err() {
        TeeVerifyError::Unsupported(kind) => assert_eq!(kind, TeeQuoteKind::AmdSevSnp),
        other => panic!("expected Unsupported, got {:?}", other),
    }
}

#[test]
fn stub_verifier_rejects_all_kinds() {
    let verifier = StubTeeVerifier;
    for kind in [
        TeeQuoteKind::IntelTdx,
        TeeQuoteKind::AmdSevSnp,
        TeeQuoteKind::AwsNitro,
        TeeQuoteKind::FirecrackerMeasurement,
        TeeQuoteKind::Stub,
    ] {
        let q = make_tee_quote(kind);
        assert!(verifier.verify(&q).is_err());
    }
}

#[test]
fn tee_quote_kind_round_trip_json() {
    for kind in [
        TeeQuoteKind::IntelTdx,
        TeeQuoteKind::AmdSevSnp,
        TeeQuoteKind::AwsNitro,
        TeeQuoteKind::FirecrackerMeasurement,
        TeeQuoteKind::Stub,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        let decoded: TeeQuoteKind = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, kind);
    }
}

#[test]
fn replay_proof_and_kudos_signature_optional() {
    let mut att = make_attestation_with_tee(None);
    att.replay_proof_blake3_hex = Some("replay-hash".to_string());
    att.kudos_signature_b64 = Some("kudos-sig".to_string());

    let json = serde_json::to_string(&att).unwrap();
    let decoded: Attestation = serde_json::from_str(&json).unwrap();
    assert_eq!(
        decoded.replay_proof_blake3_hex.as_deref(),
        Some("replay-hash")
    );
    assert_eq!(decoded.kudos_signature_b64.as_deref(), Some("kudos-sig"));
}
