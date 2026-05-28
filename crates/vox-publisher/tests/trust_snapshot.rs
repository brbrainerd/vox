//! Integration tests for TrustGraphSnapshot (P6-T8).

use vox_publisher::atlas::trust_snapshot::{
    PeerEntry, TrustGraphSnapshot, TrustGraphSnapshotBuilder,
};

fn make_peer(node_id: &str, tier: u8, successes: u64, fails: u64) -> PeerEntry {
    PeerEntry {
        node_id: node_id.to_string(),
        trust_tier: tier,
        manifest_url: format!("https://gist.github.com/raw/{}", node_id),
        last_verified_at: "2026-05-10T00:00:00Z".to_string(),
        success_count: successes,
        fail_count: fails,
        notes: None,
    }
}

#[test]
fn empty_snapshot_round_trip() {
    let snap = TrustGraphSnapshotBuilder::new("node-own", "2026-05-10T00:00:00Z").build();
    let json = serde_json::to_string(&snap).expect("serialize");
    let decoded: TrustGraphSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.node_id, "node-own");
    assert!(decoded.peers.is_empty());
}

#[test]
fn snapshot_with_peers_round_trip() {
    let mut builder = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z");
    builder.add_peer(make_peer("peer-a", 3, 100, 2));
    builder.add_peer(make_peer("peer-b", 1, 5, 0));
    let snap = builder.build();

    let json = serde_json::to_string(&snap).expect("serialize");
    let decoded: TrustGraphSnapshot = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(decoded.peers.len(), 2);
    assert_eq!(decoded.peers["peer-a"].trust_tier, 3);
    assert_eq!(decoded.peers["peer-b"].success_count, 5);
}

#[test]
fn peers_at_or_above_tier() {
    let mut builder = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z");
    builder.add_peer(make_peer("p1", 0, 0, 0)); // Unknown
    builder.add_peer(make_peer("p2", 1, 10, 0)); // Attested
    builder.add_peer(make_peer("p3", 3, 50, 1)); // Vetted
    builder.add_peer(make_peer("p4", 4, 200, 0)); // Internal
    let snap = builder.build();

    assert_eq!(snap.peers_at_or_above_tier(0), 4);
    assert_eq!(snap.peers_at_or_above_tier(1), 3);
    assert_eq!(snap.peers_at_or_above_tier(3), 2);
    assert_eq!(snap.peers_at_or_above_tier(4), 1);
    assert_eq!(snap.peers_at_or_above_tier(5), 0);
}

#[test]
fn canonical_signing_bytes_are_deterministic() {
    let mut builder = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z");
    builder.add_peer(make_peer("peer-a", 3, 100, 2));
    let snap = builder.build();

    let a = snap.canonical_signing_bytes().unwrap();
    let b = snap.canonical_signing_bytes().unwrap();
    assert_eq!(a, b);
}

#[test]
fn canonical_signing_bytes_null_signature() {
    let snap = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z").build();
    let bytes = snap.canonical_signing_bytes().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // signature_b64 should be null in canonical form.
    assert!(v["signature_b64"].is_null());
}

#[test]
fn snapshot_version_is_one() {
    let snap = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z").build();
    assert_eq!(snap.version, "1");
}

#[test]
fn add_peer_upserts() {
    let mut builder = TrustGraphSnapshotBuilder::new("own", "2026-05-10T00:00:00Z");
    builder.add_peer(make_peer("peer-x", 1, 5, 0));
    builder.add_peer(make_peer("peer-x", 2, 10, 0)); // update
    let snap = builder.build();
    assert_eq!(snap.peers.len(), 1); // still one entry
    assert_eq!(snap.peers["peer-x"].trust_tier, 2); // updated tier
}
