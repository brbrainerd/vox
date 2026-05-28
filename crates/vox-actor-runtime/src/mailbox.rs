use bytes::Bytes;
use crate::pid::Pid;
use tokio::sync::{mpsc, oneshot};

// ---------------------------------------------------------------------------
// Tuning constants (single place to change, no magic numbers scattered below)
// ---------------------------------------------------------------------------

/// Default mailbox channel buffer depth. Back-pressure will apply when full.
pub const DEFAULT_MAILBOX_CAPACITY: usize = 256;

/// Maximum number of message receptions before an actor cooperatively yields
/// to the Tokio scheduler. Matches the BEAM default heuristic.
pub const DEFAULT_MAX_REDUCTIONS: u64 = 2_000;

// ---------------------------------------------------------------------------
// Payload
// ---------------------------------------------------------------------------

/// Zero-copy message payload backed by [`bytes::Bytes`].
///
/// `Bytes` is an atomically reference-counted view over a shared heap buffer,
/// so cloning it is O(1) and never copies the underlying data.
///
/// # Layout
///
/// | Variant  | Encoding                        |
/// |----------|---------------------------------|
/// | `Text`   | UTF-8, no copy on forward/clone |
/// | `Json`   | JSON bytes, no copy             |
/// | `Binary` | Arbitrary bytes, no copy        |
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// UTF-8 text payload (zero-copy clone).
    Text(Bytes),
    /// JSON-serialised payload (zero-copy clone).
    Json(Bytes),
    /// Opaque binary payload (zero-copy clone).
    Binary(Bytes),
}

impl MessagePayload {
    /// Construct a `Text` payload from a `String` (single allocation, then shared).
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(Bytes::from(s.into().into_bytes()))
    }

    /// Construct a `Json` payload by serialising `value` (single allocation, then shared).
    pub fn json_value(value: &serde_json::Value) -> Self {
        Self::Json(Bytes::from(
            serde_json::to_vec(value).unwrap_or_default(),
        ))
    }

    /// Construct a `Json` payload from a pre-serialised string.
    pub fn json_str(s: impl Into<String>) -> Self {
        Self::Json(Bytes::from(s.into().into_bytes()))
    }

    /// Construct a `Binary` payload (zero additional copy if you already have `Bytes`).
    pub fn binary(b: impl Into<Bytes>) -> Self {
        Self::Binary(b.into())
    }

    /// Return the raw bytes regardless of variant.
    pub fn as_bytes(&self) -> &Bytes {
        match self {
            Self::Text(b) | Self::Json(b) | Self::Binary(b) => b,
        }
    }

    /// Attempt to interpret the payload as a UTF-8 `&str`.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_bytes()).ok()
    }

    /// Deserialise the payload as JSON into `T`.
    pub fn deserialize_json<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(self.as_bytes())
    }
}

// ---------------------------------------------------------------------------
// Envelope and sub-types
// ---------------------------------------------------------------------------

/// A message envelope wrapping user messages, requests, and system signals.
///
/// `Envelope` is NOT `Clone` because `Request` contains a one-shot sender.
/// Use `Message` or `Signal` variants where cloning is needed.
#[derive(Debug)]
pub enum Envelope {
    /// Fire-and-forget message payload.
    Message(Message),
    /// Request-response: caller sends a message and waits for a reply.
    Request(Request),
    /// System signal (link, monitor, exit).
    Signal(Signal),
}

/// An application-level message sent between actors.
#[derive(Debug, Clone)]
pub struct Message {
    /// Sender process id.
    pub from: Pid,
    /// Application payload (zero-copy).
    pub payload: MessagePayload,
}

/// A request expecting a response, carrying a one-shot reply channel.
#[derive(Debug)]
pub struct Request {
    /// Caller process id.
    pub from: Pid,
    /// Request body (zero-copy).
    pub payload: MessagePayload,
    /// Channel the callee uses to send the reply.
    pub reply_tx: oneshot::Sender<Bytes>,
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

/// System signals for process lifecycle management.
#[derive(Debug, Clone)]
pub enum Signal {
    /// Process exited normally.
    Exit(Pid, ExitReason),
    /// Link request between two processes.
    Link(Pid),
    /// Unlink request.
    Unlink(Pid),
    /// Monitor notification.
    Down(Pid, ExitReason),
}

/// Reason a process exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    /// Clean exit without error.
    Normal,
    /// Cooperative shutdown requested.
    Shutdown,
    /// Actor failed with a message.
    Error(String),
}

// ---------------------------------------------------------------------------
// Mailbox channel type aliases
// ---------------------------------------------------------------------------

/// A handle to send messages to a process's mailbox.
pub type MailboxSender = mpsc::Sender<Envelope>;
/// The receiving end of a process's mailbox.
pub type MailboxReceiver = mpsc::Receiver<Envelope>;

/// Create a new mailbox with the given buffer capacity.
pub fn new_mailbox(capacity: usize) -> (MailboxSender, MailboxReceiver) {
    mpsc::channel(capacity)
}

// ---------------------------------------------------------------------------
// DeepClone boundary marker trait
// ---------------------------------------------------------------------------

/// Trait used to sever GC boundaries when passing messages between actors.
///
/// The compiler derives this for custom types to deep-copy values outside
/// the GC arena before they cross a mailbox boundary. For `Bytes`-backed
/// payloads the clone is O(1); this trait remains for structured Vox types.
pub trait DeepCloneToOwned {
    type Owned;
    fn deep_clone_to_owned(&self) -> Self::Owned;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_clone_is_zero_copy() {
        let payload = MessagePayload::text("hello world");
        let clone = payload.clone();
        // Both variants should point to the same underlying buffer (same ptr).
        assert_eq!(
            payload.as_bytes().as_ptr(),
            clone.as_bytes().as_ptr(),
            "clone of Bytes payload should share the same allocation"
        );
    }

    #[test]
    fn payload_json_roundtrip() {
        let v = serde_json::json!({ "key": 42 });
        let payload = MessagePayload::json_value(&v);
        let decoded: serde_json::Value = payload.deserialize_json().unwrap();
        assert_eq!(decoded["key"], 42);
    }

    #[test]
    fn constants_are_positive() {
        assert!(DEFAULT_MAILBOX_CAPACITY > 0);
        assert!(DEFAULT_MAX_REDUCTIONS > 0);
    }
}
