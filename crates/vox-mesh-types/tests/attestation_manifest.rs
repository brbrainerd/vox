//! Integration tests for PublicAttestationManifest (P6-T2).

use vox_mesh_types::attestation_manifest::{
    AttestationCache, PublicAttestationManifest, SupportedTask,
};

fn make_manifest(node_id: &str) -> PublicAttestationManifest {
    PublicAttestationManifest {
        version: "1".to_string(),
        node_id: node_id.to_string(),
        pubkey_hex: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        published_at: "2026-05-10T00:00:00Z".to_string(),
        supported_tasks: vec![
            SupportedTask {
                kind: "text_infer".to_string(),
                supported: true,
                min_vram_mb: Some(8192),
                max_concurrent: Some(2),
            },
            SupportedTask {
                kind: "image_gen".to_string(),
                supported: false,
                min_vram_mb: None,
                max_concurrent: None,
            },
        ],
        metadata: Default::default(),
        signature_b64: String::new(),
    }
}

#[test]
fn round_trip_json() {
    let m = make_manifest("node-001");
    let json = serde_json::to_string(&m).expect("serialize");
    let decoded: PublicAttestationManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.node_id, "node-001");
    assert_eq!(decoded.supported_tasks.len(), 2);
}

#[test]
fn canonical_bytes_blank_signature() {
    let m = make_manifest("node-002");
    let bytes = m.canonical_signing_bytes().expect("canonical bytes");
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["signature_b64"].as_str(), Some(""));
}

#[test]
fn canonical_bytes_deterministic() {
    let m = make_manifest("node-003");
    let a = m.canonical_signing_bytes().unwrap();
    let b = m.canonical_signing_bytes().unwrap();
    assert_eq!(a, b);
}

#[test]
fn cache_insert_and_get() {
    let mut cache = AttestationCache::with_ttl(3600);
    let m = make_manifest("node-004");

    cache.insert(m.clone(), 1000);
    let found = cache.get("node-004", 1001);
    assert!(found.is_some());
    assert_eq!(found.unwrap().node_id, "node-004");
}

#[test]
fn cache_miss_expired() {
    let mut cache = AttestationCache::with_ttl(60);
    let m = make_manifest("node-005");

    cache.insert(m, 0); // inserted at t=0
    let found = cache.get("node-005", 61); // now t=61, ttl=60 → expired
    assert!(found.is_none());
}

#[test]
fn cache_evict_stale() {
    let mut cache = AttestationCache::with_ttl(60);
    cache.insert(make_manifest("node-006"), 0); // inserted at t=0
    cache.insert(make_manifest("node-007"), 1000); // inserted at t=1000

    // At t=50: node-006 age=50 (live), node-007 age=0 (future insert counted as live via saturating_sub)
    // Only check that both are accessible before eviction.
    assert_eq!(cache.len(50), 2); // both entries present

    cache.evict_stale(65); // node-006 is now stale (age=65 > ttl=60); node-007 keeps (age=0)

    // After eviction, only node-007 remains.
    assert_eq!(cache.len(1000), 1); // only node-007 remains
}

#[test]
fn cache_is_empty_when_all_expired() {
    let mut cache = AttestationCache::with_ttl(10);
    cache.insert(make_manifest("node-008"), 0);
    assert!(!cache.is_empty(5));
    assert!(cache.is_empty(11));
}
