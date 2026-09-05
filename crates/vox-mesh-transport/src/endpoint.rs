//! Endpoint construction and the bounded, trust-gated accept loop.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use iroh::endpoint::{Connection, NetReportConfig, presets};
use iroh::{Endpoint, EndpointId, SecretKey};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::protocol::{self, JobLimits, JobRequest, JobResponse};
use crate::trust::MeshTrust;

/// A TLS handshake is ~100 µs of *unauthenticated* work and iroh applies no
/// connection cap of its own, so this is the only thing standing between a
/// spoofed source and the CPU.
const MAX_INFLIGHT_HANDSHAKES: usize = 64;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Once this many permits are in use, make new sources prove reachability
/// before we spend a handshake on them.
const RETRY_THRESHOLD: usize = 32;

/// Close code for a peer that is not on the allowlist.
pub const REFUSED_UNTRUSTED: u32 = 4001;
/// Close code for a payload that exceeds [`JobLimits::max_payload_bytes`].
pub const REFUSED_TOO_LARGE: u32 = 4002;

/// A job that passed the trust gate and the payload check.
pub struct ReceivedJob {
    pub peer: EndpointId,
    pub request: JobRequest,
    /// Decided by *this* node, never by the sender.
    pub limits: JobLimits,
}

/// What actually runs received work. Kept behind a trait so the accept loop can
/// be tested without a sandbox, and so the sandbox can change without touching
/// the transport.
pub trait JobExecutor: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        job: ReceivedJob,
    ) -> Pin<Box<dyn Future<Output = Result<JobResponse>> + Send + 'a>>;
}

/// The default executor: answers `Probe`, and **refuses `Run`**.
///
/// Deliberately not a stub that runs things. The sandbox tiers in
/// [`Isolation`](crate::protocol::Isolation) are declared but not yet
/// implemented, and an executor that ran `Run` "for now" would be an
/// unsandboxed remote-execution path reachable by any paired peer — the exact
/// hole the HTTP plane had. Refusing is the safe default until a real sandbox
/// backs it.
#[derive(Debug, Clone, Copy)]
pub struct ProbeOnlyExecutor;

impl JobExecutor for ProbeOnlyExecutor {
    fn execute<'a>(
        &'a self,
        job: ReceivedJob,
    ) -> Pin<Box<dyn Future<Output = Result<JobResponse>> + Send + 'a>> {
        Box::pin(async move {
            Ok(match job.request {
                JobRequest::Probe => JobResponse::Probed {
                    host_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
                    vox: env!("CARGO_PKG_VERSION").to_string(),
                },
                JobRequest::Run { .. } => JobResponse::Failed(
                    "this node accepts Probe only: no sandbox is wired up yet, and \
                     running mesh-received work unsandboxed is exactly the hole this \
                     transport replaced"
                        .to_string(),
                ),
                JobRequest::Cancel { .. } => {
                    JobResponse::Failed("nothing to cancel: this node runs no jobs".to_string())
                }
            })
        })
    }
}

/// Bind a mesh endpoint.
///
/// `presets::Minimal` is not a default to be revisited — it is the whole
/// isolation guarantee. See the crate docs and ADR-047.
pub async fn bind(sk: SecretKey) -> Result<Endpoint> {
    let ep = Endpoint::builder(presets::Minimal)
        .secret_key(sk)
        // `Endpoint::bind(preset)` takes NO alpns; a server built that way
        // refuses every connection at ALPN negotiation, silently.
        .alpns(vec![protocol::ALPN.to_vec()])
        // Defence in depth. Under Minimal the relay map is empty, so the
        // default HTTPS latency probes and captive-portal check have no target
        // — but that is a property of another struct's defaults, not of ours.
        .net_report_config(NetReportConfig::minimal())
        .address_lookup(iroh_mdns_address_lookup::MdnsAddressLookup::builder())
        .bind()
        .await?;
    Ok(ep)
}

/// Accept loop. Bounded, trust-gated, and free of 0-RTT.
pub async fn serve(ep: Endpoint, trust: Arc<MeshTrust>, exec: Arc<dyn JobExecutor>) {
    let gate = Arc::new(Semaphore::new(MAX_INFLIGHT_HANDSHAKES));
    while let Some(incoming) = ep.accept().await {
        if gate.available_permits() < MAX_INFLIGHT_HANDSHAKES - RETRY_THRESHOLD
            && !incoming.remote_addr_validated()
        {
            let _ = incoming.retry();
            continue;
        }
        let Ok(permit) = Arc::clone(&gate).try_acquire_owned() else {
            // `ignore()` sends nothing at all; `refuse()` would answer, which
            // is both work for us and a signal for a scanner.
            incoming.ignore();
            continue;
        };
        let (trust, exec) = (Arc::clone(&trust), Arc::clone(&exec));
        tokio::spawn(async move {
            let _permit = permit;
            // Awaiting `incoming` completes the handshake, so `remote_id()`
            // below is the peer's *proven* public key. Never call
            // `Accepting::into_0rtt()`: there `remote_id()` is fallible and
            // every check that follows becomes advisory.
            let Ok(Ok(conn)) = timeout(HANDSHAKE_TIMEOUT, incoming).await else {
                return;
            };
            let remote = conn.remote_id();
            if !trust.is_trusted(&remote) {
                // No protocol-level explanation to a stranger — it would be an
                // oracle for "is this endpoint id known to you".
                conn.close(REFUSED_UNTRUSTED.into(), b"not trusted");
                return;
            }
            trust.register(remote, conn.clone());
            if let Err(e) = handle(conn, remote, exec).await {
                tracing::debug!(peer = %remote, error = %e, "mesh job stream ended");
            }
        });
    }
}

/// Serve one connection: greet, check the payload claim, execute, reply.
async fn handle(conn: Connection, peer: EndpointId, exec: Arc<dyn JobExecutor>) -> Result<()> {
    let (mut send, mut recv) = conn.accept_bi().await?;

    let hello: protocol::Hello = protocol::read_frame(&mut recv, 4096).await?;
    protocol::check_hello(&hello)?;

    let request: JobRequest = protocol::read_frame(&mut recv, 64 * 1024).await?;
    let limits = JobLimits::default();

    // Checked BEFORE the transfer, so an oversized job costs us a frame rather
    // than a gigabyte of disk.
    if let JobRequest::Run { payload_bytes, .. } = &request {
        if *payload_bytes > limits.max_payload_bytes {
            let msg = format!(
                "payload of {payload_bytes} bytes exceeds the {} byte cap",
                limits.max_payload_bytes
            );
            protocol::write_frame(&mut send, &JobResponse::Failed(msg)).await?;
            send.finish()?;
            conn.closed().await;
            return Ok(());
        }
    }

    let response = exec
        .execute(ReceivedJob {
            peer,
            request,
            limits,
        })
        .await
        .unwrap_or_else(|e| JobResponse::Failed(e.to_string()));

    protocol::write_frame(&mut send, &response).await?;
    send.finish()?;
    // `finish()` signals end-of-stream; it does NOT flush. Dropping the
    // Connection here would close it before the bytes reach the wire and the
    // peer would see `closed by peer: 0` with no payload. Measured during the
    // Task 0.2 spike; see ADR-047.
    conn.closed().await;
    Ok(())
}
