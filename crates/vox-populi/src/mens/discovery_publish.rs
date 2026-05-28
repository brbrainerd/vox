//! Opt-in discovery publishing for the grand volunteer network (P6-T6).
//!
//! This module implements the Scientia feedback loop that aggregates local
//! telemetry into `ProviderAtlasFinding` observations and optionally publishes
//! them to the provider's attestation Gist / `.well-known/vox-manifest.json`.
//!
//! **Default: disabled.** The discovery loop never broadcasts without explicit
//! operator action. A fresh node has `mesh.discovery_publishing.enabled = false`
//! and this module only activates when gated behind `mesh-discovery-publish`.
//!
//! # Feature gate
//!
//! This module is gated behind `#[cfg(feature = "mesh-discovery-publish")]`
//! in `crates/vox-populi/src/mens/mod.rs`.

use vox_publisher::atlas::provider_atlas::{AtlasObservation, ProviderAtlasFindingBuilder};
use vox_publisher::atlas::trust_snapshot::{TrustGraphSnapshot, TrustGraphSnapshotBuilder};

/// Configuration for the discovery-publish cron skill.
#[derive(Debug, Clone)]
pub struct DiscoveryPublishConfig {
    /// Node ID of this operator node.
    pub node_id: String,
    /// Publish target URL (Gist raw URL or `.well-known` path).
    /// When `None`, findings are logged but not uploaded.
    pub target_url: Option<String>,
    /// Minimum window size in seconds before emitting a finding.
    pub min_window_secs: u64,
    /// Whether to include a trust-graph snapshot in the publication.
    pub include_trust_snapshot: bool,
}

impl Default for DiscoveryPublishConfig {
    fn default() -> Self {
        Self {
            node_id: "unknown".to_string(),
            target_url: None,
            min_window_secs: 300, // 5 minutes
            include_trust_snapshot: false,
        }
    }
}

/// Emit a provider atlas observation from the given telemetry snapshot.
///
/// Callers pass a snapshot of current node telemetry; this function wraps it
/// in an `AtlasObservation` and feeds it to a builder. When the builder has
/// accumulated enough data (≥ `config.min_window_secs`), a `ProviderAtlasFinding`
/// is produced and optionally published.
pub fn observe_telemetry(
    builder: &mut ProviderAtlasFindingBuilder,
    node_id: &str,
    task_kinds: Vec<String>,
    tasks_completed_delta: u64,
    tasks_failed_delta: u64,
    gpu_utilisation: f64,
    vram_mb: Option<u32>,
    public_mesh_opt_in: bool,
    now_iso8601: &str,
) {
    let obs = AtlasObservation {
        node_id: node_id.to_string(),
        observed_at: now_iso8601.to_string(),
        task_kinds,
        gpu_utilisation,
        tasks_completed_delta,
        tasks_failed_delta,
        vram_mb,
        public_mesh_opt_in,
    };
    builder.observe(&obs);
}

/// Run one cycle of the trust-graph snapshot collector.
///
/// Accepts a list of `(node_id, trust_tier, manifest_url, success, fail)`
/// tuples and builds a `TrustGraphSnapshot` for publication.
pub fn run_trust_snapshot_cycle(
    own_node_id: &str,
    snapshot_at: &str,
    peers: impl IntoIterator<
        Item = (
            String, // node_id
            u8,     // trust_tier
            String, // manifest_url
            u64,    // success_count
            u64,    // fail_count
        ),
    >,
) -> TrustGraphSnapshot {
    let mut builder = TrustGraphSnapshotBuilder::new(own_node_id, snapshot_at);
    for (node_id, trust_tier, manifest_url, success_count, fail_count) in peers {
        builder.add_peer(vox_publisher::atlas::trust_snapshot::PeerEntry {
            node_id,
            trust_tier,
            manifest_url,
            last_verified_at: snapshot_at.to_string(),
            success_count,
            fail_count,
            notes: None,
        });
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_publisher::atlas::provider_atlas::ProviderAtlasFindingBuilder;

    #[test]
    fn observe_builds_finding() {
        let mut builder = ProviderAtlasFindingBuilder::new("node-x", "2026-05-10T00:00:00Z");
        observe_telemetry(
            &mut builder,
            "node-x",
            vec!["text_infer".to_string()],
            10,
            1,
            0.75,
            Some(16384),
            true,
            "2026-05-10T00:05:00Z",
        );
        let finding = builder.build();
        assert_eq!(finding.node_id, "node-x");
        assert_eq!(finding.tasks_completed, 10);
        assert_eq!(finding.tasks_failed, 1);
        assert!(finding.consistently_opted_in);
    }

    #[test]
    fn trust_snapshot_cycle() {
        let snapshot = run_trust_snapshot_cycle(
            "own-node",
            "2026-05-10T00:00:00Z",
            vec![
                (
                    "peer-a".to_string(),
                    3,
                    "https://gist/peer-a".to_string(),
                    100,
                    2,
                ),
                (
                    "peer-b".to_string(),
                    1,
                    "https://gist/peer-b".to_string(),
                    5,
                    0,
                ),
            ],
        );
        assert_eq!(snapshot.node_id, "own-node");
        assert_eq!(snapshot.peers.len(), 2);
        assert_eq!(snapshot.peers_at_or_above_tier(3), 1);
    }
}
