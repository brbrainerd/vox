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
        set.spawn(async move {
            let mut addr = EndpointAddr::new(id);
            for a in addrs {
                addr = addr.with_ip_addr(a);
            }
            match timeout(PROBE_TIMEOUT, probe_one(&ep, addr)).await {
                Ok(Ok(JobResponse::Probed {
                    host_triple,
                    vox,
                    task_kinds,
                })) => Some(PeerEntry {
                    endpoint_id: id,
                    label,
                    host_triple,
                    vox,
                    task_kinds,
                }),
                // Unreachable, refused, or answered something other than a probe
                // result. All three mean "do not route work here".
                _ => None,
            }
        });
    }

    let mut out = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(Some(entry)) = joined {
            out.push(entry);
        }
    }
    // Stable order: callers build model ids from this and a shuffling list would
    // churn them every refresh.
    out.sort_by_key(|e| e.endpoint_id.to_string());
    out
}

/// One `Probe` round-trip against a single peer.
async fn probe_one(ep: &Endpoint, addr: EndpointAddr) -> anyhow::Result<JobResponse> {
    let conn = ep.connect(addr, protocol::ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    protocol::write_frame(&mut send, &Hello::current()).await?;
    protocol::write_frame(&mut send, &JobRequest::Probe).await?;
    send.finish()?;
    let resp = protocol::read_frame(&mut recv, 64 * 1024).await?;
    conn.close(0u32.into(), b"probed");
    Ok(resp)
}
