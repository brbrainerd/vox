//! Lifecycle hooks bridging the OS suspend/resume cycle into Vox subsystems.
//!
//! Mobile platforms suspend backgrounded apps aggressively: iOS gives a
//! grace period of ~30 seconds after `applicationWillResignActive` before
//! the OS may kill the process; Android's `onPause` is similar in spirit.
//! Subsystems that ignore these events lose un-flushed state.
//!
//! The [`Suspendable`] trait lets every actor / workflow / inference loop
//! participate in the suspend flow with a single method call. The runtime
//! invokes `suspend()` in lifecycle order (most recently spawned first),
//! gives each subsystem a [`SuspendDeadline`] worth of time, and proceeds
//! whether or not a subsystem signaled completion within that window.
//!
//! Desktop subsystems can opt out (their profile won't call `suspend()`).

use std::time::Duration;

use thiserror::Error;

/// Default time iOS gives an app after `applicationWillResignActive` before
/// it may kill the process. Subsystems that take longer than this risk
/// losing state.
pub const IOS_SUSPEND_GRACE: Duration = Duration::from_secs(30);

/// Default safety budget the runtime uses when calling `suspend()`. Picked
/// below the OS grace period so we have buffer for the final flush + lock.
pub const DEFAULT_SUSPEND_DEADLINE: Duration = Duration::from_secs(5);

/// How long a suspendable subsystem has to complete its `suspend()` call.
///
/// `Strict` deadlines force the runtime to abandon any subsystem that
/// hasn't returned in time; `Advisory` deadlines log a warning and continue
/// waiting (used in tests + on desktop where the OS isn't about to kill us).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendDeadline {
    /// Wait at most `duration`; cancel slow subsystems.
    Strict {
        /// Max time to wait per subsystem.
        duration: Duration,
    },
    /// Log a warning if `duration` is exceeded but keep waiting.
    Advisory {
        /// Warn-threshold time.
        duration: Duration,
    },
}

impl SuspendDeadline {
    /// Default strict deadline for mobile profiles.
    pub fn mobile_default() -> Self {
        Self::Strict {
            duration: DEFAULT_SUSPEND_DEADLINE,
        }
    }

    /// Default advisory deadline for desktop profiles.
    pub fn desktop_default() -> Self {
        Self::Advisory {
            duration: Duration::from_secs(60),
        }
    }

    /// Get the inner [`Duration`] regardless of variant.
    pub fn duration(self) -> Duration {
        match self {
            Self::Strict { duration } | Self::Advisory { duration } => duration,
        }
    }
}

/// Error raised by a subsystem's `suspend()` implementation.
#[derive(Debug, Error)]
pub enum SuspendError {
    /// The subsystem couldn't flush all state within the deadline.
    #[error("subsystem did not complete suspend within the deadline ({elapsed:?})")]
    Timeout {
        /// How long the suspend call took before the timeout fired.
        elapsed: Duration,
    },
    /// A flush operation failed.
    #[error("flush failed during suspend: {message}")]
    FlushFailed {
        /// Human-readable failure description.
        message: String,
    },
    /// Caller-provided context; subsystems may use this for I/O errors etc.
    #[error("{0}")]
    Other(String),
}

/// Error raised by a subsystem's `resume()` implementation.
#[derive(Debug, Error)]
pub enum ResumeError {
    /// The persisted journal could not be replayed.
    #[error("journal replay failed: {message}")]
    ReplayFailed {
        /// Human-readable description.
        message: String,
    },
    /// Caller-provided context.
    #[error("{0}")]
    Other(String),
}

/// A subsystem that can be suspended in response to an OS lifecycle event.
///
/// Implementors are expected to:
///
/// 1. Flush any in-memory state to durable storage (journal, snapshot, etc.).
/// 2. Cancel any background tasks that hold non-snapshottable handles.
/// 3. Return promptly — the runtime calls every subsystem's `suspend()`
///    in sequence, and the OS grace period is shared across all of them.
pub trait Suspendable {
    /// Flush in-memory state and prepare for the runtime to be paused.
    ///
    /// The `deadline` is advisory: the runtime *might* abandon you if you
    /// exceed it, but you should still respect it as a soft contract.
    fn suspend(&self, deadline: SuspendDeadline) -> Result<(), SuspendError>;
}

/// A subsystem that can resume after a prior suspend.
///
/// Resume is the inverse of [`Suspendable::suspend`]: the runtime calls
/// `resume()` once the app returns to the foreground, and the implementor
/// replays the journal / re-mounts handles / restarts background tasks.
pub trait Resumable {
    /// Restore state from the durable store and prepare to accept work.
    fn resume(&self) -> Result<(), ResumeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ios_grace_is_thirty_seconds() {
        assert_eq!(IOS_SUSPEND_GRACE, Duration::from_secs(30));
    }

    #[test]
    fn default_suspend_deadline_is_below_grace() {
        assert!(DEFAULT_SUSPEND_DEADLINE < IOS_SUSPEND_GRACE);
    }

    #[test]
    fn mobile_default_is_strict() {
        let d = SuspendDeadline::mobile_default();
        assert!(matches!(d, SuspendDeadline::Strict { .. }));
        assert_eq!(d.duration(), DEFAULT_SUSPEND_DEADLINE);
    }

    #[test]
    fn desktop_default_is_advisory() {
        let d = SuspendDeadline::desktop_default();
        assert!(matches!(d, SuspendDeadline::Advisory { .. }));
    }

    #[test]
    fn suspend_error_displays_helpfully() {
        let e = SuspendError::Timeout {
            elapsed: Duration::from_secs(7),
        };
        let msg = format!("{e}");
        assert!(msg.contains("7"), "got: {msg}");
        assert!(msg.contains("deadline"), "got: {msg}");
    }

    /// Sanity check: a simple in-memory subsystem can implement Suspendable.
    /// Acts as a usage example for downstream crates.
    #[test]
    fn suspendable_can_be_implemented() {
        struct Counter {
            value: std::cell::Cell<u32>,
            flushed: std::cell::Cell<u32>,
        }
        impl Suspendable for Counter {
            fn suspend(&self, _deadline: SuspendDeadline) -> Result<(), SuspendError> {
                self.flushed.set(self.value.get());
                Ok(())
            }
        }
        impl Resumable for Counter {
            fn resume(&self) -> Result<(), ResumeError> {
                self.value.set(self.flushed.get());
                Ok(())
            }
        }
        let c = Counter {
            value: std::cell::Cell::new(42),
            flushed: std::cell::Cell::new(0),
        };
        c.suspend(SuspendDeadline::mobile_default()).unwrap();
        assert_eq!(c.flushed.get(), 42);
        c.value.set(99);
        c.resume().unwrap();
        assert_eq!(c.value.get(), 42);
    }
}
