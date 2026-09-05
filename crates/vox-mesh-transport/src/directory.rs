//! Peer directory — what replaces the HTTP `federation_directory()` endpoint.
//!
//! The old directory was a list a control plane *asserted*. This one is a list
//! the network *demonstrates*: every entry was reachable and answered a `Probe`
//! just now. A peer that is trusted but switched off is simply absent, which is
//! what the model selector needs — it must not route work to a dark machine.
//!
//! Consumers are `vox-orchestrator`'s `registry.rs`, `catalog.rs` and
//! `task_submit.rs` (plan Task 3.2). That wiring needs a
//! `vox-orchestrator -> vox-mesh-transport` crate edge, which is
//! user-authorised-only, so it is proposed rather than taken here.

use std::sync::Arc;
use std::time::Duration;

use iroh::{Endpoint, EndpointAddr, EndpointId};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::protocol::{self, Hello, JobRequest, JobResponse};
use crate::trust::MeshTrust;

/// How long one peer gets to answer before it counts as absent.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// One reachable peer, as demonstrated by a `Probe` round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerEntry {
    pub endpoint_id: EndpointId,
    pub label: Option<String>,
    pub host_triple: String,
    pub vox: String,
    pub task_kinds: Vec<vox_mesh_types::TaskKind>,
}

/// Probe every trusted peer; return those that answered.
///
/// Peers are probed concurrently, so the call is bounded by [`PROBE_TIMEOUT`]
/// rather than `PROBE_TIMEOUT * peers` — a directory that got slower the more
/// machines you own would be the wrong shape.
///
/// Never returns `Err`: one unreachable peer must not hide the others, so a
/// failure is an omission from the list rather than an error for all of it.
pub async fn directory(ep: &Endpoint, trust: &Arc<MeshTrust>) -> Vec<PeerEntry> {
    let mut out: Vec<PeerEntry> = fan_out(ep, trust, JobRequest::Probe)
        .await
        .into_iter()
        .filter_map(|(endpoint_id, label, resp)| match resp {
            JobResponse::Probed {
                host_triple,
                vox,
                task_kinds,
            } => Some(PeerEntry {
                endpoint_id,
                label,
                host_triple,
                vox,
                task_kinds,
            }),
            // Answered something other than a probe result. Same conclusion as
            // silence: do not route work here.
            _ => None,
        })
        .collect();
    // Stable order: callers build model ids from this and a shuffling list would
    // churn them every refresh.
    out.sort_by_key(|e| e.endpoint_id.to_string());
    out
}

/// Queue depth summed over the peers that answered.
///
/// The totals are the sum of what peers *claimed*; `peers_answered` is there so
/// a caller can tell "the mesh says zero" from "no mesh answered", which is the
/// difference between reporting a depth and falling back to a local source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeshQueueTotals {
    pub pending_count: u64,
    pub pending_by_kind: Vec<(vox_mesh_types::TaskKind, u64)>,
    pub pending_by_priority: Vec<(u8, u64)>,
    pub peers_answered: usize,
}

/// Ask every trusted peer how deep its queue is.
///
/// Same shape as [`directory`] and for the same reason: concurrent, bounded by
/// [`PROBE_TIMEOUT`], and never `Err` — one dark machine must not erase the
/// depth the others reported.
pub async fn queue_stats(ep: &Endpoint, trust: &Arc<MeshTrust>) -> MeshQueueTotals {
    let answers = fan_out(ep, trust, JobRequest::QueueStats)
        .await
        .into_iter()
        .filter_map(|(_, _, resp)| match resp {
            JobResponse::QueueStats(s) => Some(s),
            _ => None,
        });
    fold_stats(answers)
}

/// Sum per-peer stats into one total.
///
/// Split out from [`queue_stats`] because it is the only part that is not a
/// round-trip: everything else the live tests cover, this is arithmetic.
fn fold_stats(answers: impl IntoIterator<Item = protocol::QueueStats>) -> MeshQueueTotals {
    let mut totals = MeshQueueTotals::default();
    for s in answers {
        totals.peers_answered += 1;
        totals.pending_count = totals.pending_count.saturating_add(s.pending_count);
        for (kind, n) in s.pending_by_kind {
            match totals.pending_by_kind.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, acc)) => *acc = acc.saturating_add(n),
                None => totals.pending_by_kind.push((kind, n)),
            }
        }
        for (prio, n) in s.pending_by_priority {
            match totals
                .pending_by_priority
                .iter_mut()
                .find(|(p, _)| *p == prio)
            {
                Some((_, acc)) => *acc = acc.saturating_add(n),
                None => totals.pending_by_priority.push((prio, n)),
            }
        }
    }
    totals.pending_by_kind.sort_by_key(|(k, _)| k.as_str());
    totals.pending_by_priority.sort_by_key(|(p, _)| *p);
    totals
}

/// Send one request to every trusted, dialable peer; yield what came back.
///
/// Shared by [`directory`] and [`queue_stats`] so the address handling and the
/// timeout live in one place — two copies would drift, and the mDNS finding
/// below is exactly the kind of thing that only gets fixed in one copy.
async fn fan_out(
    ep: &Endpoint,
    trust: &Arc<MeshTrust>,
    request: JobRequest,
) -> Vec<(EndpointId, Option<String>, JobResponse)> {
    let mut set = JoinSet::new();

    for row in trust.rows() {
        let Ok(id) = row.endpoint_id.parse::<EndpointId>() else {
            continue;
        };
        // No addresses means unreachable, not "discover it": mDNS does not
        // announce (spike findings Q4), so an id alone is not dialable.
        let addrs: Vec<std::net::SocketAddr> =
            row.addrs.iter().filter_map(|a| a.parse().ok()).collect();
        if addrs.is_empty() {
            tracing::debug!(peer = %id, "trusted peer has no stored address; skipping");
            continue;
        }

        let ep = ep.clone();
        let label = row.label.clone();
        let request = request.clone();
        set.spawn(async move {
            let mut addr = EndpointAddr::new(id);
            for a in addrs {
                addr = addr.with_ip_addr(a);
            }
            match timeout(PROBE_TIMEOUT, ask_one(&ep, addr, request)).await {
                Ok(Ok(resp)) => Some((id, label, resp)),
                // Unreachable or refused. Both mean "this peer told us nothing".
                _ => None,
            }
        });
    }

    let mut out = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(Some(answer)) = joined {
            out.push(answer);
        }
    }
    out
}

/// One request/response round-trip against a single peer.
async fn ask_one(
    ep: &Endpoint,
    addr: EndpointAddr,
    request: JobRequest,
) -> anyhow::Result<JobResponse> {
    let conn = ep.connect(addr, protocol::ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    protocol::write_frame(&mut send, &Hello::current()).await?;
    protocol::write_frame(&mut send, &request).await?;
    send.finish()?;
    let resp = protocol::read_frame(&mut recv, 64 * 1024).await?;
    conn.close(0u32.into(), b"asked");
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_mesh_types::TaskKind;

    #[test]
    fn totals_add_across_peers_and_keep_the_breakdowns() {
        let totals = fold_stats([
            protocol::QueueStats {
                pending_count: 3,
                pending_by_kind: vec![(TaskKind::VoxScript, 3)],
                pending_by_priority: vec![(5, 3)],
            },
            protocol::QueueStats {
                pending_count: 4,
                pending_by_kind: vec![(TaskKind::VoxScript, 1), (TaskKind::Embed, 3)],
                pending_by_priority: vec![(9, 4)],
            },
        ]);
        assert_eq!(totals.pending_count, 7);
        assert_eq!(totals.peers_answered, 2);
        // Sorted by kind name, so the MCP tool's JSON does not reorder itself
        // between refreshes.
        assert_eq!(
            totals.pending_by_kind,
            vec![(TaskKind::Embed, 3), (TaskKind::VoxScript, 4)]
        );
        assert_eq!(totals.pending_by_priority, vec![(5, 3), (9, 4)]);
    }

    #[test]
    fn no_answers_is_zero_peers_not_a_zero_depth_claim() {
        // The caller distinguishes these two, so the type must as well.
        let totals = fold_stats([]);
        assert_eq!(totals.peers_answered, 0);
        assert_eq!(totals.pending_count, 0);
    }

    #[test]
    fn a_lying_peer_cannot_overflow_the_total() {
        let liar = protocol::QueueStats {
            pending_count: u64::MAX,
            pending_by_kind: vec![(TaskKind::Embed, u64::MAX)],
            pending_by_priority: Vec::new(),
        };
        let totals = fold_stats([liar.clone(), liar]);
        assert_eq!(totals.pending_count, u64::MAX);
        assert_eq!(totals.pending_by_kind, vec![(TaskKind::Embed, u64::MAX)]);
    }
}
