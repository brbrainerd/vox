//! `DurablePromise<T>` — the single awaitable primitive for distributed durable work.
//!
//! Subsumes `Future[T]`, `Promise[T]`, the activity-result handle, signal awaits,
//! and awakeables. Lowered from Vox `DurablePromise[T]` by `vox-codegen`.
//!
//! Semantics:
//! - **Live execution**: the workflow runtime registers the `activity_id` and returns a
//!   `DurablePromise<T>` whose `.await` suspends the workflow until the activity completes.
//!   The result is journaled on completion.
//! - **Replay**: the runtime sees the `activity_id` is already completed and resolves the
//!   promise from the journal *without* re-issuing the dispatch (journal-backed fast path).

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::oneshot;

/// Error variants for a durable activity execution.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// The activity executed but returned an error.
    #[error("activity {activity_id} failed: {source}")]
    ActivityFailed {
        /// Stable activity identifier.
        activity_id: String,
        /// Underlying error from the activity.
        #[source]
        source: anyhow::Error,
    },
    /// The journal record for this activity is unreadable or corrupt.
    #[error("journal corruption for activity {0}: {1}")]
    JournalCorrupt(String, String),
    /// The parent workflow was cancelled before the activity completed.
    #[error("workflow cancelled before activity {0} completed")]
    Cancelled(String),
    /// The oneshot sender was dropped without sending a result.
    #[error("oneshot sender dropped without resolving {0}")]
    SenderDropped(String),
}

/// The single awaitable primitive. Use `.await` in Vox / `.poll` in Rust.
///
/// Construction is private to the runtime — user code receives these as the
/// result of a `@remote` call, an activity dispatch, a `signal()`, or a
/// `side_effect { … }` block. The `activity_id` uniquely identifies the work
/// item in the durable journal.
pub struct DurablePromise<T> {
    activity_id: String,
    state: PromiseState<T>,
}

enum PromiseState<T> {
    /// Live execution: result will arrive via the oneshot channel.
    /// Only constructed by `DurablePromise::pending`, which is currently
    /// exercised by tests; the live dispatch wiring is staged.
    #[cfg_attr(not(test), allow(dead_code))]
    Pending(oneshot::Receiver<Result<T, JournalError>>),
    /// Replay: result was loaded from the journal immediately.
    Replayed(Result<T, JournalError>),
    /// Already polled to completion; polling again panics.
    Done,
}

impl<T> DurablePromise<T> {
    /// Mint a promise for a live activity dispatch — resolves when the sender fires.
    /// Currently only used by in-crate tests; the live dispatch path will call
    /// this when activity dispatch is wired through the runtime.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn pending(
        activity_id: String,
        rx: oneshot::Receiver<Result<T, JournalError>>,
    ) -> Self {
        Self {
            activity_id,
            state: PromiseState::Pending(rx),
        }
    }

    /// Mint a promise that resolves immediately from a journal entry (replay path).
    pub(crate) fn replayed(activity_id: String, value: Result<T, JournalError>) -> Self {
        Self {
            activity_id,
            state: PromiseState::Replayed(value),
        }
    }

    /// The stable `activity_id` that identifies this unit of work in the journal.
    pub fn activity_id(&self) -> &str {
        &self.activity_id
    }
}

impl<T> std::future::Future for DurablePromise<T>
where
    T: Unpin,
{
    type Output = Result<T, JournalError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        match std::mem::replace(&mut me.state, PromiseState::Done) {
            PromiseState::Pending(mut rx) => match Pin::new(&mut rx).poll(cx) {
                Poll::Ready(Ok(v)) => Poll::Ready(v),
                Poll::Ready(Err(_)) => {
                    Poll::Ready(Err(JournalError::SenderDropped(me.activity_id.clone())))
                }
                Poll::Pending => {
                    me.state = PromiseState::Pending(rx);
                    Poll::Pending
                }
            },
            PromiseState::Replayed(v) => Poll::Ready(v),
            PromiseState::Done => panic!(
                "DurablePromise<{}> for activity '{}' polled after completion",
                std::any::type_name::<T>(),
                me.activity_id,
            ),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for DurablePromise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DurablePromise")
            .field("activity_id", &self.activity_id)
            .finish()
    }
}

/// Deserialise a completed activity result from journal bytes (codegen path).
pub fn from_serialised<T: DeserializeOwned>(
    activity_id: String,
    bytes: &[u8],
) -> Result<DurablePromise<T>, JournalError> {
    match serde_json::from_slice::<T>(bytes) {
        Ok(v) => Ok(DurablePromise::replayed(activity_id, Ok(v))),
        Err(e) => Err(JournalError::JournalCorrupt(activity_id, e.to_string())),
    }
}

/// Build a promise that resolves immediately with a known value (tests / codegen stubs).
#[doc(hidden)]
pub fn ready<T: Serialize + Send + 'static>(activity_id: String, v: T) -> DurablePromise<T> {
    DurablePromise::replayed(activity_id, Ok(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replayed_resolves_synchronously() {
        let p: DurablePromise<i32> = DurablePromise::replayed("act1".into(), Ok(42));
        assert_eq!(p.activity_id(), "act1");
        assert_eq!(p.await.unwrap(), 42);
    }

    #[tokio::test]
    async fn pending_resolves_when_sender_completes() {
        let (tx, rx) = oneshot::channel();
        let p: DurablePromise<i32> = DurablePromise::pending("act2".into(), rx);
        let handle = tokio::spawn(p);
        tx.send(Ok(7)).unwrap();
        assert_eq!(handle.await.unwrap().unwrap(), 7);
    }

    #[tokio::test]
    async fn pending_propagates_journal_failure() {
        let (tx, rx) = oneshot::channel();
        let p: DurablePromise<i32> = DurablePromise::pending("act3".into(), rx);
        tx.send(Err(JournalError::ActivityFailed {
            activity_id: "act3".into(),
            source: anyhow::anyhow!("dispatch refused"),
        }))
        .unwrap();
        let err = p.await.unwrap_err();
        assert!(matches!(err, JournalError::ActivityFailed { .. }));
    }

    #[tokio::test]
    async fn dropped_sender_yields_sender_dropped() {
        let (tx, rx) = oneshot::channel::<Result<i32, JournalError>>();
        drop(tx);
        let p: DurablePromise<i32> = DurablePromise::pending("act4".into(), rx);
        let err = p.await.unwrap_err();
        assert!(matches!(err, JournalError::SenderDropped(_)));
    }
}
