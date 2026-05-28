use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, timeout};

/// A simple waitable barrier to replace `sleep` in tests.
#[derive(Clone, Default)]
pub struct TestBarrier {
    notify: Arc<Notify>,
}

impl TestBarrier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals that an event occurred.
    pub fn signal(&self) {
        self.notify.notify_waiters();
    }

    /// Waits for a signal, with a default 5-second timeout to prevent
    /// tests from hanging indefinitely in CI.
    pub async fn wait(&self) -> bool {
        self.wait_with_timeout(vox_config::timeouts::D_5S).await
    }

    /// Waits for a signal with a specific timeout.
    pub async fn wait_with_timeout(&self, dur: Duration) -> bool {
        timeout(dur, self.notify.notified()).await.is_ok()
    }
}
