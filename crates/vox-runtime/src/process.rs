use crate::mailbox::{
    new_mailbox, Envelope, MailboxReceiver, MailboxSender, MessagePayload, Request,
};
use crate::pid::Pid;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Internal state of a running actor process.
pub struct ProcessContext {
    pub pid: Pid,
    pub name: Option<String>,
    pub mailbox_rx: MailboxReceiver,
    pub reduction_count: u64,
    pub max_reductions: u64,
}

impl ProcessContext {
    pub fn new(pid: Pid, mailbox_rx: MailboxReceiver) -> Self {
        Self {
            pid,
            name: None,
            mailbox_rx,
            reduction_count: 0,
            max_reductions: 2000, // Cooperative scheduling limit
        }
    }

    /// Receive next envelope, blocking until one arrives.
    pub async fn receive(&mut self) -> Option<Envelope> {
        self.reduction_count += 1;
        if self.reduction_count >= self.max_reductions {
            self.reduction_count = 0;
            tokio::task::yield_now().await;
        }
        self.mailbox_rx.recv().await
    }

    /// Reply to a request by sending a response through the oneshot channel.
    pub fn reply(request: Request, response: String) {
        let _ = request.reply_tx.send(response);
    }
}

/// External handle to a running actor process, used to send messages.
#[derive(Clone)]
pub struct ProcessHandle {
    pub pid: Pid,
    pub mailbox_tx: MailboxSender,
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
    /// Returns the reply string from the actor.
    pub async fn call(&self, payload: MessagePayload) -> Result<String, CallError> {
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
    #[error("Failed to send request to actor")]
    SendFailed,
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
    let (tx, rx) = new_mailbox(256);
    let ctx = ProcessContext::new(pid, rx);
    let task = tokio::spawn(behavior(ctx));

    ProcessHandle {
        pid,
        mailbox_tx: tx,
        task: Some(std::sync::Arc::new(task)),
    }
}
