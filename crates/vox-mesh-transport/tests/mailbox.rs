//! The properties store-and-forward reduces to, over two live iroh endpoints.
//!
//! A request/response RPC would pass none of these: the point of the mailbox is
//! that the peer is *off* when you send, and that nothing is dropped when the
//! network lies to you about whether it arrived.

use std::sync::Arc;
use std::time::Duration;

use iroh::{Endpoint, SecretKey};
use tokio::time::timeout;
use vox_mesh_transport::endpoint::ProbeOnlyExecutor;
use vox_mesh_transport::mailbox::{Inbox, MailboxLimits, Outbox};
use vox_mesh_transport::trust::MeshTrust;
use vox_mesh_types::A2ADeliverRequest;

fn msg(idempotency_key: &str, payload: &str) -> A2ADeliverRequest {
    A2ADeliverRequest {
        sender_agent_id: "1".into(),
        receiver_agent_id: "2".into(),
        message_type: "vox.remote_task_result".into(),
        payload: payload.into(),
        idempotency_key: Some(idempotency_key.into()),
        privacy_class: None,
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    }
}

/// A receiver: its own endpoint, trust store and inbox, all under one tempdir
/// so a test can restart the *sender* without disturbing it.
struct Receiver {
    id: iroh::EndpointId,
    port: u16,
    trust: Arc<MeshTrust>,
    inbox: Arc<Inbox>,
    _dir: tempfile::TempDir,
}

async fn start_receiver_with(inbox: Arc<Inbox>, sk: SecretKey) -> Receiver {
    let dir = tempfile::tempdir().unwrap();
    let trust = Arc::new(MeshTrust::at(&dir.path().join("mesh_trust.json")));
    let ep = vox_mesh_transport::endpoint::bind(sk).await.expect("bind");
    let id = ep.id();
    // `Endpoint::addr()` advertises LAN/VPN addresses and never loopback; the
    // endpoint binds `0.0.0.0`, so taking the port and dialing 127.0.0.1 keeps
    // the test hermetic and off the physical network.
    let port = ep
        .addr()
        .ip_addrs()
        .next()
        .expect("a bound endpoint advertises an address")
        .port();
    tokio::spawn(vox_mesh_transport::endpoint::serve(
        ep,
        Arc::clone(&trust),
        Arc::new(ProbeOnlyExecutor),
        Some(Arc::clone(&inbox)),
    ));
    Receiver {
        id,
        port,
        trust,
        inbox,
        _dir: dir,
    }
}

async fn start_receiver(sk: SecretKey) -> (tempfile::TempDir, Receiver) {
    let inbox_dir = tempfile::tempdir().unwrap();
    let inbox = Arc::new(Inbox::at(inbox_dir.path()));
    let r = start_receiver_with(inbox, sk).await;
    (inbox_dir, r)
}

async fn sender_endpoint(sk: SecretKey) -> Endpoint {
    Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(sk)
        .bind()
        .await
        .expect("bind sender")
}

fn loopback(port: u16) -> Vec<std::net::SocketAddr> {
    vec![format!("127.0.0.1:{port}").parse().unwrap()]
}

#[tokio::test]
async fn a_message_queued_for_an_offline_peer_is_delivered_when_it_comes_back() {
    // The whole reason this is a mailbox and not an RPC. The receiver's key is
    // fixed up front so the peer that comes back is the same peer we queued for.
    let receiver_sk = SecretKey::generate();
    let receiver_id = receiver_sk.public();

    let out_dir = tempfile::tempdir().unwrap();
    let sender_trust_dir = tempfile::tempdir().unwrap();
    let sender_trust = MeshTrust::at(&sender_trust_dir.path().join("mesh_trust.json"));
    // A port nothing is listening on yet: the peer is switched off.
    let port = 32871;
    sender_trust
        .trust_with_addrs(&receiver_id, Some("dark"), &loopback(port))
        .unwrap();

    let sender = sender_endpoint(SecretKey::generate()).await;
    let outbox = Outbox::at(out_dir.path());
    outbox.queue(&receiver_id, &msg("k1", "hello")).unwrap();

    let delivered = timeout(
        Duration::from_secs(20),
        outbox.flush(&sender, &sender_trust),
    )
    .await
    .expect("flush against a dark peer must not hang");
    assert_eq!(delivered, 0, "nothing can be delivered to a dark peer");
    assert_eq!(
        outbox.depth(),
        1,
        "an undelivered message must stay queued, not evaporate"
    );

    // The peer comes back -- and the sender is a *fresh* Outbox handle over the
    // same directory, which is what a process restart looks like from here.
    let inbox_dir = tempfile::tempdir().unwrap();
    let receiver = start_receiver_with(Arc::new(Inbox::at(inbox_dir.path())), receiver_sk).await;
    receiver.trust.trust(&sender.id(), None).unwrap();
    let reopened_trust = MeshTrust::at(&sender_trust_dir.path().join("mesh_trust.json"));
    reopened_trust
        .trust_with_addrs(&receiver.id, Some("back"), &loopback(receiver.port))
        .unwrap();

    let reopened = Outbox::at(out_dir.path());
    assert_eq!(reopened.depth(), 1, "the queue is on disk, not in memory");
    let delivered = timeout(
        Duration::from_secs(20),
        reopened.flush(&sender, &reopened_trust),
    )
    .await
    .expect("no timeout");
    assert_eq!(delivered, 1, "the message must land once the peer answers");
    assert_eq!(reopened.depth(), 0, "a delivered message leaves the outbox");
    assert_eq!(receiver.inbox.messages().len(), 1);
    assert_eq!(receiver.inbox.messages()[0].payload, "hello");
}

#[tokio::test]
async fn a_redelivery_after_a_lost_ack_does_not_duplicate() {
    // A lost ack is indistinguishable from a lost message, so the sender must
    // be free to resend -- which is only safe if the receiver deduplicates.
    let (_inbox_dir, receiver) = start_receiver(SecretKey::generate()).await;
    let sender_sk = SecretKey::generate();
    let sender = sender_endpoint(sender_sk.clone()).await;
    receiver.trust.trust(&sender_sk.public(), None).unwrap();

    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    trust
        .trust_with_addrs(&receiver.id, None, &loopback(receiver.port))
        .unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        let outbox = Outbox::at(out_dir.path());
        outbox
            .queue(&receiver.id, &msg("same-key", "body"))
            .unwrap();
        timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
            .await
            .expect("no timeout");
    }

    assert_eq!(
        receiver.inbox.messages().len(),
        1,
        "the same idempotency key must land once, however many times it is sent"
    );
}

#[tokio::test]
async fn an_untrusted_sender_cannot_write_to_the_inbox() {
    let (_inbox_dir, receiver) = start_receiver(SecretKey::generate()).await;
    // Deliberately no `trust()` on the receiver: this sender is a stranger.
    let sender = sender_endpoint(SecretKey::generate()).await;

    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    trust
        .trust_with_addrs(&receiver.id, None, &loopback(receiver.port))
        .unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::at(out_dir.path());
    outbox.queue(&receiver.id, &msg("k", "payload")).unwrap();
    let delivered = timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
        .await
        .expect("no timeout");

    assert_eq!(delivered, 0, "a stranger's message must not be accepted");
    assert!(
        receiver.inbox.messages().is_empty(),
        "an untrusted peer must never reach the inbox"
    );
    assert_eq!(outbox.depth(), 1, "a refused message stays the sender's");
}

#[tokio::test]
async fn revoking_trust_stops_delivery_mid_connection() {
    // Trust is checked at accept *and* per message: a connection opened while
    // trusted must not keep writing after revocation.
    let (_inbox_dir, receiver) = start_receiver(SecretKey::generate()).await;
    let sender_sk = SecretKey::generate();
    let sender_id = sender_sk.public();
    let sender = sender_endpoint(sender_sk).await;
    receiver.trust.trust(&sender_id, None).unwrap();

    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    trust
        .trust_with_addrs(&receiver.id, None, &loopback(receiver.port))
        .unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::at(out_dir.path());
    outbox.queue(&receiver.id, &msg("first", "a")).unwrap();
    let delivered = timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
        .await
        .expect("no timeout");
    assert_eq!(delivered, 1);

    receiver.trust.untrust(&sender_id).unwrap();
    outbox.queue(&receiver.id, &msg("second", "b")).unwrap();
    let delivered = timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
        .await
        .expect("no timeout");
    assert_eq!(delivered, 0, "revocation must stop the next message");
    assert_eq!(receiver.inbox.messages().len(), 1);
}

#[tokio::test]
async fn a_message_that_could_not_be_stored_is_never_acked() {
    // The ordering that decides whether a crash loses messages: an ack means
    // "durably stored", never "I have the bytes". Pointing the inbox at a path
    // that cannot be created makes every store fail.
    let unwritable = std::path::Path::new("/nonexistent-vox-mailbox-root/inbox");
    let receiver =
        start_receiver_with(Arc::new(Inbox::at(unwritable)), SecretKey::generate()).await;
    let sender_sk = SecretKey::generate();
    let sender = sender_endpoint(sender_sk.clone()).await;
    receiver.trust.trust(&sender_sk.public(), None).unwrap();

    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    trust
        .trust_with_addrs(&receiver.id, None, &loopback(receiver.port))
        .unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::at(out_dir.path());
    outbox.queue(&receiver.id, &msg("k", "body")).unwrap();
    let delivered = timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
        .await
        .expect("no timeout");

    assert_eq!(delivered, 0, "a failed store must not be acked");
    assert_eq!(
        outbox.depth(),
        1,
        "the sender keeps the only surviving copy when the receiver could not store it"
    );
}

#[tokio::test]
async fn a_message_over_the_size_cap_is_refused_rather_than_stored() {
    let inbox_dir = tempfile::tempdir().unwrap();
    let limits = MailboxLimits {
        max_message_bytes: 1024,
        ..MailboxLimits::default()
    };
    let receiver = start_receiver_with(
        Arc::new(Inbox::with_limits(inbox_dir.path(), limits)),
        SecretKey::generate(),
    )
    .await;
    let sender_sk = SecretKey::generate();
    let sender = sender_endpoint(sender_sk.clone()).await;
    receiver.trust.trust(&sender_sk.public(), None).unwrap();

    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    trust
        .trust_with_addrs(&receiver.id, None, &loopback(receiver.port))
        .unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::at(out_dir.path());
    outbox
        .queue(&receiver.id, &msg("big", &"x".repeat(64 * 1024)))
        .unwrap();
    let delivered = timeout(Duration::from_secs(20), outbox.flush(&sender, &trust))
        .await
        .expect("no timeout");

    assert_eq!(delivered, 0, "an oversized message must be refused");
    assert!(receiver.inbox.messages().is_empty());
}

#[tokio::test]
async fn a_peer_with_no_stored_address_is_skipped_rather_than_dialed() {
    // Regression guard for the mDNS finding: an EndpointId alone is not
    // dialable, so a queue for an address-less peer must not stall the flush.
    let sender = sender_endpoint(SecretKey::generate()).await;
    let trust_dir = tempfile::tempdir().unwrap();
    let trust = MeshTrust::at(&trust_dir.path().join("mesh_trust.json"));
    let peer = SecretKey::generate().public();
    trust.trust(&peer, None).unwrap();

    let out_dir = tempfile::tempdir().unwrap();
    let outbox = Outbox::at(out_dir.path());
    outbox.queue(&peer, &msg("k", "body")).unwrap();

    let delivered = timeout(Duration::from_secs(5), outbox.flush(&sender, &trust))
        .await
        .expect("an address-less peer must be skipped, not dialed until timeout");
    assert_eq!(delivered, 0);
    assert_eq!(outbox.depth(), 1);
}
