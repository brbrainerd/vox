//! Trusted-peer enumeration for the model selector (plan Task 3.2).
//!
//! Replaces `PopuliHttpClient::federation_directory()` as the source of
//! `ProviderType::PopuliMesh` candidates. The HTTP directory returned whatever
//! a control plane had been told; this returns only peers that answered a
//! `Probe` just now, so a machine that is off cannot be routed work.

use std::sync::Arc;

use tokio::sync::OnceCell;
use vox_mesh_transport::{MeshTrust, PeerEntry};

/// Bound once per process. Binding an iroh endpoint opens sockets and starts
/// background tasks, which is far too heavy for a catalog refresh.
static ENDPOINT: OnceCell<Option<iroh::Endpoint>> = OnceCell::const_new();

pub(crate) fn vox_dir() -> std::path::PathBuf {
    vox_config::paths::dot_vox_user_dir()
}

/// The process's one mesh endpoint, or `None` when this node has no usable mesh
/// identity.
///
/// Shared rather than per-caller: two endpoints built from the same `mesh.key`
/// would be one `EndpointId` reachable at two ports, and every peer's stored
/// `addrs` would then be half-right about where to find us.
pub async fn endpoint() -> Option<&'static iroh::Endpoint> {
    ENDPOINT
        .get_or_init(|| async {
            let sk = match vox_mesh_transport::identity::load_or_create(&vox_dir().join("mesh.key"))
            {
                Ok(sk) => sk,
                Err(e) => {
                    tracing::warn!(target: "vox.orchestrator.models", error = %e, "mesh identity unavailable; no mesh candidates");
                    return None;
                }
            };
            match vox_mesh_transport::endpoint::bind(sk).await {
                Ok(ep) => Some(ep),
                Err(e) => {
                    tracing::warn!(target: "vox.orchestrator.models", error = %e, "mesh endpoint bind failed; no mesh candidates");
                    None
                }
            }
        })
        .await
        .as_ref()
}

/// Peers that are trusted *and* answered a probe. Empty on any failure —
/// the selector must degrade to "no mesh candidates", never to an error.
pub async fn trusted_peers() -> Vec<PeerEntry> {
    let Some(ep) = endpoint().await else {
        return Vec::new();
    };
    vox_mesh_transport::directory(ep, &trust_store()).await
}

/// Queue depth as the trusted peers report it (plan Task 3.3).
///
/// Replaces `PopuliHttpClient::queue_stats()` for the MCP tool. Every number is
/// **peer-asserted** — a peer reports its own depth, and nothing here verifies
/// it. `peers_answered` is what lets a caller tell "the mesh says zero" from
/// "no mesh answered"; weighting the claims by trust is Phase 4.
pub async fn queue_stats() -> vox_mesh_transport::MeshQueueTotals {
    let Some(ep) = endpoint().await else {
        return Default::default();
    };
    vox_mesh_transport::queue_stats(ep, &trust_store()).await
}

fn trust_store() -> Arc<MeshTrust> {
    Arc::new(MeshTrust::at(&vox_dir().join("mesh_trust.json")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_trust_store_yields_no_candidates_rather_than_an_error() {
        // The selector runs on every routing decision. A missing or unreadable
        // mesh must cost it nothing.
        let trust = Arc::new(MeshTrust::at(std::path::Path::new(
            "/nonexistent/dir/mesh_trust.json",
        )));
        assert!(trust.rows().is_empty());
    }

    #[test]
    fn the_no_endpoint_answer_reads_as_no_peers_not_as_an_empty_queue() {
        // What `queue_stats()` returns when the endpoint will not bind. The MCP
        // tool keys its local-registry fallback on `peers_answered == 0`, so a
        // default that claimed a peer had answered would silently report a mesh
        // depth of zero over a real local queue. The wire behaviour itself is
        // covered live in `vox-mesh-transport/tests/security.rs`; this pins the
        // degrade contract, which is all this module adds.
        let totals = vox_mesh_transport::MeshQueueTotals::default();
        assert_eq!(totals.peers_answered, 0);
        assert_eq!(totals.pending_count, 0);
    }

    #[test]
    fn state_is_read_from_the_user_dot_vox_dir() {
        // Must match where `vox mesh join` writes, or pairing and routing
        // disagree about which peers exist.
        assert!(vox_dir().ends_with(".vox"));
    }
}
