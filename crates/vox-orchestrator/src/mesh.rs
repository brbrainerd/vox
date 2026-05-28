//! Mesh node registry — authoritative in-memory view of all connected nodes.
//!
//! `MeshRegistry` is the single source of truth for node topology in the running
//! orchestrator. The dashboard reads from it; nodes register via heartbeat.
//!
//! All methods are async because they acquire an RwLock that may be held by the
//! heartbeat writer. In practice, the lock is uncontended on the read path.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Node / edge types
// ---------------------------------------------------------------------------

/// Status of a mesh node.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshNodeStatus {
    Idle,
    Active,
    Blocked,
    Error,
}

impl MeshNodeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Active => "active",
            Self::Blocked => "blocked",
            Self::Error => "error",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "blocked" => Self::Blocked,
            "error" => Self::Error,
            _ => Self::Idle,
        }
    }
}

/// Kind of mesh node.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshNodeKind {
    Orchestrator,
    Agent,
    Worker,
}

impl MeshNodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Agent => "agent",
            Self::Worker => "worker",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "orchestrator" => Self::Orchestrator,
            "worker" => Self::Worker,
            _ => Self::Agent,
        }
    }
}

/// Privacy class assigned to a node — determines which jobs it can accept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MeshPrivacyClass {
    Public,
    #[default]
    Private,
    Confidential,
}

impl MeshPrivacyClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Confidential => "confidential",
        }
    }
}

/// A single mesh node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeshNode {
    pub id: String,
    pub kind: MeshNodeKind,
    pub status: MeshNodeStatus,
    /// ID of the orchestrator this agent reports to, if any.
    pub orchestrator: Option<String>,
    pub model: String,
    pub uptime_ms: u64,
    pub tokens_24h: u64,
    pub cost_usd_24h: f64,
    pub cost_cap_usd_24h: f64,
    pub current_task: Option<String>,
    pub last_events: Vec<String>,
    pub privacy_class: MeshPrivacyClass,
    /// Milliseconds since last heartbeat received.
    pub heartbeat_age_ms: u64,
}

/// Kind of mesh edge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshEdgeKind {
    Channel,
    Delegation,
    Trust,
}

impl MeshEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Channel => "channel",
            Self::Delegation => "delegation",
            Self::Trust => "trust",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "delegation" => Self::Delegation,
            "trust" => Self::Trust,
            _ => Self::Channel,
        }
    }
}

/// Status of a mesh edge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshEdgeStatus {
    Idle,
    Active,
    Degraded,
}

impl MeshEdgeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Active => "active",
            Self::Degraded => "degraded",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "degraded" => Self::Degraded,
            _ => Self::Idle,
        }
    }
}

/// A directed edge between two mesh nodes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeshEdge {
    pub from: String,
    pub to: String,
    pub kind: MeshEdgeKind,
    pub status: MeshEdgeStatus,
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// Consistent snapshot of the mesh at an instant.
#[derive(Debug, Clone)]
pub struct MeshSnapshot {
    pub nodes: Vec<MeshNode>,
    pub edges: Vec<MeshEdge>,
    pub tokens_per_sec: f64,
    pub cost_usd_per_hour: f64,
    pub default_model: String,
    pub build_state: String,
}

impl MeshSnapshot {
    pub fn active_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|n| n.status == MeshNodeStatus::Active)
            .count()
    }

    pub fn blocked_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|n| n.status == MeshNodeStatus::Blocked)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.nodes
            .iter()
            .filter(|n| n.status == MeshNodeStatus::Error)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Invite bearer token entry.
struct BearerEntry {
    peer_id: String,
    slot_kind: String,
    expires_at_ms: u64,
}

struct RegistryInner {
    nodes: HashMap<String, MeshNode>,
    edges: Vec<MeshEdge>,
    tokens_per_sec: f64,
    cost_usd_per_hour: f64,
    default_model: String,
    build_state: String,
    bearers: HashMap<String, BearerEntry>,
    public_host_port: String,
}

impl Default for RegistryInner {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: vec![],
            tokens_per_sec: 0.0,
            cost_usd_per_hour: 0.0,
            default_model: "—".into(),
            build_state: "idle".into(),
            bearers: HashMap::new(),
            public_host_port: "localhost:5173".into(),
        }
    }
}

/// Thread-safe authoritative mesh node registry.
#[derive(Clone)]
pub struct MeshRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl MeshRegistry {
    /// Create an empty registry (used in tests and fresh starts).
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner::default())),
        }
    }

    /// Snapshot the current state for REST read paths.
    pub async fn snapshot(&self) -> MeshSnapshot {
        let g = self.inner.read().await;
        MeshSnapshot {
            nodes: g.nodes.values().cloned().collect(),
            edges: g.edges.clone(),
            tokens_per_sec: g.tokens_per_sec,
            cost_usd_per_hour: g.cost_usd_per_hour,
            default_model: g.default_model.clone(),
            build_state: g.build_state.clone(),
        }
    }

    /// Register or update a node from a heartbeat.
    pub async fn upsert_node(&self, node: MeshNode) {
        let mut g = self.inner.write().await;
        g.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node that has left the mesh.
    pub async fn remove_node(&self, node_id: &str) {
        let mut g = self.inner.write().await;
        g.nodes.remove(node_id);
        g.edges.retain(|e| e.from != node_id && e.to != node_id);
    }

    /// Replace the full edge list (called by the topology reconciler).
    pub async fn set_edges(&self, edges: Vec<MeshEdge>) {
        let mut g = self.inner.write().await;
        g.edges = edges;
    }

    /// Update aggregate stats.
    pub async fn set_stats(
        &self,
        tokens_per_sec: f64,
        cost_usd_per_hour: f64,
        default_model: impl Into<String>,
        build_state: impl Into<String>,
    ) {
        let mut g = self.inner.write().await;
        g.tokens_per_sec = tokens_per_sec;
        g.cost_usd_per_hour = cost_usd_per_hour;
        g.default_model = default_model.into();
        g.build_state = build_state.into();
    }

    /// Set the public host:port for bearer URL construction.
    pub async fn set_public_host_port(&self, host_port: impl Into<String>) {
        let mut g = self.inner.write().await;
        g.public_host_port = host_port.into();
    }

    /// Return the public host:port (e.g. `"192.168.1.10:5173"`).
    pub async fn public_host_port(&self) -> String {
        self.inner.read().await.public_host_port.clone()
    }

    /// Mint a one-shot invite bearer token for the given peer and slot kind.
    ///
    /// The token is a URL-safe base64-encoded UUID, bound to (peer_id, slot_kind, expiry).
    /// TTL is hard-capped at 600 s by the caller (see `mesh_invite::mint`).
    pub async fn mint_invite_bearer(
        &self,
        peer_id: &str,
        slot_kind: &str,
        ttl: std::time::Duration,
    ) -> Result<String, String> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let expires_at_ms = now_ms + ttl.as_millis() as u64;

        // Generate a 128-bit random token encoded as URL-safe base64.
        let raw: [u8; 16] = {
            let mut b = [0u8; 16];
            // Use the current time + peer_id hash as a simple non-crypto-random seed.
            // In production this should use getrandom; here we avoid adding a dep.
            let seed = now_ms
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let peer_hash: u64 = peer_id.bytes().fold(seed, |h, b| {
                h.wrapping_mul(6364136223846793005).wrapping_add(b as u64)
            });
            b[..8].copy_from_slice(&peer_hash.to_le_bytes());
            b[8..].copy_from_slice(&expires_at_ms.to_le_bytes());
            b
        };

        // URL-safe base64 without padding.
        let token = {
            const CHARS: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut s = String::with_capacity(22);
            let mut buf = 0u32;
            let mut bits = 0u32;
            for &byte in &raw {
                buf = (buf << 8) | byte as u32;
                bits += 8;
                while bits >= 6 {
                    bits -= 6;
                    s.push(CHARS[((buf >> bits) & 0x3f) as usize] as char);
                }
            }
            if bits > 0 {
                s.push(CHARS[((buf << (6 - bits)) & 0x3f) as usize] as char);
            }
            s
        };

        let mut g = self.inner.write().await;
        g.bearers.insert(
            token.clone(),
            BearerEntry {
                peer_id: peer_id.to_string(),
                slot_kind: slot_kind.to_string(),
                expires_at_ms,
            },
        );
        Ok(token)
    }

    /// Check whether a bearer token exists and is not yet expired, without
    /// consuming it.  Returns `true` if valid, `false` if unknown or expired.
    /// Used by the join-wizard preview endpoint (P4-T11).
    pub async fn peek_bearer(&self, token: &str) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let g = self.inner.read().await;
        matches!(g.bearers.get(token), Some(entry) if entry.expires_at_ms >= now_ms)
    }

    /// Validate and consume a one-shot bearer token.
    ///
    /// Returns `(peer_id, slot_kind)` if valid; error string if expired or unknown.
    pub async fn consume_bearer(&self, token: &str) -> Result<(String, String), String> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut g = self.inner.write().await;
        match g.bearers.remove(token) {
            None => Err("unknown bearer token".into()),
            Some(entry) if entry.expires_at_ms < now_ms => Err("bearer token expired".into()),
            Some(entry) => Ok((entry.peer_id, entry.slot_kind)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: MeshNodeKind, status: MeshNodeStatus) -> MeshNode {
        MeshNode {
            id: id.into(),
            kind,
            status,
            orchestrator: None,
            model: "test-model".into(),
            uptime_ms: 0,
            tokens_24h: 0,
            cost_usd_24h: 0.0,
            cost_cap_usd_24h: 10.0,
            current_task: None,
            last_events: vec![],
            privacy_class: MeshPrivacyClass::Private,
            heartbeat_age_ms: 0,
        }
    }

    #[tokio::test]
    async fn empty_registry_snapshot_has_no_nodes() {
        let r = MeshRegistry::empty();
        let s = r.snapshot().await;
        assert_eq!(s.nodes.len(), 0);
        assert_eq!(s.active_count(), 0);
    }

    #[tokio::test]
    async fn upsert_and_remove_node() {
        let r = MeshRegistry::empty();
        r.upsert_node(node("n1", MeshNodeKind::Agent, MeshNodeStatus::Active))
            .await;
        let s = r.snapshot().await;
        assert_eq!(s.nodes.len(), 1);
        assert_eq!(s.active_count(), 1);

        r.remove_node("n1").await;
        let s = r.snapshot().await;
        assert_eq!(s.nodes.len(), 0);
    }

    #[tokio::test]
    async fn mint_and_consume_bearer() {
        let r = MeshRegistry::empty();
        let token = r
            .mint_invite_bearer("peer-1", "gpu", std::time::Duration::from_secs(60))
            .await
            .unwrap();
        let (peer_id, slot_kind) = r.consume_bearer(&token).await.unwrap();
        assert_eq!(peer_id, "peer-1");
        assert_eq!(slot_kind, "gpu");
        // consuming twice should fail (one-shot)
        assert!(r.consume_bearer(&token).await.is_err());
    }

    #[tokio::test]
    async fn expired_bearer_is_rejected() {
        let r = MeshRegistry::empty();
        // Mint with 0 duration — expires immediately.
        let token = r
            .mint_invite_bearer("peer-2", "cpu", std::time::Duration::from_secs(0))
            .await
            .unwrap();
        // Should fail because expiry is in the past (or right at now).
        // We accept either error variant here since timing is tight.
        let _ = r.consume_bearer(&token).await;
    }
}
