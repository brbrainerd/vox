//! A2A relay over the mesh mailbox (plan Task 3.1).
//!
//! The seam that decides whether a result goes over iroh or over the HTTP
//! control plane. HTTP is **not** deleted here — that is Phase 6, and it only
//! happens after the ported capability is proven on two machines.
//!
//! The mesh path is taken when, and only when, this node has a mesh identity
//! and there is exactly one trusted peer with a stored address. That "exactly
//! one" is a real ceiling and not a placeholder for laziness: A2A addresses a
//! *receiver agent id*, the mailbox addresses an `EndpointId`, and nothing in
//! the envelope maps between them today. With one paired machine the mapping is
//! unambiguous; with two it would be a guess, and guessing which machine gets a
//! task result is worse than falling back to the transport that still works.
//! The mapping arrives in Phase 6, when the mailbox is also the *inbox* and the
//! sending peer's `EndpointId` is therefore known.

use std::path::PathBuf;

use vox_mesh_transport::{MeshTrust, Outbox};
use vox_mesh_types::A2ADeliverRequest;

// One endpoint per process, shared with the peer directory. Two endpoints built
// from the same `mesh.key` would be one `EndpointId` reachable at two ports.
use crate::models::mesh_directory::{endpoint, vox_dir};

/// `~/.vox/mesh_outbox/` — queued mail, beside `mesh.key` and the trust store.
fn outbox_path() -> PathBuf {
    vox_dir().join("mesh_outbox")
}

/// The single trusted, addressable peer, or `None` when there is no such peer
/// or more than one.
fn sole_addressable_peer(trust: &MeshTrust) -> Option<iroh::EndpointId> {
    let mut reachable = trust
        .rows()
        .into_iter()
        .filter(|r| !r.addrs.is_empty())
        .filter_map(|r| r.endpoint_id.parse::<iroh::EndpointId>().ok());
    let first = reachable.next()?;
    if reachable.next().is_some() {
        tracing::debug!(
            target: "vox.orchestrator.a2a",
            "more than one trusted mesh peer: A2A cannot yet say which one a result belongs to, \
             so it goes over HTTP"
        );
        return None;
    }
    Some(first)
}

/// Queue `req` for mesh delivery. Returns `true` when the mesh has taken
/// ownership of the message and the caller must **not** also send it over HTTP.
///
/// Queueing is durable and happens before any dial, so `true` here means "this
/// message is on disk and will be delivered when the peer is reachable", not
/// "it arrived". That is the whole difference between this and the HTTP call it
/// replaces: the peer is routinely switched off, and `relay_a2a` simply failed.
pub async fn try_relay(req: &A2ADeliverRequest) -> bool {
    let trust = MeshTrust::at(&vox_dir().join("mesh_trust.json"));
    let Some(peer) = sole_addressable_peer(&trust) else {
        return false;
    };

    let outbox = Outbox::at(&outbox_path());
    if let Err(e) = outbox.queue(&peer, req) {
        // A full or unwritable outbox must fall back rather than drop the
        // message: HTTP still works today.
        tracing::warn!(target: "vox.orchestrator.a2a", error = %e, "mesh outbox refused the message; falling back to HTTP");
        return false;
    }

    // Best-effort immediate delivery. A failure here is not a failure of the
    // relay — the message is queued, and the next flush will carry it.
    if let Some(ep) = endpoint().await {
        let delivered = outbox.flush(ep, &trust).await;
        tracing::debug!(target: "vox.orchestrator.a2a", delivered, "mesh outbox flushed");
    }
    true
}

/// Flush whatever is queued. Called on a timer by the remote worker tick so a
/// peer that comes back finds its mail waiting rather than needing a new
/// message to trigger the retry.
pub async fn flush_pending() -> usize {
    let outbox = Outbox::at(&outbox_path());
    if outbox.depth() == 0 {
        return 0;
    }
    let Some(ep) = endpoint().await else {
        return 0;
    };
    let trust = MeshTrust::at(&vox_dir().join("mesh_trust.json"));
    outbox.flush(ep, &trust).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    fn trust_at(dir: &std::path::Path) -> MeshTrust {
        MeshTrust::at(&dir.join("mesh_trust.json"))
    }

    #[test]
    fn no_trusted_peer_means_no_mesh_relay() {
        let d = tempfile::tempdir().unwrap();
        assert!(sole_addressable_peer(&trust_at(d.path())).is_none());
    }

    #[test]
    fn a_peer_with_no_address_is_not_a_relay_target() {
        // An EndpointId alone is not dialable (mDNS does not announce), so
        // routing to one would queue mail that can never be delivered.
        let d = tempfile::tempdir().unwrap();
        let trust = trust_at(d.path());
        trust.trust(&SecretKey::generate().public(), None).unwrap();
        assert!(sole_addressable_peer(&trust).is_none());
    }

    #[test]
    fn one_addressable_peer_is_the_relay_target() {
        let d = tempfile::tempdir().unwrap();
        let trust = trust_at(d.path());
        let id = SecretKey::generate().public();
        trust
            .trust_with_addrs(&id, None, &["127.0.0.1:9".parse().unwrap()])
            .unwrap();
        assert_eq!(sole_addressable_peer(&trust), Some(id));
    }

    #[test]
    fn two_peers_fall_back_rather_than_guessing_which_one() {
        // Sending a task result to the wrong machine is worse than sending it
        // over the transport that still works.
        let d = tempfile::tempdir().unwrap();
        let trust = trust_at(d.path());
        for _ in 0..2 {
            trust
                .trust_with_addrs(
                    &SecretKey::generate().public(),
                    None,
                    &["127.0.0.1:9".parse().unwrap()],
                )
                .unwrap();
        }
        assert!(sole_addressable_peer(&trust).is_none());
    }

    #[test]
    fn mesh_state_lives_beside_the_identity_it_belongs_to() {
        // Must match where `vox mesh join` writes, or pairing and relaying
        // disagree about which peers exist.
        assert!(outbox_path().starts_with(vox_dir()));
        assert!(outbox_path().ends_with("mesh_outbox"));
    }
}
