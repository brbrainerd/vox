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

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn wait_with_timeout_returns_true_on_signal() {
        let barrier = TestBarrier::new();
        let b2 = barrier.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            b2.signal();
        });
        let result = barrier.wait_with_timeout(Duration::from_millis(500)).await;
        handle.await.unwrap();
        assert!(
            result,
            "should return true when signal arrives within timeout"
        );
    }

    #[tokio::test]
    async fn wait_with_timeout_returns_false_on_timeout() {
        let barrier = TestBarrier::new();
        let result = barrier.wait_with_timeout(Duration::from_millis(10)).await;
        assert!(!result, "should return false when no signal arrives");
    }

    #[tokio::test]
    async fn wait_with_timeout_true_when_signal_arrives_in_time() {
        let barrier = TestBarrier::new();
        let b2 = barrier.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            b2.signal();
        });
        let result = barrier.wait_with_timeout(Duration::from_millis(500)).await;
        assert!(result);
    }

    #[tokio::test]
    async fn wait_with_timeout_false_when_no_signal() {
        let barrier = TestBarrier::new();
        let result = barrier.wait_with_timeout(Duration::from_millis(10)).await;
        assert!(!result);
    }
}
