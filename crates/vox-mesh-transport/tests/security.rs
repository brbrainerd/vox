//! The four properties the mesh's security model reduces to. Each runs two real
//! iroh endpoints over loopback — a mock of the transport would not exercise
//! the thing under test.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use iroh::{Endpoint, EndpointId, SecretKey};
use tokio::time::timeout;
use vox_mesh_transport::endpoint::{JobExecutor, ReceivedJob};
use vox_mesh_transport::protocol::{
    self, ALPN, Hello, Isolation, JobLimits, JobRequest, JobResponse,
};
use vox_mesh_transport::trust::MeshTrust;

/// Counts invocations and remembers the limits it was handed, so a test can
/// assert both "the executor was never reached" and "it ran sandboxed".
#[derive(Default)]
struct SpyExecutor {
    invocations: AtomicUsize,
    last_limits: Mutex<Option<JobLimits>>,
}

impl SpyExecutor {
    fn invocations(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }
    fn last_limits(&self) -> Option<JobLimits> {
        *self.last_limits.lock().unwrap()
    }
}

impl JobExecutor for SpyExecutor {
    fn execute<'a>(
        &'a self,
        job: ReceivedJob,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<JobResponse>> + Send + 'a>> {
        Box::pin(async move {
            self.invocations.fetch_add(1, Ordering::SeqCst);
            *self.last_limits.lock().unwrap() = Some(job.limits);
            Ok(JobResponse::Output(b"ok".to_vec()))
        })
    }
}

struct Server {
    id: EndpointId,
    addr: iroh::EndpointAddr,
    trust: Arc<MeshTrust>,
    spy: Arc<SpyExecutor>,
    _dir: tempfile::TempDir,
}

async fn start_server() -> Server {
    let dir = tempfile::tempdir().unwrap();
    let trust = Arc::new(MeshTrust::at(&dir.path().join("mesh_trust.json")));
    let spy = Arc::new(SpyExecutor::default());
    let ep = vox_mesh_transport::endpoint::bind(SecretKey::generate())
        .await
        .expect("bind server");
    let (id, addr) = (ep.id(), ep.addr());
    let exec: Arc<dyn JobExecutor> = spy.clone();
    tokio::spawn(vox_mesh_transport::endpoint::serve(
        ep,
        Arc::clone(&trust),
        exec,
    ));
    Server {
        id,
        addr,
        trust,
        spy,
        _dir: dir,
    }
}

/// A client endpoint. Built with the same `Minimal` preset, so nothing in this
/// test suite reaches a relay or a DNS server.
async fn client_endpoint(sk: SecretKey) -> Endpoint {
    Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(sk)
        .bind()
        .await
        .expect("bind client")
}

async fn send_request_on(
    conn: &iroh::endpoint::Connection,
    request: JobRequest,
) -> Result<JobResponse> {
    let (mut send, mut recv) = conn.open_bi().await?;
    protocol::write_frame(&mut send, &Hello::current()).await?;
    protocol::write_frame(&mut send, &request).await?;
    send.finish()?;
    protocol::read_frame(&mut recv, 16 * 1024 * 1024).await
}

#[tokio::test]
async fn an_untrusted_peer_cannot_reach_the_executor() {
    let server = start_server().await;
    let client = client_endpoint(SecretKey::generate()).await;

    // No `trust()` call: this peer is a stranger.
    let conn = client
        .connect(server.addr.clone(), ALPN)
        .await
        .expect("connect");

    let result = timeout(
        Duration::from_secs(10),
        send_request_on(&conn, JobRequest::Probe),
    )
    .await
    .expect("no timeout");

    // Non-vacuous on two axes: the executor must not run, AND the request must
    // actually fail. Asserting only the first would still pass if the whole
    // connection had failed for an unrelated reason.
    assert!(
        result.is_err(),
        "a stranger's request must not succeed, got {result:?}"
    );
    assert_eq!(
        server.spy.invocations(),
        0,
        "an untrusted peer must never reach the executor"
    );
    assert!(
        timeout(Duration::from_secs(5), conn.closed()).await.is_ok(),
        "the server must close on a stranger rather than leaving them connected"
    );
}

#[tokio::test]
async fn a_trusted_peer_gets_a_sandbox_by_default() {
    let server = start_server().await;
    let sk = SecretKey::generate();
    let client_id = sk.public();
    let client = client_endpoint(sk).await;

    // Ordinary pairing — exactly what `vox mesh join` calls.
    server.trust.trust(&client_id, None).unwrap();

    let conn = client
        .connect(server.addr.clone(), ALPN)
        .await
        .expect("connect");
    let resp = timeout(
        Duration::from_secs(10),
        send_request_on(
            &conn,
            JobRequest::Run {
                kind: vox_mesh_types::TaskKind::VoxScript,
                payload_bytes: 128,
            },
        ),
    )
    .await
    .expect("no timeout")
    .expect("job ran");

    assert!(matches!(resp, JobResponse::Output(_)), "{resp:?}");
    let limits = server.spy.last_limits().expect("executor saw limits");
    assert_eq!(
        limits.isolation,
        Isolation::Wasm,
        "pairing must not grant native execution"
    );
    assert!(limits.wall_clock <= Duration::from_secs(300));
}

#[tokio::test]
async fn untrust_closes_a_live_connection() {
    // iroh has no Endpoint-level "close everything to this peer", so MeshTrust
    // holds the handles. Without this, revocation is a file write and nothing
    // else — the peer keeps the connection it already has.
    let server = start_server().await;
    let sk = SecretKey::generate();
    let client_id = sk.public();
    let client = client_endpoint(sk).await;
    server.trust.trust(&client_id, None).unwrap();

    let conn = client
        .connect(server.addr.clone(), ALPN)
        .await
        .expect("connect");
    // Give the server's accept task a moment to register the connection.
    for _ in 0..50 {
        if server.trust.is_trusted(&client_id) {
            tokio::time::sleep(Duration::from_millis(20)).await;
            break;
        }
    }

    server.trust.untrust(&client_id).unwrap();

    assert!(
        timeout(Duration::from_secs(5), conn.closed()).await.is_ok(),
        "untrust must close the live connection, not just edit a file"
    );
}

#[tokio::test]
async fn a_payload_larger_than_the_cap_is_refused_before_any_transfer() {
    let server = start_server().await;
    let sk = SecretKey::generate();
    let client_id = sk.public();
    let client = client_endpoint(sk).await;
    server.trust.trust(&client_id, None).unwrap();

    let over = JobLimits::default().max_payload_bytes + 1;
    let conn = client
        .connect(server.addr.clone(), ALPN)
        .await
        .expect("connect");
    let resp = timeout(
        Duration::from_secs(10),
        send_request_on(
            &conn,
            JobRequest::Run {
                kind: vox_mesh_types::TaskKind::VoxScript,
                payload_bytes: over,
            },
        ),
    )
    .await
    .expect("no timeout")
    .expect("got a response");

    match resp {
        JobResponse::Failed(msg) => assert!(msg.contains("exceeds"), "{msg}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(
        server.spy.invocations(),
        0,
        "the cap must be enforced before the executor is reached"
    );
}

#[tokio::test]
async fn the_server_id_is_the_public_key() {
    // The EndpointId IS the ed25519 public key; trust is keyed on it, so a
    // divergence here would silently break every allowlist lookup.
    let server = start_server().await;
    assert_eq!(server.id, server.addr.id);
}
