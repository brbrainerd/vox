use crate::pid::Pid;
use tokio::sync::{mpsc, oneshot};

/// A message envelope wrapping user messages, requests, and system signals.
///
/// `Envelope` is NOT Clone because `Request` contains a oneshot sender.
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
    pub from: Pid,
    pub payload: MessagePayload,
}

/// A request expecting a response, carrying a oneshot reply channel.
#[derive(Debug)]
pub struct Request {
    pub from: Pid,
    pub payload: MessagePayload,
    pub reply_tx: oneshot::Sender<String>,
}

/// Dynamic message payload (typed via serde-like serialization in practice).
#[derive(Debug, Clone)]
pub enum MessagePayload {
    Text(String),
    Json(String), // JSON-serialized data
    Binary(Vec<u8>),
}

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
    Normal,
    Shutdown,
    Error(String),
}

/// A handle to send messages to a process's mailbox.
pub type MailboxSender = mpsc::Sender<Envelope>;
/// The receiving end of a process's mailbox.
pub type MailboxReceiver = mpsc::Receiver<Envelope>;

/// Create a new mailbox with the given buffer capacity.
pub fn new_mailbox(capacity: usize) -> (MailboxSender, MailboxReceiver) {
    mpsc::channel(capacity)
}
