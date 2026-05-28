//! Bounded gossip with Bloom-filter anti-entropy for convergence op-log (P3-T3).
//!
//! Anti-entropy strategy: Demers et al. "Epidemic algorithms for replicated database
//! maintenance," PODC 1987. Periodic sweep emits a Bloom-encoded Summary to all
//! known peers; peers reply with op-fragments not covered by that filter.

use serde::{Deserialize, Serialize};

/// Stable A2A wire type for op-fragment gossip (forward-compatible with older peers).
pub const OP_FRAGMENT_SYNC_TYPE: &str = "vox.orchestrator.OpFragmentSync.v1";

// Bloom filter: m = 2^20 bits (128 KiB), k = 7 hash functions.
// FPR at 100k items ≈ (1 − e^(−7·100_000 / 1_048_576))^7 ≈ 0.8%.
const M_BITS: usize = 1 << 20;
const K: usize = 7;

/// Bloom filter over op-ids. Uses blake3 to derive k independent bit positions.
pub struct OpIdBloom {
    bits: Vec<u64>, // M_BITS / 64 words
    /// Lowest op-id inserted (u64::MAX when empty).
    pub floor: u64,
    /// Highest op-id inserted (0 when empty).
    pub ceiling: u64,
}

impl Default for OpIdBloom {
    fn default() -> Self {
        Self::new()
    }
}

impl OpIdBloom {
    pub fn new() -> Self {
        Self {
            bits: vec![0u64; M_BITS / 64],
            floor: u64::MAX,
            ceiling: 0,
        }
    }

    pub fn insert(&mut self, op_id: u64) {
        for i in 0..K {
            self.set_bit(self.idx(op_id, i));
        }
        if op_id < self.floor {
            self.floor = op_id;
        }
        if op_id > self.ceiling {
            self.ceiling = op_id;
        }
    }

    pub fn might_contain(&self, op_id: u64) -> bool {
        (0..K).all(|i| self.get_bit(self.idx(op_id, i)))
    }

    fn idx(&self, op_id: u64, i: usize) -> usize {
        let mut h = blake3::Hasher::new();
        h.update(&op_id.to_be_bytes());
        h.update(&(i as u64).to_be_bytes());
        let out = h.finalize();
        let bytes: [u8; 8] = out.as_bytes()[0..8].try_into().unwrap();
        (u64::from_be_bytes(bytes) as usize) % M_BITS
    }

    fn set_bit(&mut self, i: usize) {
        self.bits[i / 64] |= 1u64 << (i % 64);
    }

    fn get_bit(&self, i: usize) -> bool {
        self.bits[i / 64] & (1u64 << (i % 64)) != 0
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bits.len() * 8);
        for w in &self.bits {
            out.extend_from_slice(&w.to_be_bytes());
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != M_BITS / 8 {
            return None;
        }
        let mut bits = vec![0u64; M_BITS / 64];
        for (i, chunk) in b.chunks_exact(8).enumerate() {
            bits[i] = u64::from_be_bytes(chunk.try_into().ok()?);
        }
        Some(Self {
            bits,
            floor: 0,
            ceiling: 0,
        })
    }
}

/// Wire envelope for op-fragment gossip.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpFragmentSync {
    /// "I have these ops; tell me what I'm missing." Sweep request.
    Summary {
        daemon_id: [u8; 16],
        set_id: [u8; 16],
        /// Base64-encoded `OpIdBloom::to_bytes()` (~128 KiB).
        bloom_b64: String,
        floor_op_id: u64,
        ceiling_op_id: u64,
    },
    /// Reply with op-fragments not covered by the requester's bloom.
    /// Bounded to ~1 MiB; `more_after` is the cursor for continuation.
    Reply {
        daemon_id: [u8; 16],
        fragments: Vec<OpFragmentBlob>,
        more_after: Option<u64>,
    },
    /// Continuation when a Reply hit the byte limit.
    Continue { daemon_id: [u8; 16], cursor: u64 },
}

/// A single convergence op-log entry transported over the gossip wire.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpFragmentBlob {
    pub op_id: u64,
    pub parent_op_ids: Vec<u64>,
    pub kind_json: String,
    pub payload: Vec<u8>,
    /// Ed25519 signature bytes (64 bytes).
    pub signature: Vec<u8>,
    /// BLAKE3 of the signing verifying-key bytes (32 bytes).
    pub signing_key_id: [u8; 32],
    pub daemon_id: [u8; 16],
    pub produced_at: u64,
}

/// A known peer daemon in the gossip set.
#[derive(Clone, Debug)]
pub struct PeerEntry {
    pub agent_id: crate::types::AgentId,
    pub daemon_id: [u8; 16],
}

/// Shared registry of peer daemons for gossip sweeps.
pub struct PeerRegistry {
    local_daemon_id: [u8; 16],
    set_id: [u8; 16],
    peers: std::sync::RwLock<Vec<PeerEntry>>,
}

impl PeerRegistry {
    pub fn new(local_daemon_id: [u8; 16], set_id: [u8; 16]) -> Self {
        Self {
            local_daemon_id,
            set_id,
            peers: std::sync::RwLock::new(Vec::new()),
        }
    }

    pub fn local_daemon_id(&self) -> [u8; 16] {
        self.local_daemon_id
    }

    pub fn set_id(&self) -> [u8; 16] {
        self.set_id
    }

    pub fn add_peer(&self, peer: PeerEntry) {
        self.peers.write().unwrap().push(peer);
    }

    pub fn snapshot(&self) -> Vec<PeerEntry> {
        self.peers.read().unwrap().clone()
    }
}

/// Errors from the gossip sweep loop.
#[derive(Debug, thiserror::Error)]
pub enum GossipError {
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("relay: {0}")]
    Relay(String),
}

#[cfg(feature = "populi-transport")]
pub use populi_impl::run_sweep_loop;

#[cfg(feature = "populi-transport")]
mod populi_impl {
    use std::sync::Arc;
    use std::time::Duration;

    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as B64;
    use tokio::sync::RwLock;
    use tokio::time::MissedTickBehavior;

    use vox_orchestrator_queue::oplog::OpLog;

    use crate::types::AgentId;

    use super::{GossipError, OP_FRAGMENT_SYNC_TYPE, OpFragmentSync, OpIdBloom, PeerRegistry};

    /// Periodically emit a Bloom-summary to all known peers (Demers et al. anti-entropy).
    ///
    /// Default `period` is 30 seconds. Call from a detached `tokio::spawn`.
    pub async fn run_sweep_loop(
        inbox_agent_id: AgentId,
        peers: Arc<PeerRegistry>,
        log: Arc<RwLock<OpLog>>,
        client: vox_populi::http_client::PopuliHttpClient,
        period: Duration,
    ) {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match sweep_once(&inbox_agent_id, &peers, &log, &client).await {
                Ok(()) => {
                    tracing::debug!(agent = inbox_agent_id.0, "op_fragment_sync sweep ok");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "op_fragment_sync sweep failed");
                }
            }
        }
    }

    async fn sweep_once(
        inbox_agent_id: &AgentId,
        peers: &PeerRegistry,
        log: &Arc<RwLock<OpLog>>,
        client: &vox_populi::http_client::PopuliHttpClient,
    ) -> Result<(), GossipError> {
        let bloom = build_bloom(log).await;
        let (floor, ceiling) = (bloom.floor, bloom.ceiling);
        let summary = OpFragmentSync::Summary {
            daemon_id: peers.local_daemon_id(),
            set_id: peers.set_id(),
            bloom_b64: B64.encode(bloom.to_bytes()),
            floor_op_id: if floor == u64::MAX { 0 } else { floor },
            ceiling_op_id: ceiling,
        };
        let payload = serde_json::to_string(&summary)?;
        let payload_len = payload.len() as u64;
        for peer in peers.snapshot() {
            client
                .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
                    sender_agent_id: inbox_agent_id.0.to_string(),
                    receiver_agent_id: peer.agent_id.0.to_string(),
                    message_type: OP_FRAGMENT_SYNC_TYPE.to_string(),
                    payload: payload.clone(),
                    idempotency_key: None,
                    privacy_class: None,
                    payload_blake3_hex: None,
                    worker_ed25519_sig_b64: None,
                    jwe_payload: None,
                    task_kind: None,
                    model_id: None,
                    traceparent: None,
                    priority: 64,
                })
                .await
                .map_err(|e| GossipError::Relay(e.to_string()))?;
            tracing::debug!(
                bytes = payload_len,
                peer_agent = peer.agent_id.0,
                "gossip summary sent"
            );
        }
        Ok(())
    }

    async fn build_bloom(log: &Arc<RwLock<OpLog>>) -> OpIdBloom {
        let guard = log.read().await;
        let mut bloom = OpIdBloom::new();
        for entry in guard.history() {
            bloom.insert(entry.id.0);
        }
        bloom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as B64;

    #[test]
    fn bloom_insert_and_contains() {
        let mut b = OpIdBloom::new();
        b.insert(42);
        b.insert(1_000_000);
        assert!(b.might_contain(42));
        assert!(b.might_contain(1_000_000));
        assert!(!b.might_contain(99));
    }

    #[test]
    fn bloom_empty_does_not_contain() {
        let b = OpIdBloom::new();
        for id in [0u64, 1, 100, 9999, u64::MAX - 1] {
            assert!(!b.might_contain(id), "empty bloom should not contain {id}");
        }
    }

    #[test]
    fn bloom_round_trips_bytes() {
        let mut b = OpIdBloom::new();
        for id in [1u64, 2, 3, 100, 9999] {
            b.insert(id);
        }
        let bytes = b.to_bytes();
        assert_eq!(
            bytes.len(),
            M_BITS / 8,
            "bloom byte length must be M_BITS/8"
        );
        let b2 = OpIdBloom::from_bytes(&bytes).expect("round-trip");
        for id in [1u64, 2, 3, 100, 9999] {
            assert!(
                b2.might_contain(id),
                "round-tripped bloom must contain {id}"
            );
        }
    }

    #[test]
    fn bloom_from_bytes_rejects_wrong_length() {
        assert!(OpIdBloom::from_bytes(&[0u8; 64]).is_none());
        assert!(OpIdBloom::from_bytes(&[]).is_none());
    }

    #[test]
    fn bloom_floor_ceiling_tracking() {
        let mut b = OpIdBloom::new();
        b.insert(50);
        b.insert(10);
        b.insert(200);
        assert_eq!(b.floor, 10);
        assert_eq!(b.ceiling, 200);
    }

    #[test]
    fn bloom_single_entry_floor_equals_ceiling() {
        let mut b = OpIdBloom::new();
        b.insert(77);
        assert_eq!(b.floor, 77);
        assert_eq!(b.ceiling, 77);
    }

    #[test]
    fn op_fragment_sync_summary_round_trips() {
        let bloom = OpIdBloom::new();
        let msg = OpFragmentSync::Summary {
            daemon_id: [1u8; 16],
            set_id: [2u8; 16],
            bloom_b64: B64.encode(bloom.to_bytes()),
            floor_op_id: 0,
            ceiling_op_id: 999,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"kind\":\"summary\""));
        let back: OpFragmentSync = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, OpFragmentSync::Summary { .. }));
    }

    #[test]
    fn op_fragment_sync_reply_round_trips() {
        let msg = OpFragmentSync::Reply {
            daemon_id: [3u8; 16],
            fragments: vec![],
            more_after: Some(42),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: OpFragmentSync = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            OpFragmentSync::Reply {
                more_after: Some(42),
                ..
            }
        ));
    }

    #[test]
    fn op_fragment_sync_continue_round_trips() {
        let msg = OpFragmentSync::Continue {
            daemon_id: [7u8; 16],
            cursor: 1234,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: OpFragmentSync = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            OpFragmentSync::Continue { cursor: 1234, .. }
        ));
    }

    #[test]
    fn op_fragment_blob_round_trips() {
        let blob = OpFragmentBlob {
            op_id: 5,
            parent_op_ids: vec![1, 2, 3],
            kind_json: r#"{"type":"noop"}"#.into(),
            payload: vec![0xde, 0xad, 0xbe, 0xef],
            signature: vec![0u8; 64],
            signing_key_id: [9u8; 32],
            daemon_id: [4u8; 16],
            produced_at: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&blob).unwrap();
        let back: OpFragmentBlob = serde_json::from_str(&json).unwrap();
        assert_eq!(back.op_id, 5);
        assert_eq!(back.parent_op_ids, [1, 2, 3]);
        assert_eq!(back.signing_key_id, [9u8; 32]);
    }

    #[test]
    fn peer_registry_snapshot_and_add() {
        use crate::types::AgentId;
        let reg = PeerRegistry::new([0u8; 16], [1u8; 16]);
        assert_eq!(reg.snapshot().len(), 0);
        reg.add_peer(super::PeerEntry {
            agent_id: AgentId(42),
            daemon_id: [2u8; 16],
        });
        assert_eq!(reg.snapshot().len(), 1);
        assert_eq!(reg.snapshot()[0].agent_id.0, 42);
    }
}
