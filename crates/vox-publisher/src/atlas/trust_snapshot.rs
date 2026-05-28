//! Trust-graph snapshot self-publication (P6-T8).
//!
//! A `TrustGraphSnapshot` is a node's view of its local trust graph at a
//! point in time — which peers it trusts, at what tier, and when the
//! assessment was last updated. Nodes periodically publish their snapshot
//! to their attestation Gist so that federation partners can discover
//! their trust relationships without a central server.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Peer entry
// ---------------------------------------------------------------------------

/// A single peer's trust record inside a `TrustGraphSnapshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEntry {
    /// The peer's node ID.
    pub node_id: String,
    /// Numeric trust tier (0 = Unknown, 1 = Attested, 2 = Reputable, 3 = Vetted, 4 = Internal).
    pub trust_tier: u8,
    /// URL of the peer's attestation manifest (Gist raw URL or .well-known path).
    pub manifest_url: String,
    /// ISO-8601 timestamp of the last verification of this peer.
    pub last_verified_at: String,
    /// Cumulative successful task count from this peer.
    pub success_count: u64,
    /// Cumulative failed task count from this peer.
    pub fail_count: u64,
    /// Optional free-form notes (human-readable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// A point-in-time view of a node's local trust graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustGraphSnapshot {
    /// Schema version (`"1"`).
    pub version: String,
    /// Node ID of the snapshot owner.
    pub node_id: String,
    /// ISO-8601 timestamp when the snapshot was produced.
    pub snapshot_at: String,
    /// All known peers, keyed by `node_id`.
    pub peers: HashMap<String, PeerEntry>,
    /// Optional base64-encoded Ed25519 signature over the canonical snapshot bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_b64: Option<String>,
}

impl TrustGraphSnapshot {
    /// Return the canonical bytes to sign (all fields, `signature_b64` set to `null`).
    pub fn canonical_signing_bytes(&self) -> serde_json::Result<Vec<u8>> {
        let mut v = serde_json::to_value(self)?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert("signature_b64".to_string(), serde_json::Value::Null);
        }
        let sorted = sort_json_value(v);
        serde_json::to_vec(&sorted)
    }

    /// Return the number of peers at or above the given trust tier.
    pub fn peers_at_or_above_tier(&self, min_tier: u8) -> usize {
        self.peers
            .values()
            .filter(|p| p.trust_tier >= min_tier)
            .count()
    }
}

fn sort_json_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut sorted = serde_json::Map::with_capacity(map.len());
            for k in keys {
                sorted.insert(k.clone(), sort_json_value(map[&k].clone()));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_json_value).collect())
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builds a `TrustGraphSnapshot` incrementally.
#[derive(Debug, Default)]
pub struct TrustGraphSnapshotBuilder {
    pub node_id: String,
    pub snapshot_at: String,
    pub peers: HashMap<String, PeerEntry>,
}

impl TrustGraphSnapshotBuilder {
    /// Create a new builder for the given node.
    pub fn new(node_id: impl Into<String>, snapshot_at: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            snapshot_at: snapshot_at.into(),
            ..Default::default()
        }
    }

    /// Add or update a peer entry.
    pub fn add_peer(&mut self, entry: PeerEntry) {
        self.peers.insert(entry.node_id.clone(), entry);
    }

    /// Build the snapshot.
    pub fn build(self) -> TrustGraphSnapshot {
        TrustGraphSnapshot {
            version: "1".to_string(),
            node_id: self.node_id,
            snapshot_at: self.snapshot_at,
            peers: self.peers,
            signature_b64: None,
        }
    }
}
