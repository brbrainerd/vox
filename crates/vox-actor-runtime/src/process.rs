use crate::mailbox::{
    DEFAULT_MAILBOX_CAPACITY, DEFAULT_MAX_REDUCTIONS, Envelope, MailboxReceiver, MailboxSender,
    MessagePayload, Request, new_mailbox,
};
use bytes::Bytes;
use crate::pid::Pid;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Internal state of a running actor process.
pub struct ProcessContext {
    /// This actor's process id.
    pub pid: Pid,
    /// Optional human-readable name registered with a supervisor or registry.
    pub name: Option<String>,
    /// Inbound mailbox for this actor.
    pub mailbox_rx: MailboxReceiver,
    /// Count of receives since last scheduler yield (cooperative fairness).
    pub reduction_count: u64,
    /// Threshold at which the actor yields to Tokio before receiving again.
    pub max_reductions: u64,
    /// Isolated memory arena localized strictly to this actor loop.
    pub heap: crate::gc::ActorHeap,
}

impl ProcessContext {
    /// Creates context for a new actor with default reduction limits.
    pub fn new(pid: Pid, mailbox_rx: MailboxReceiver) -> Self {
        Self {
            pid,
            name: None,
            mailbox_rx,
            reduction_count: 0,
            max_reductions: DEFAULT_MAX_REDUCTIONS,
            heap: crate::gc::ActorHeap::new(),
        }
    }

    /// Receive next envelope, blocking until one arrives.
    pub async fn receive(&mut self) -> Option<Envelope> {
        self.reduction_count += 1;
        if self.reduction_count >= self.max_reductions {
            self.reduction_count = 0;
            if self.heap.should_collect() {
                self.heap.collect();
            }
            tokio::task::yield_now().await;
        }
        self.mailbox_rx.recv().await
    }

    /// Reply to a request by sending a response through the oneshot channel.
    pub fn reply(request: Request, response: impl Into<Bytes>) {
        let _ = request.reply_tx.send(response.into());
    }
}

/// External handle to a running actor process, used to send messages.
#[derive(Clone)]
pub struct ProcessHandle {
    /// Target actor process id.
    pub pid: Pid,
    /// Sender half of the actor mailbox.
    pub mailbox_tx: MailboxSender,
    /// Spawned Tokio task handle when this handle owns the runtime task.
    pub task: Option<std::sync::Arc<JoinHandle<()>>>,
}

impl ProcessHandle {
    /// Send a fire-and-forget message to this process.
    pub async fn send(
        &self,
        envelope: Envelope,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<Envelope>> {
        self.mailbox_tx.send(envelope).await
    }

    /// Send a request and wait for a response (request-response pattern).
    /// Returns the reply bytes from the actor.
    pub async fn call(&self, payload: MessagePayload) -> Result<Bytes, CallError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let request = Request {
            from: Pid::new(),
            payload,
            reply_tx,
        };
        self.mailbox_tx
            .send(Envelope::Request(request))
            .await
            .map_err(|_| CallError::SendFailed)?;
        reply_rx.await.map_err(|_| CallError::NoReply)
    }

    /// Check if the underlying task is still running.
    pub fn is_alive(&self) -> bool {
        self.task.as_ref().is_some_and(|t| !t.is_finished())
    }
}

/// Errors that can occur during a `call()` request.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// Mailbox closed or full so the request envelope could not be sent.
    #[error("Failed to send request to actor")]
    SendFailed,
    /// Oneshot receiver dropped without a reply (actor exited or ignored the request).
    #[error("Actor did not reply (channel dropped)")]
    NoReply,
}

/// Spawn a new actor process with the given behavior function.
/// Returns a ProcessHandle for communication.
pub fn spawn_process<F, Fut>(behavior: F) -> ProcessHandle
where
    F: FnOnce(ProcessContext) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let pid = Pid::new();
    let (tx, rx) = new_mailbox(DEFAULT_MAILBOX_CAPACITY);
    let ctx = ProcessContext::new(pid, rx);
    let task = tokio::spawn(behavior(ctx));

    ProcessHandle {
        pid,
        mailbox_tx: tx,
        task: Some(std::sync::Arc::new(task)),
    }
}
