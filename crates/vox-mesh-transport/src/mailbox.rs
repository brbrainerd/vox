//! A2A store-and-forward mailbox, on its own ALPN (plan Task 3.1).
//!
//! What replaces `PopuliHttpClient::relay_a2a()`. A request/response RPC is not
//! a substitute for it: the peer this delivers to is routinely **switched off**,
//! and the HTTP call it replaces simply failed in that case and left the caller
//! to invent a retry. Here the queue is on disk before the dial is attempted,
//! so a message survives both a dark peer and a restart of this process.
//!
//! Two orderings this module exists to get right, both of which lose messages
//! if they are the other way round:
//!
//! 1. **The sender writes to the outbox before it dials.** A message that only
//!    ever existed in memory is lost by the crash that stopped it being sent.
//! 2. **The receiver stores before it acks.** An ack that means "I have the
//!    bytes" rather than "I have them on disk" turns every receiver crash into
//!    silent message loss — the sender has already deleted its copy.
//!
//! Deliberately its own ALPN rather than another `JobRequest` variant: a job is
//! a request whose answer the caller waits for, a mailbox message is a fact the
//! caller hands over and forgets. Sharing a protocol would mean sharing
//! [`JobLimits`](crate::protocol::JobLimits), and a mailbox message is four
//! orders of magnitude smaller than a job payload.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use iroh::endpoint::Connection;
use iroh::{Endpoint, EndpointAddr, EndpointId};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use vox_mesh_types::A2ADeliverRequest;

use crate::protocol::{self, Hello};
use crate::trust::MeshTrust;

/// ALPN for the mailbox protocol. Separate from `vox/job/1` on purpose: a peer
/// that serves jobs but not mail should refuse at TLS negotiation rather than
/// at a decode error three frames in.
pub const ALPN: &[u8] = b"vox/a2a/1";

/// How long one peer gets to accept a message before the flush gives up on it
/// and leaves the message queued.
const DELIVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Bounds applied to every mailbox message and to the queue itself.
///
/// Same shape and spirit as [`JobLimits`](crate::protocol::JobLimits): a cap on
/// the wire, a cap on disk, and a cap on concurrency, none of them the peer's
/// to choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxLimits {
    /// Largest single message accepted, checked against the declared frame
    /// length before a body byte is read.
    pub max_message_bytes: usize,
    /// How many messages may sit in the outbox before `queue` refuses. An
    /// unbounded outbox is a disk-filling bug that only shows up after a peer
    /// has been off for a week.
    pub max_outbox_depth: usize,
    /// Peers flushed concurrently. One dark peer must not delay the others, and
    /// eight machines is already more than this mesh's stated scope.
    pub max_inflight_peers: usize,
}

impl Default for MailboxLimits {
    fn default() -> Self {
        Self {
            // A2A payloads are JSON strings, not job bundles; the job plane's
            // 1 GiB cap would be a licence to fill the disk with mail.
            max_message_bytes: 4 * 1024 * 1024,
            max_outbox_depth: 4096,
            max_inflight_peers: 8,
        }
    }
}

/// What the sender puts on the wire.
///
/// The message travels as **JSON inside** the postcard frame, which looks
/// redundant and is not. `A2ADeliverRequest` marks its optional fields
/// `skip_serializing_if = "Option::is_none"`, and postcard is not
/// self-describing: a skipped field is simply absent from the byte stream, and
/// the decoder — which expects every field positionally — reads the next
/// field's bytes as the missing one's discriminant. The failure is a decode
/// error at best and a silently mis-parsed message at worst, and it is
/// invisible until a message with a `None` field is sent. The alternative,
/// changing those attributes, is a wire-format change to a type the HTTP plane
/// still serves.
///
/// No `PartialEq`: `A2ADeliverRequest` has none, and deriving one here would
/// mean adding it to a type in another crate to satisfy a test.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MailboxRequest {
    Deliver { json: String },
}

impl MailboxRequest {
    /// Wrap a message for the wire.
    pub fn deliver(req: &A2ADeliverRequest) -> Result<Self> {
        Ok(Self::Deliver {
            json: serde_json::to_string(req).context("serializing an A2A message for the mesh")?,
        })
    }

    /// Unwrap a message from the wire.
    pub fn into_request(self) -> Result<A2ADeliverRequest> {
        let Self::Deliver { json } = self;
        serde_json::from_str(&json).context("decoding an A2A message from the mesh")
    }
}

/// What the receiver answers, **after** the message is on its disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailboxAck {
    /// Durably stored. `duplicate` means the idempotency key was already
    /// present — which is still a successful delivery, and the sender must
    /// drop its copy rather than resend forever.
    Stored { duplicate: bool },
    /// Refused, with a reason phrased for a human. The sender keeps its copy.
    Refused(String),
}

// ---------------------------------------------------------------------------
// On-disk queues
// ---------------------------------------------------------------------------

/// Turn an idempotency key into a filename.
///
/// Hex rather than the raw key because keys are arbitrary UTF-8 and filenames
/// are not, and truncated-plus-FNV rather than plain hex because
/// `remote-result-<task>-<idempotency>` keys run past the 255-byte filename
/// limit on every filesystem we target. Not a security construct — a collision
/// costs a deduplicated message, not an authorisation.
fn slot_name(key: &str) -> String {
    const KEEP: usize = 96;
    let hex: String = key.bytes().map(|b| format!("{b:02x}")).collect();
    if hex.len() <= KEEP {
        return hex;
    }
    // FNV-1a, hand-rolled: this is a filename, not a digest, so it is
    // deliberately not routed through `vox-crypto`.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{}{h:016x}", &hex[..KEEP])
}

/// The dedupe slot for a message: its idempotency key when it has one, and
/// otherwise a name unique to this moment.
///
/// A message with no idempotency key cannot be deduplicated — that is the
/// caller's choice, made by leaving the field `None` — so it gets a fresh slot
/// rather than colliding with every other keyless message.
fn slot_for(req: &A2ADeliverRequest) -> String {
    match req.idempotency_key.as_deref() {
        Some(k) if !k.trim().is_empty() => format!("k{}", slot_name(k)),
        _ => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            format!("u{nanos:039x}")
        }
    }
}

/// Write `req` to `path` atomically, creating parents.
///
/// Temp-plus-rename for the same reason [`MeshTrust`] uses it: `fs::write`
/// truncates and then writes, so a crash in between leaves a zero-byte file —
/// here that is a message that was acked and is now unreadable, which is
/// exactly the loss the ack ordering exists to prevent.
fn write_atomically(path: &Path, req: &A2ADeliverRequest) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(req).context("serializing mailbox message")?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &json).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Every `*.json` under `dir`, ignoring what does not parse.
///
/// A single corrupt file must not hide the rest of the queue; it is logged and
/// skipped, which is the same fail-open-per-row / fail-closed-per-file choice
/// the trust store makes.
fn read_dir_messages(dir: &Path) -> Vec<(PathBuf, A2ADeliverRequest)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read(&path)
            .map_err(anyhow::Error::from)
            .and_then(|b| {
                serde_json::from_slice::<A2ADeliverRequest>(&b).map_err(anyhow::Error::from)
            }) {
            Ok(req) => out.push((path, req)),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipping unreadable mailbox message")
            }
        }
    }
    // Stable order so a flush is reproducible and a test can name the first
    // message it expects to see.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Messages this node has durably accepted from its peers.
///
/// One file per message, named by idempotency slot, so redelivery after a lost
/// ack is a no-op rather than a duplicate. The consumer drains it; this crate
/// only ever adds.
#[derive(Debug)]
pub struct Inbox {
    dir: PathBuf,
    limits: MailboxLimits,
}

impl Inbox {
    pub fn at(dir: &Path) -> Self {
        Self::with_limits(dir, MailboxLimits::default())
    }

    pub fn with_limits(dir: &Path, limits: MailboxLimits) -> Self {
        Self {
            dir: dir.to_path_buf(),
            limits,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn limits(&self) -> MailboxLimits {
        self.limits
    }

    /// Durably store `req`. `Ok(false)` means the slot already existed, which
    /// is a successful delivery of a message we already have.
    ///
    /// **Returns `Err` rather than swallowing an I/O failure**: the caller
    /// turns that into a refusal, and an ack sent over a failed write is how a
    /// store-and-forward system loses mail.
    pub fn store(&self, peer: &EndpointId, req: &A2ADeliverRequest) -> Result<bool> {
        let path = self
            .dir
            .join(peer.to_string())
            .join(format!("{}.json", slot_for(req)));
        if path.exists() {
            return Ok(false);
        }
        write_atomically(&path, req)?;
        Ok(true)
    }

    /// Everything received, from every peer.
    pub fn messages(&self) -> Vec<A2ADeliverRequest> {
        let Ok(peers) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for peer in peers.flatten() {
            out.extend(read_dir_messages(&peer.path()).into_iter().map(|(_, m)| m));
        }
        out
    }

    /// Remove a message the consumer has finished with.
    ///
    /// Keyed by the same slot the sender's idempotency key produced, so a
    /// redelivery after removal is stored again — the alternative, a permanent
    /// tombstone, is an unbounded on-disk set.
    pub fn remove(&self, peer: &EndpointId, req: &A2ADeliverRequest) -> Result<()> {
        let path = self
            .dir
            .join(peer.to_string())
            .join(format!("{}.json", slot_for(req)));
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow::Error::from(e).context(format!("removing {}", path.display()))),
        }
    }
}

/// Messages queued for peers, on disk, one directory per peer.
///
/// The queue is written before the dial, so it is the sender's durable copy:
/// a message is removed only once a peer has said it stored it.
#[derive(Debug)]
pub struct Outbox {
    dir: PathBuf,
    limits: MailboxLimits,
}

impl Outbox {
    pub fn at(dir: &Path) -> Self {
        Self::with_limits(dir, MailboxLimits::default())
    }

    pub fn with_limits(dir: &Path, limits: MailboxLimits) -> Self {
        Self {
            dir: dir.to_path_buf(),
            limits,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Queue `req` for `peer`, durably, before anything is dialed.
    pub fn queue(&self, peer: &EndpointId, req: &A2ADeliverRequest) -> Result<()> {
        if self.depth() >= self.limits.max_outbox_depth {
            anyhow::bail!(
                "mesh outbox is full at {} messages; the peer has been unreachable long enough \
                 that queueing more would fill the disk",
                self.limits.max_outbox_depth
            );
        }
        let path = self
            .dir
            .join(peer.to_string())
            .join(format!("{}.json", slot_for(req)));
        write_atomically(&path, req)
    }

    /// How many messages are queued for all peers.
    pub fn depth(&self) -> usize {
        let Ok(peers) = std::fs::read_dir(&self.dir) else {
            return 0;
        };
        peers
            .flatten()
            .map(|p| read_dir_messages(&p.path()).len())
            .sum()
    }

    /// What is queued for one peer, oldest slot first.
    pub fn pending_for(&self, peer: &EndpointId) -> Vec<A2ADeliverRequest> {
        read_dir_messages(&self.dir.join(peer.to_string()))
            .into_iter()
            .map(|(_, m)| m)
            .collect()
    }

    /// Try to deliver everything queued. Returns how many messages were
    /// accepted.
    ///
    /// Never returns `Err`: a peer that is off is the normal case, not a
    /// failure, and one dark machine must not stop the others being flushed.
    /// Peers are flushed concurrently up to
    /// [`MailboxLimits::max_inflight_peers`].
    pub async fn flush(&self, ep: &Endpoint, trust: &MeshTrust) -> usize {
        let Ok(peer_dirs) = std::fs::read_dir(&self.dir) else {
            return 0;
        };
        let rows = trust.rows();
        let mut set = tokio::task::JoinSet::new();
        let mut delivered = 0usize;
        let mut inflight = 0usize;

        for entry in peer_dirs.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(peer) = name.parse::<EndpointId>() else {
                continue;
            };
            let queued = read_dir_messages(&entry.path());
            if queued.is_empty() {
                continue;
            }
            // No stored address means unreachable, not "discover it": mDNS does
            // not announce (spike findings Q4), so an id alone is not dialable.
            let addrs: Vec<std::net::SocketAddr> = rows
                .iter()
                .find(|r| r.endpoint_id == name)
                .map(|r| r.addrs.iter().filter_map(|a| a.parse().ok()).collect())
                .unwrap_or_default();
            if addrs.is_empty() {
                tracing::debug!(%peer, "queued mail for a peer with no stored address; skipping");
                continue;
            }

            if inflight >= self.limits.max_inflight_peers
                && let Some(Ok(n)) = set.join_next().await
            {
                delivered += n;
                inflight -= 1;
            }
            let ep = ep.clone();
            inflight += 1;
            set.spawn(async move { deliver_to_peer(&ep, peer, addrs, queued).await });
        }

        while let Some(joined) = set.join_next().await {
            delivered += joined.unwrap_or(0);
        }
        delivered
    }
}

/// Deliver every queued message to one peer over a single connection, removing
/// each file only once the peer says it stored it.
///
/// One connection, many bi-streams: a dial per message would make a backlog of
/// a thousand messages a thousand handshakes.
async fn deliver_to_peer(
    ep: &Endpoint,
    peer: EndpointId,
    addrs: Vec<std::net::SocketAddr>,
    queued: Vec<(PathBuf, A2ADeliverRequest)>,
) -> usize {
    let mut addr = EndpointAddr::new(peer);
    for a in addrs {
        addr = addr.with_ip_addr(a);
    }
    let conn = match timeout(DELIVER_TIMEOUT, ep.connect(addr, ALPN)).await {
        Ok(Ok(conn)) => conn,
        // Off, refused, or unreachable. All three mean "keep the queue".
        _ => return 0,
    };

    let mut delivered = 0usize;
    for (path, req) in queued {
        match timeout(DELIVER_TIMEOUT, deliver_one(&conn, &req)).await {
            Ok(Ok(MailboxAck::Stored { .. })) => {
                // Only now is it safe to drop our copy.
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!(path = %path.display(), error = %e, "delivered mail could not be dequeued; it will be resent and deduplicated");
                }
                delivered += 1;
            }
            Ok(Ok(MailboxAck::Refused(why))) => {
                tracing::warn!(%peer, reason = %why, "peer refused a mailbox message; leaving it queued");
                break;
            }
            Ok(Err(e)) => {
                tracing::debug!(%peer, error = %e, "mailbox delivery failed mid-connection");
                break;
            }
            Err(_) => {
                tracing::debug!(%peer, "mailbox delivery timed out");
                break;
            }
        }
    }
    conn.close(0u32.into(), b"flushed");
    delivered
}

/// One message on one bi-stream: greet, deliver, wait for the ack.
async fn deliver_one(conn: &Connection, req: &A2ADeliverRequest) -> Result<MailboxAck> {
    let (mut send, mut recv) = conn.open_bi().await?;
    protocol::write_frame(&mut send, &Hello::current()).await?;
    protocol::write_frame(&mut send, &MailboxRequest::deliver(req)?).await?;
    send.finish()?;
    // The ack is read on this same stream before the connection is closed, so
    // `finish()` not flushing is not a hazard here the way it is for a
    // fire-and-forget reply — but the read below is what proves it.
    protocol::read_frame(&mut recv, 64 * 1024).await
}

/// Serve one mailbox connection: greet, store, ack, repeat until the peer
/// closes.
///
/// Trust is re-checked per message, not only at accept: a connection opened
/// while trusted must stop being served the moment an operator revokes it.
pub async fn handle(
    conn: Connection,
    peer: EndpointId,
    trust: Arc<MeshTrust>,
    inbox: Arc<Inbox>,
) -> Result<()> {
    let max = inbox.limits().max_message_bytes;
    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
        let hello: Hello = protocol::read_frame(&mut recv, 4096).await?;
        protocol::check_hello(&hello)?;

        let framed = protocol::read_frame::<MailboxRequest>(&mut recv, max).await;
        if framed.is_ok() && !trust.is_trusted(&peer) {
            // Re-checked per message, not only at accept: a connection opened
            // while trusted must stop being served the moment trust is revoked.
            protocol::write_frame(&mut send, &MailboxAck::Refused("not trusted".into())).await?;
            send.finish()?;
            conn.close(crate::trust::REVOKED.into(), b"trust revoked");
            return Ok(());
        }
        let ack = match framed.and_then(MailboxRequest::into_request) {
            Ok(req) => match inbox.store(&peer, &req) {
                // Stored, and only now do we say so. The sender deletes its
                // copy on the strength of this frame.
                Ok(stored) => MailboxAck::Stored { duplicate: !stored },
                Err(e) => {
                    tracing::error!(%peer, error = %e, "mailbox store failed; refusing rather than acking");
                    MailboxAck::Refused(format!("could not store the message: {e}"))
                }
            },
            Err(e) => MailboxAck::Refused(e.to_string()),
        };

        protocol::write_frame(&mut send, &ack).await?;
        send.finish()?;
    }
    // Awaiting `accept_bi` above is what keeps the connection alive long enough
    // for the last ack to reach the wire; `finish()` alone does not flush.
    conn.closed().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    fn peer() -> EndpointId {
        SecretKey::from_bytes(&[3u8; 32]).public()
    }

    fn req(key: Option<&str>) -> A2ADeliverRequest {
        A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "2".into(),
            message_type: "t".into(),
            payload: "{}".into(),
            idempotency_key: key.map(str::to_owned),
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

    #[test]
    fn the_alpn_is_not_the_job_alpn() {
        // Sharing one would mean a job-only peer decoding mail as a JobRequest.
        assert_ne!(ALPN, crate::protocol::ALPN);
    }

    #[test]
    fn a_slot_name_is_a_legal_filename_however_long_the_key() {
        let long = "remote-result-".to_string() + &"9".repeat(4000);
        let name = slot_name(&long);
        assert!(name.len() < 200, "must fit a filename: {}", name.len());
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(
            slot_name(&long),
            slot_name(&(long.clone() + "x")),
            "two long keys sharing a prefix must not share a slot"
        );
    }

    #[test]
    fn keyless_messages_do_not_deduplicate_against_each_other() {
        // Leaving idempotency_key None is the caller saying "these are distinct".
        assert_ne!(slot_for(&req(None)), slot_for(&req(None)));
        assert_eq!(slot_for(&req(Some("k"))), slot_for(&req(Some("k"))));
    }

    #[test]
    fn storing_the_same_key_twice_is_a_duplicate_not_a_second_message() {
        let d = tempfile::tempdir().unwrap();
        let inbox = Inbox::at(d.path());
        assert!(inbox.store(&peer(), &req(Some("k"))).unwrap());
        assert!(
            !inbox.store(&peer(), &req(Some("k"))).unwrap(),
            "a redelivery must report duplicate, not store a second copy"
        );
        assert_eq!(inbox.messages().len(), 1);
    }

    #[test]
    fn a_store_that_cannot_write_reports_an_error_rather_than_success() {
        // The caller turns this into a refusal. Silently succeeding here is
        // exactly how an ack comes to mean nothing.
        let inbox = Inbox::at(Path::new("/nonexistent-vox-root/inbox"));
        assert!(inbox.store(&peer(), &req(Some("k"))).is_err());
    }

    #[test]
    fn a_removed_message_leaves_the_inbox() {
        let d = tempfile::tempdir().unwrap();
        let inbox = Inbox::at(d.path());
        inbox.store(&peer(), &req(Some("k"))).unwrap();
        inbox.remove(&peer(), &req(Some("k"))).unwrap();
        assert!(inbox.messages().is_empty());
        // Removing twice is not an error: the consumer may crash mid-drain.
        inbox.remove(&peer(), &req(Some("k"))).unwrap();
    }

    #[test]
    fn a_queued_message_is_on_disk_before_anything_is_dialed() {
        let d = tempfile::tempdir().unwrap();
        Outbox::at(d.path())
            .queue(&peer(), &req(Some("k")))
            .unwrap();
        // A fresh handle over the same directory is what a restart looks like.
        assert_eq!(Outbox::at(d.path()).depth(), 1);
        assert_eq!(Outbox::at(d.path()).pending_for(&peer()).len(), 1);
    }

    #[test]
    fn the_outbox_refuses_to_grow_without_bound() {
        let d = tempfile::tempdir().unwrap();
        let limits = MailboxLimits {
            max_outbox_depth: 2,
            ..MailboxLimits::default()
        };
        let outbox = Outbox::with_limits(d.path(), limits);
        outbox.queue(&peer(), &req(Some("a"))).unwrap();
        outbox.queue(&peer(), &req(Some("b"))).unwrap();
        let err = outbox.queue(&peer(), &req(Some("c"))).unwrap_err();
        assert!(err.to_string().contains("full"), "{err}");
    }

    #[test]
    fn queue_writes_are_atomic_so_a_crash_cannot_leave_an_empty_message() {
        let d = tempfile::tempdir().unwrap();
        let outbox = Outbox::at(d.path());
        outbox.queue(&peer(), &req(Some("k"))).unwrap();
        let peer_dir = d.path().join(peer().to_string());
        let leftovers: Vec<_> = std::fs::read_dir(&peer_dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temp files may survive a write");
        assert_eq!(outbox.pending_for(&peer())[0].payload, "{}");
    }

    #[test]
    fn a_corrupt_message_file_does_not_hide_the_rest_of_the_queue() {
        let d = tempfile::tempdir().unwrap();
        let outbox = Outbox::at(d.path());
        outbox.queue(&peer(), &req(Some("good"))).unwrap();
        std::fs::write(
            d.path().join(peer().to_string()).join("broken.json"),
            b"{ not json",
        )
        .unwrap();
        assert_eq!(outbox.depth(), 1, "the readable message must survive");
    }

    #[test]
    fn the_defaults_are_bounded() {
        let l = MailboxLimits::default();
        assert_eq!(l.max_message_bytes, 4 * 1024 * 1024);
        assert_eq!(l.max_outbox_depth, 4096);
        assert_eq!(l.max_inflight_peers, 8);
    }

    #[test]
    fn the_wire_types_round_trip() {
        // Non-vacuous on the field that broke it: a message with `None`
        // optionals is the case postcard cannot carry directly, because
        // `skip_serializing_if` omits bytes the decoder expects positionally.
        let r = MailboxRequest::deliver(&req(Some("k"))).unwrap();
        let back: MailboxRequest = protocol::decode(&protocol::encode(&r).unwrap()).unwrap();
        let back = back.into_request().unwrap();
        assert_eq!(back.idempotency_key.as_deref(), Some("k"));
        assert_eq!(back.payload, "{}");
        assert!(back.privacy_class.is_none());

        let keyless = MailboxRequest::deliver(&req(None)).unwrap();
        let back: MailboxRequest = protocol::decode(&protocol::encode(&keyless).unwrap()).unwrap();
        assert!(back.into_request().unwrap().idempotency_key.is_none());
        for ack in [
            MailboxAck::Stored { duplicate: false },
            MailboxAck::Stored { duplicate: true },
            MailboxAck::Refused("no".into()),
        ] {
            let back: MailboxAck = protocol::decode(&protocol::encode(&ack).unwrap()).unwrap();
            assert_eq!(ack, back);
        }
    }
}
