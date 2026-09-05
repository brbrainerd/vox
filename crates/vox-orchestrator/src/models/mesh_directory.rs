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

fn vox_dir() -> std::path::PathBuf {
    vox_config::paths::dot_vox_user_dir()
}

/// Peers that are trusted *and* answered a probe. Empty on any failure —
/// the selector must degrade to "no mesh candidates", never to an error.
pub async fn trusted_peers() -> Vec<PeerEntry> {
    let ep = ENDPOINT
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
        .await;

    let Some(ep) = ep.as_ref() else {
        return Vec::new();
    };
    let trust = Arc::new(MeshTrust::at(&vox_dir().join("mesh_trust.json")));
    vox_mesh_transport::directory(ep, &trust).await
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
    fn state_is_read_from_the_user_dot_vox_dir() {
        // Must match where `vox mesh join` writes, or pairing and routing
        // disagree about which peers exist.
        assert!(vox_dir().ends_with(".vox"));
    }
}
