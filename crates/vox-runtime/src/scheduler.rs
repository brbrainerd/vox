use crate::process::{ProcessHandle, ProcessContext, spawn_process};
use crate::registry::ProcessRegistry;


/// Cooperative scheduler for the Vox actor runtime.
/// Uses Tokio's work-stealing executor under the hood, with
/// reduction counting in each ProcessContext for fairness.
pub struct Scheduler {
    registry: ProcessRegistry,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            registry: ProcessRegistry::new(),
        }
    }

    /// Spawn a new actor process in the scheduler.
    pub fn spawn<F, Fut>(&self, behavior: F) -> ProcessHandle
    where
        F: FnOnce(ProcessContext) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = spawn_process(behavior);
        self.registry.register(handle.clone());
        handle
    }

    /// Spawn a named actor process.
    pub fn spawn_named<F, Fut>(&self, name: &str, behavior: F) -> ProcessHandle
    where
        F: FnOnce(ProcessContext) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = spawn_process(behavior);
        self.registry.register_name(name.to_string(), handle.clone());
        handle
    }

    /// Get the process registry.
    pub fn registry(&self) -> &ProcessRegistry {
        &self.registry
    }

    /// Number of active processes.
    pub fn process_count(&self) -> usize {
        self.registry.len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mailbox::{Envelope, Message, MessagePayload};

    #[tokio::test]
    async fn test_spawn_and_send() {
        let scheduler = Scheduler::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();

        let handle = scheduler.spawn(|mut ctx| async move {
            if let Some(Envelope::Message(msg)) = ctx.receive().await {
                if let MessagePayload::Text(text) = msg.payload {
                    let _ = tx.send(text);
                }
            }
        });

        let msg = Envelope::Message(Message {
            from: crate::pid::Pid::new(),
            payload: MessagePayload::Text("hello vox".into()),
        });
        handle.send(msg).await.unwrap();

        let result = rx.await.unwrap();
        assert_eq!(result, "hello vox");
    }

    #[tokio::test]
    async fn test_named_process() {
        let scheduler = Scheduler::new();
        let _handle = scheduler.spawn_named("echo", |mut ctx| async move {
            while let Some(env) = ctx.receive().await {
                match env {
                    Envelope::Message(_msg) => {
                        // Echo actor: just processes messages
                    }
                    _ => break,
                }
            }
        });

        let found = scheduler.registry().lookup_name("echo");
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_call_reply() {
        use crate::process::ProcessContext;

        let scheduler = Scheduler::new();

        // Spawn an echo actor that replies to requests
        let handle = scheduler.spawn(|mut ctx: ProcessContext| async move {
            while let Some(env) = ctx.receive().await {
                match env {
                    Envelope::Request(req) => {
                        if let MessagePayload::Json(json_str) = &req.payload {
                            let reply = format!("Echo: {}", json_str);
                            ProcessContext::reply(req, reply);
                        }
                    }
                    _ => {}
                }
            }
        });

        // Call the echo actor and verify reply
        let response = handle
            .call(MessagePayload::Json("hello from caller".to_string()))
            .await
            .unwrap();

        assert_eq!(response, "Echo: hello from caller");
    }
}
