//! Two-machine smoke test for the mesh transport.
//!
//! Exercises the real crate — persisted identity, the trust allowlist, the
//! framed protocol, and the bounded accept loop — between two machines, before
//! the `vox mesh join` CLI exists. Phase 2 replaces the argument parsing here
//! with those verbs; the transport path underneath is the same.
//!
//! ```text
//! # on the listener
//! cargo run -p vox-mesh-transport --example mesh_smoke -- serve <state-dir>
//! # it prints its EndpointId and ticket; then, on the dialer
//! cargo run -p vox-mesh-transport --example mesh_smoke -- trust <state-dir> <peer-id>
//! cargo run -p vox-mesh-transport --example mesh_smoke -- dial <state-dir> <ticket>
//! ```
//!
//! The listener must be told to trust the dialer first — refusing by default is
//! the property under test, not an inconvenience to work around.

use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Result, bail};
use iroh::EndpointId;
use iroh_tickets::endpoint::EndpointTicket;
use vox_mesh_transport::endpoint::{JobExecutor, ReceivedJob};
use vox_mesh_transport::protocol::{self, ALPN, Hello, JobRequest, JobResponse};
use vox_mesh_transport::{MeshTrust, identity};

/// Answers `Probe` with this machine's host triple, which is the whole point of
/// the cross-platform check: the reply must name the *other* machine's target.
struct ProbeExecutor;

impl JobExecutor for ProbeExecutor {
    fn execute<'a>(
        &'a self,
        job: ReceivedJob,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<JobResponse>> + Send + 'a>> {
        Box::pin(async move {
            println!(
                "  executing {:?} for {} at {:?}",
                job.request, job.peer, job.limits.isolation
            );
            Ok(JobResponse::Probed {
                host_triple: current_arch_os(),
                vox: env!("CARGO_PKG_VERSION").to_string(),
            })
        })
    }
}

/// `arch-os`, not a full LLVM target triple.
///
/// A real `host_triple` comes from the capability probe layer that ADR-018
/// upholds and that survives this rewrite; the smoke test only needs enough to
/// show that the reply describes the *other* machine.
fn current_arch_os() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let dir = PathBuf::from(args.next().unwrap_or_else(|| ".mesh-smoke".into()));

    let sk = identity::load_or_create(&dir.join("mesh.key"))?;
    let trust = Arc::new(MeshTrust::at(&dir.join("mesh_trust.json")));

    match cmd.as_str() {
        "serve" => {
            let ep = vox_mesh_transport::endpoint::bind(sk).await?;
            println!("endpoint-id: {}", ep.id());
            println!("ticket: {}", EndpointTicket::new(ep.addr()));
            println!("addr: {:?}", ep.addr());
            let exec: Arc<dyn JobExecutor> = Arc::new(ProbeExecutor);
            vox_mesh_transport::endpoint::serve(ep, trust, exec).await;
        }
        "trust" => {
            let peer = args.next().unwrap_or_default();
            let id = EndpointId::from_str(&peer)?;
            trust.trust(&id, Some("mesh-smoke"))?;
            println!("trusted {id} at {:?}", trust.level(&id));
        }
        "dial" => {
            let ticket: EndpointTicket = args.next().unwrap_or_default().parse()?;
            let ep = vox_mesh_transport::endpoint::bind(sk).await?;
            println!("our endpoint-id: {}", ep.id());
            let t0 = std::time::Instant::now();
            let conn = ep.connect(ticket.endpoint_addr().clone(), ALPN).await?;
            println!("connected in {:?} to {}", t0.elapsed(), conn.remote_id());

            let (mut send, mut recv) = conn.open_bi().await?;
            protocol::write_frame(&mut send, &Hello::current()).await?;
            protocol::write_frame(&mut send, &JobRequest::Probe).await?;
            send.finish()?;
            let resp: JobResponse = protocol::read_frame(&mut recv, 1024 * 1024).await?;
            println!("response: {resp:?}");
            conn.close(0u32.into(), b"done");
        }
        _ => bail!("serve <dir> | trust <dir> <peer-id> | dial <dir> <ticket>"),
    }
    Ok(())
}
