//! P2-T4 acceptance tests for CAS bundle seeding helpers.

use std::sync::Arc;

use vox_orchestrator::a2a::dispatch::bundle_fetch::{
    INLINE_BUNDLE_BYTE_LIMIT, decode_inline, resolve_local, ship_decision,
};
use vox_package::bundle::{Bundle, BundleRef, BundleStore};

fn dummy_ref() -> BundleRef {
    BundleRef {
        fn_hash: [0xabu8; 64],
    }
}

fn make_bundle(size: usize) -> Bundle {
    Bundle {
        fn_hash: [0xabu8; 64],
        deps: vec![],
        bytes: Arc::new(vec![0xffu8; size]),
        manifest: serde_json::json!({"kind": "workflow"}),
    }
}

#[test]
fn envelope_round_trips_with_bundle_ref() {
    use vox_orchestrator::a2a::RemoteTaskEnvelope;

    let r = dummy_ref();
    let env = RemoteTaskEnvelope {
        idempotency_key: "k1".into(),
        task_id: 1,
        repository_id: "repo".into(),
        capability_requirements_json: "{}".into(),
        payload: "p".into(),
        privacy_class: None,
        populi_scope_id: None,
        submitted_unix_ms: None,
        exec_lease_id: None,
        campaign_id: None,
        artifact_refs_json: None,
        session_id: None,
        thread_id: None,
        context_envelope_json: None,
        harness_spec_json: None,
        parent_task_id: None,
        caller_agent_id: None,
        trace_id: None,
        span_depth: None,
        bundle_ref: Some(r),
        bundle_inline_b64: Some("aGVsbG8=".into()),
    };
    let json = serde_json::to_string(&env).unwrap();
    let back: RemoteTaskEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(back.bundle_ref.unwrap().fn_hash, [0xabu8; 64]);
    assert_eq!(back.bundle_inline_b64.as_deref(), Some("aGVsbG8="));
}

#[test]
fn legacy_envelope_without_bundle_ref_still_deserializes() {
    use vox_orchestrator::a2a::RemoteTaskEnvelope;

    let json = r#"{
        "idempotency_key": "k2",
        "task_id": 2,
        "repository_id": "repo",
        "capability_requirements_json": "{}",
        "payload": "p"
    }"#;
    let env: RemoteTaskEnvelope = serde_json::from_str(json).unwrap();
    assert!(env.bundle_ref.is_none());
    assert!(env.bundle_inline_b64.is_none());
}

#[test]
fn small_bundle_inlines() {
    let bundle = make_bundle(1024);
    let (r, inline) = ship_decision(&bundle);
    assert_eq!(r.fn_hash, bundle.fn_hash);
    assert!(inline.is_some(), "small bundle should be inlined");
}

#[test]
fn large_bundle_drops_to_request_round_trip() {
    let bundle = make_bundle(INLINE_BUNDLE_BYTE_LIMIT + 1);
    let (_, inline) = ship_decision(&bundle);
    assert!(inline.is_none(), "oversized bundle must not be inlined");
}

#[test]
fn decode_inline_recovers_original_bytes() {
    let original = vec![1u8, 2, 3, 4, 5];
    let bundle = Bundle {
        fn_hash: [0xabu8; 64],
        deps: vec![],
        bytes: Arc::new(original.clone()),
        manifest: serde_json::json!({"kind": "activity"}),
    };
    let (r, b64) = ship_decision(&bundle);
    let b64 = b64.expect("should inline");
    let recovered = decode_inline(&r, &b64, vec![], serde_json::json!({"kind": "activity"}))
        .expect("decode should succeed");
    assert_eq!(recovered.bytes.as_ref(), &original);
    assert_eq!(recovered.fn_hash, bundle.fn_hash);
}

#[test]
fn resolve_local_returns_none_on_miss() {
    let dir = tempfile::tempdir().unwrap();
    let store = BundleStore::open(dir.path().to_path_buf()).unwrap();
    let r = dummy_ref();
    let result = resolve_local(&store, &r).unwrap();
    assert!(result.is_none());
}
