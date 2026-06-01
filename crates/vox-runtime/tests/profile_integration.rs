//! Integration test exercising the cross-module invariants of `vox-runtime`.
//!
//! These run as a separate crate from the unit tests inside `src/` so the
//! public API surface is exercised exactly the way downstream crates
//! (`vox-workflow-runtime`, `vox-actor-runtime`, the uniffi `vox-runtime-rn`
//! shim) will consume it.

use std::path::PathBuf;
use std::time::Duration;

use vox_runtime::{
    JournalFlushStrategy, ModelLoadingStrategy, Resumable, ResumeError, RuntimeProfile,
    SuspendDeadline, SuspendError, Suspendable, VoxConfig,
};

#[test]
fn desktop_config_has_eager_models_and_periodic_journal() {
    let cfg = VoxConfig::desktop();
    assert_eq!(cfg.profile, RuntimeProfile::Desktop);
    let p = cfg.profile;
    assert!(matches!(
        p.journal_flush_strategy(),
        JournalFlushStrategy::Periodic { interval_ms: 5_000 }
    ));
    assert_eq!(p.model_loading_strategy(), ModelLoadingStrategy::Eager);
    assert!(!p.requires_suspend_hooks());
}

#[test]
fn mobile_config_has_lazy_models_and_on_lifecycle_journal() {
    let cfg = VoxConfig::mobile(PathBuf::from("/tmp/vox-mobile-it"));
    assert_eq!(cfg.profile, RuntimeProfile::Mobile);
    let p = cfg.profile;
    assert!(matches!(
        p.journal_flush_strategy(),
        JournalFlushStrategy::OnLifecycle
    ));
    assert!(matches!(
        p.model_loading_strategy(),
        ModelLoadingStrategy::Lazy {
            unload_on_memory_pressure: true
        }
    ));
    assert!(p.requires_suspend_hooks());
}

/// End-to-end usage shape: a downstream subsystem implements `Suspendable`
/// and `Resumable`, the runtime drives the lifecycle, and state survives
/// across the suspend/resume boundary.
#[test]
fn subsystem_round_trips_state_through_suspend_resume() {
    use std::sync::Mutex;

    struct InMemoryJournal {
        entries: Mutex<Vec<String>>,
        flushed: Mutex<Vec<String>>,
    }

    impl Suspendable for InMemoryJournal {
        fn suspend(&self, _deadline: SuspendDeadline) -> Result<(), SuspendError> {
            let entries = self.entries.lock().unwrap();
            let mut flushed = self.flushed.lock().unwrap();
            *flushed = entries.clone();
            Ok(())
        }
    }

    impl Resumable for InMemoryJournal {
        fn resume(&self) -> Result<(), ResumeError> {
            let flushed = self.flushed.lock().unwrap();
            let mut entries = self.entries.lock().unwrap();
            *entries = flushed.clone();
            Ok(())
        }
    }

    let j = InMemoryJournal {
        entries: Mutex::new(vec!["entry_1".into(), "entry_2".into()]),
        flushed: Mutex::new(Vec::new()),
    };

    // Pre-suspend.
    assert_eq!(j.entries.lock().unwrap().len(), 2);
    assert!(j.flushed.lock().unwrap().is_empty());

    // App backgrounded — runtime calls suspend.
    j.suspend(SuspendDeadline::mobile_default()).unwrap();
    assert_eq!(j.flushed.lock().unwrap().len(), 2);

    // Simulate process kill: in-memory entries lost.
    j.entries.lock().unwrap().clear();

    // App reopened — runtime calls resume.
    j.resume().unwrap();
    assert_eq!(j.entries.lock().unwrap().len(), 2);
    assert_eq!(j.entries.lock().unwrap()[0], "entry_1");
}

#[test]
fn suspend_deadlines_distinguish_strict_from_advisory() {
    let strict = SuspendDeadline::mobile_default();
    let advisory = SuspendDeadline::desktop_default();
    assert!(matches!(strict, SuspendDeadline::Strict { .. }));
    assert!(matches!(advisory, SuspendDeadline::Advisory { .. }));
    // Advisory desktop deadline should be larger — desktop isn't racing the OS.
    assert!(advisory.duration() > strict.duration());
}

#[test]
fn suspend_error_includes_elapsed_duration() {
    let e = SuspendError::Timeout {
        elapsed: Duration::from_secs(7),
    };
    let msg = format!("{e}");
    assert!(
        msg.contains("7"),
        "expected elapsed seconds in message, got: {msg}"
    );
}
