use crate::process::{ProcessContext, ProcessHandle, spawn_process};
use crate::registry::{ProcessRegistry, RegistryError};

/// Cooperative scheduler for the Vox actor runtime.
/// Uses Tokio's work-stealing executor under the hood, with
/// reduction counting in each ProcessContext for fairness.
pub struct Scheduler {
    registry: ProcessRegistry,
}

impl Scheduler {
    /// Creates a scheduler with an empty [`ProcessRegistry`].
    pub fn new() -> Self {
        Self {
            registry: ProcessRegistry::new(),
        }
    }

    /// Spawn a new actor process in the scheduler.
    pub fn spawn<F, Fut>(&self, behavior: F) -> Result<ProcessHandle, RegistryError>
    where
        F: FnOnce(ProcessContext) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = spawn_process(behavior);
        self.registry.register(handle.clone())?;
        Ok(handle)
    }

    /// Spawn a named actor process.
    pub fn spawn_named<F, Fut>(
        &self,
        name: &str,
        behavior: F,
    ) -> Result<ProcessHandle, RegistryError>
    where
        F: FnOnce(ProcessContext) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = spawn_process(behavior);
        self.registry
            .register_name(name.to_string(), handle.clone())?;
        Ok(handle)
    }

    /// Get the process registry.
    pub fn registry(&self) -> &ProcessRegistry {
        &self.registry
    }

    /// Number of active processes.
    pub fn process_count(&self) -> Result<usize, RegistryError> {
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
    async fn test_spawn_and_send() -> Result<(), RegistryError> {
        let scheduler = Scheduler::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<bytes::Bytes>();

        let handle = scheduler.spawn(|mut ctx| async move {
            if let Some(Envelope::Message(msg)) = ctx.receive().await {
                if let MessagePayload::Text(text) = msg.payload {
                    let _ = tx.send(text);
                }
            }
        })?;

        let msg = Envelope::Message(Message {
            from: crate::pid::Pid::new(),
            payload: MessagePayload::text("hello vox"),
        });
        handle.send(msg).await.unwrap();

        let result = rx.await.unwrap();
        assert_eq!(result.as_ref(), b"hello vox");
        Ok(())
    }

    #[tokio::test]
    async fn test_named_process() -> Result<(), RegistryError> {
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
        })?;

        let found = scheduler.registry().lookup_name("echo")?;
        assert!(found.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_call_reply() -> Result<(), RegistryError> {
        use crate::process::ProcessContext;

        let scheduler = Scheduler::new();

        // Spawn an echo actor that replies to requests
        let handle = scheduler.spawn(|mut ctx: ProcessContext| async move {
            while let Some(env) = ctx.receive().await {
                if let Envelope::Request(req) = env {
                    if let MessagePayload::Json(json_bytes) = &req.payload {
                        let text = std::str::from_utf8(json_bytes).unwrap();
                        let reply = format!("Echo: {}", text);
                        ProcessContext::reply(req, reply);
                    }
                }
            }
        })?;

        // Call the echo actor and verify reply
        let response = handle
            .call(MessagePayload::json_str("hello from caller"))
            .await
            .unwrap();

        assert_eq!(response.as_ref(), b"Echo: hello from caller");
        Ok(())
    }
}
