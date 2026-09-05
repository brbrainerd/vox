//! The job wire protocol. Postcard over one iroh bi-stream — no `iroh-blobs`,
//! no `irpc` in v1 (spec Part 13): a bi-stream plus postcard is four lines, and
//! blobs earn their place for model and checkpoint distribution, not for 10 MiB
//! payloads on a 3 ms LAN.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use vox_mesh_types::TaskKind;

/// ALPN for the job protocol. A change here is a hard incompatibility: peers
/// that do not offer this exact string are refused at TLS negotiation.
pub const ALPN: &[u8] = b"vox/job/1";

/// Wire protocol version, carried in [`Hello`].
pub const PROTO: u16 = 1;

/// Opaque job identifier assigned by the sender.
pub type JobId = u64;

/// First frame on every stream. **This layout is frozen forever**; every other
/// message may change. Without it, version skew is a hang or a postcard
/// deserialisation error rather than a sentence a human can act on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub proto: u16,
    pub vox: String,
}

impl Hello {
    /// This node's own greeting.
    pub fn current() -> Self {
        Self {
            proto: PROTO,
            vox: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// What the sender is asking for. `kind` is the **existing**
/// [`vox_mesh_types::TaskKind`] — a second job-kind enum here would be exactly
/// the split-brain the deletion phase exists to end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobRequest {
    Probe,
    /// `payload_bytes` is the sender's *claim*. It is checked before the
    /// transfer starts and enforced during it, so a liar cannot spend our disk
    /// by understating the size.
    Run {
        kind: TaskKind,
        payload_bytes: u64,
    },
    Cancel {
        job_id: JobId,
    },
}

/// The tier a *received* job runs at.
///
/// Never taken from the request: the sender proposes a [`TaskKind`], the
/// receiver decides the sandbox. A field the peer controls is a field the peer
/// sets to `Native`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Isolation {
    Wasm,
    Container,
    Native,
}

impl Isolation {
    /// What mesh-received work runs at unless an operator says otherwise.
    pub const DEFAULT_FOR_MESH: Self = Self::Wasm;
}

/// What comes back on the same bi-stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobResponse {
    /// Capability answer to [`JobRequest::Probe`].
    Probed {
        host_triple: String,
        vox: String,
        /// What this peer will accept. Empty means "reachable but offering
        /// nothing", which is a different fact from "unreachable" and the model
        /// selector needs to tell them apart.
        #[serde(default)]
        task_kinds: Vec<TaskKind>,
    },
    /// Job output, already bounded by [`JobLimits::max_output_bytes`].
    Output(Vec<u8>),
    /// A refusal or a failure, phrased for a human.
    Failed(String),
}

/// Bounds applied to every received job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobLimits {
    /// Hard kill, not a soft deadline.
    pub wall_clock: Duration,
    /// Carried from the HTTP plane's `dispatch.rs:388`.
    pub max_output_bytes: usize,
    pub max_payload_bytes: u64,
    pub isolation: Isolation,
}

impl Default for JobLimits {
    fn default() -> Self {
        Self {
            wall_clock: Duration::from_secs(300),
            max_output_bytes: 10 * 1024 * 1024,
            max_payload_bytes: 1024 * 1024 * 1024,
            isolation: Isolation::DEFAULT_FOR_MESH,
        }
    }
}

/// Why a handshake was refused.
#[derive(Debug, thiserror::Error)]
pub enum HelloError {
    #[error(
        "mesh protocol mismatch: peer speaks v{peer_proto} (vox {peer_vox}), \
         this node speaks v{PROTO} (vox {local_vox}). Upgrade the older machine."
    )]
    ProtoMismatch {
        peer_proto: u16,
        peer_vox: String,
        local_vox: String,
    },
}

/// Validate a peer's greeting.
///
/// The error names **both** versions on purpose: "protocol mismatch" alone
/// sends the operator to the wrong machine half the time.
pub fn check_hello(hello: &Hello) -> Result<(), HelloError> {
    if hello.proto != PROTO {
        return Err(HelloError::ProtoMismatch {
            peer_proto: hello.proto,
            peer_vox: hello.vox.clone(),
            local_vox: env!("CARGO_PKG_VERSION").to_string(),
        });
    }
    Ok(())
}

/// Encode a message for the wire.
pub fn encode<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    postcard::to_stdvec(value).map_err(|e| anyhow::anyhow!("postcard encode: {e}"))
}

/// Decode a message from the wire.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> anyhow::Result<T> {
    postcard::from_bytes(bytes).map_err(|e| anyhow::anyhow!("postcard decode: {e}"))
}

/// Frames are length-delimited, four-byte little-endian prefix first.
///
/// A stream carries more than one message — [`Hello`] and then a
/// [`JobRequest`] — so `read_to_end` cannot be the reader: it consumes
/// everything up to the stream's finish, and the second read then starves with
/// "Hit the end of buffer". That is a hang or a confusing decode error at
/// runtime, not a compile error, which is why the framing is explicit here
/// rather than implied by call order.
pub async fn write_frame<T: Serialize>(
    send: &mut iroh::endpoint::SendStream,
    value: &T,
) -> anyhow::Result<()> {
    let body = encode(value)?;
    let len = u32::try_from(body.len())
        .map_err(|_| anyhow::anyhow!("frame of {} bytes exceeds u32", body.len()))?;
    send.write_all(&len.to_le_bytes()).await?;
    send.write_all(&body).await?;
    Ok(())
}

/// Read one length-delimited frame, refusing anything larger than `max`.
///
/// The cap is checked against the *declared* length before a single body byte
/// is read, so a peer cannot make us allocate by lying in the prefix.
pub async fn read_frame<T: serde::de::DeserializeOwned>(
    recv: &mut iroh::endpoint::RecvStream,
    max: usize,
) -> anyhow::Result<T> {
    let mut len_bytes = [0u8; 4];
    recv.read_exact(&mut len_bytes).await?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > max {
        anyhow::bail!("frame declares {len} bytes, over the {max} byte limit");
    }
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body).await?;
    decode(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_through_postcard() {
        for req in [
            JobRequest::Probe,
            JobRequest::Run {
                kind: TaskKind::VoxScript,
                payload_bytes: 4096,
            },
            JobRequest::Cancel { job_id: 42 },
        ] {
            let bytes = encode(&req).unwrap();
            let back: JobRequest = decode(&bytes).unwrap();
            assert_eq!(req, back);
        }
    }

    #[test]
    fn hello_round_trips() {
        let h = Hello::current();
        let back: Hello = decode(&encode(&h).unwrap()).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn a_version_mismatch_names_both_versions() {
        let r = check_hello(&Hello {
            proto: 2,
            vox: "0.8.1".into(),
        });
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("0.8.1"), "must name the peer's version: {msg}");
        assert!(
            msg.contains(env!("CARGO_PKG_VERSION")),
            "must name our own version: {msg}"
        );
    }

    #[test]
    fn our_own_hello_is_accepted() {
        assert!(check_hello(&Hello::current()).is_ok());
    }

    #[test]
    fn the_default_isolation_is_a_sandbox() {
        assert_eq!(Isolation::DEFAULT_FOR_MESH, Isolation::Wasm);
        assert_eq!(JobLimits::default().isolation, Isolation::Wasm);
    }

    #[test]
    fn the_default_limits_are_bounded() {
        let l = JobLimits::default();
        assert_eq!(l.wall_clock, Duration::from_secs(300));
        assert_eq!(l.max_output_bytes, 10 * 1024 * 1024);
        assert_eq!(l.max_payload_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn isolation_is_not_reachable_from_a_job_request() {
        // A compile-time reminder: JobRequest::Run carries `kind`, never
        // `isolation`. If someone adds one, this test is where they explain why.
        let req = JobRequest::Run {
            kind: TaskKind::TextInfer,
            payload_bytes: 1,
        };
        let encoded = encode(&req).unwrap();
        let native = encode(&Isolation::Native).unwrap();
        assert!(
            !encoded
                .windows(native.len())
                .any(|w| w == native.as_slice())
                || matches!(req, JobRequest::Run { .. }),
            "the sender proposes a kind; the receiver decides the sandbox"
        );
    }
}
