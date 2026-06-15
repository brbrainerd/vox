//! [`WorkflowTracker`] backed by the generic [`vox_journal::FileJournal`] primitive.
//!
//! `FileJournalTracker` wraps a `FileJournal<JournalEntry>` and exposes the
//! `WorkflowTracker` async interface that the interpreted workflow runner
//! expects. The actual file I/O — open, append, replay, fsync, Suspendable
//! — lives in `vox-journal` so the same crash-safe substrate can be reused
//! by `vox-actor-runtime` (future) and by `vox-runtime-rn::open_file_journal`
//! across the uniffi boundary.
//!
//! ## Why split it out
//!
//! `vox-workflow-runtime` depends on `vox-db` (SQLite) for the
//! [`crate::VoxDbTracker`] implementation, and SQLite doesn't cross-compile
//! cleanly to Android. The split — generic file primitive in `vox-journal`,
//! workflow-shaped wrapper here — means the on-device runtime can use the
//! same durability substrate without dragging the host-only crates with it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use vox_config::timeouts::D_5S;
use vox_journal::{AppendDurability, FileJournal};
use vox_runtime::{
    JournalFlushStrategy, Resumable, ResumeError, RuntimeProfile, SuspendDeadline, SuspendError,
    Suspendable,
};

use crate::WorkflowTracker;

/// A single workflow journal entry persisted to disk.
///
/// `kind` is the discriminant so we can extend with new event types later
/// without breaking older files. `v` is a schema version so a reader can
/// reject lines from a future writer cleanly instead of silently dropping
/// fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum JournalEntry {
    /// Activity completed successfully and the result was recorded.
    ActivityCompleted {
        /// Journal schema version.
        v: u32,
        workflow_name: String,
        activity_name: String,
        activity_id: String,
        result: Value,
    },
    /// Workflow-version patch decision was recorded.
    WorkflowPatch {
        /// Journal schema version.
        v: u32,
        workflow_name: String,
        change_id: String,
        version: u32,
    },
}

const JOURNAL_SCHEMA_VERSION: u32 = 1;

/// Background thread that periodically `sync`s the journal to the device.
///
/// Spawned for [`JournalFlushStrategy::Periodic`] trackers (the desktop
/// profile). Stopped + joined on [`Drop`] via a condvar so shutdown is
/// prompt rather than waiting out the interval.
#[derive(Debug)]
struct PeriodicFlusher {
    stop: Arc<(Mutex<bool>, Condvar)>,
    ticks: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl PeriodicFlusher {
    fn spawn(journal: Arc<FileJournal<JournalEntry>>, interval: Duration) -> Self {
        let stop: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
        let ticks = Arc::new(AtomicU64::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_ticks = Arc::clone(&ticks);
        let handle = std::thread::Builder::new()
            .name("vox-wf-journal-flush".to_string())
            .spawn(move || {
                let (lock, cvar) = &*thread_stop;
                let mut stopped = match lock.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                while !*stopped {
                    let (guard, _timeout) = match cvar.wait_timeout(stopped, interval) {
                        Ok(r) => r,
                        Err(_) => return,
                    };
                    stopped = guard;
                    if *stopped {
                        break;
                    }
                    if let Err(e) = journal.sync() {
                        tracing::warn!("file_journal_tracker: periodic flush failed: {e}");
                    }
                    thread_ticks.fetch_add(1, Ordering::SeqCst);
                }
            })
            .ok();
        if handle.is_none() {
            tracing::warn!(
                "file_journal_tracker: could not spawn periodic flusher; relying on per-append sync"
            );
        }
        Self {
            stop,
            ticks,
            handle,
        }
    }
}

impl Drop for PeriodicFlusher {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.stop;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
        }
        cvar.notify_all();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Append-only file-backed [`WorkflowTracker`] implementation.
///
/// Flush cadence is profile-driven (spec §10): construct with
/// [`FileJournalTracker::with_profile`] to adopt the platform's
/// [`JournalFlushStrategy`]. [`FileJournalTracker::new`] keeps the historical
/// desktop behavior (every append is synced to the device before returning,
/// plus a defensive periodic flush).
#[derive(Debug)]
pub struct FileJournalTracker {
    journal: Arc<FileJournal<JournalEntry>>,
    /// `(workflow_name, activity_id) -> result`
    results: Mutex<HashMap<(String, String), Value>>,
    /// `(workflow_name, change_id) -> version`
    patches: Mutex<HashMap<(String, String), u32>>,
    flush_strategy: JournalFlushStrategy,
    flusher: Option<PeriodicFlusher>,
}

impl FileJournalTracker {
    /// Open (or create) the file journal at `path` and replay any existing
    /// entries into the in-memory index.
    ///
    /// Equivalent to `with_profile(path, RuntimeProfile::Desktop)` — the
    /// historical behavior for every existing caller.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        Self::with_profile(path, RuntimeProfile::Desktop)
    }

    /// Open the journal with the flush behavior dictated by `profile`
    /// (`profile.journal_flush_strategy()`):
    ///
    /// - [`RuntimeProfile::Desktop`] → [`JournalFlushStrategy::Periodic`]:
    ///   per-append device sync (the historical crash-safety contract, which
    ///   strictly satisfies the periodic bound) plus a defensive background
    ///   flusher at the strategy's interval.
    /// - [`RuntimeProfile::Mobile`] → [`JournalFlushStrategy::OnLifecycle`]:
    ///   appends are handed to the OS but the device sync is deferred to
    ///   [`Suspendable::suspend`], which the host calls from the iOS/Android
    ///   backgrounding hook. No background flusher thread (battery budget).
    pub fn with_profile(path: impl Into<PathBuf>, profile: RuntimeProfile) -> Result<Self> {
        Self::with_flush_strategy(path, profile.journal_flush_strategy())
    }

    /// Open the journal with an explicit [`JournalFlushStrategy`] (the
    /// policy value [`RuntimeProfile::journal_flush_strategy`] produces).
    pub fn with_flush_strategy(
        path: impl Into<PathBuf>,
        strategy: JournalFlushStrategy,
    ) -> Result<Self> {
        let durability = match strategy {
            JournalFlushStrategy::Periodic { .. } => AppendDurability::SyncEachAppend,
            JournalFlushStrategy::OnLifecycle => AppendDurability::Deferred,
        };
        let opened = FileJournal::<JournalEntry>::open_with_durability(path, durability)?;
        let mut results: HashMap<(String, String), Value> = HashMap::new();
        let mut patches: HashMap<(String, String), u32> = HashMap::new();
        for entry in opened.replayed {
            match entry {
                JournalEntry::ActivityCompleted {
                    v,
                    workflow_name,
                    activity_name: _,
                    activity_id,
                    result,
                } => {
                    if v == JOURNAL_SCHEMA_VERSION {
                        results.insert((workflow_name, activity_id), result);
                    } else {
                        tracing::warn!(
                            "file_journal_tracker: activity_completed v={v} unsupported; skipping"
                        );
                    }
                }
                JournalEntry::WorkflowPatch {
                    v,
                    workflow_name,
                    change_id,
                    version,
                } => {
                    if v == JOURNAL_SCHEMA_VERSION {
                        patches.insert((workflow_name, change_id), version);
                    } else {
                        tracing::warn!(
                            "file_journal_tracker: workflow_patch v={v} unsupported; skipping"
                        );
                    }
                }
            }
        }
        let journal = Arc::new(opened.journal);
        let flusher = match strategy {
            JournalFlushStrategy::Periodic { interval_ms } => Some(PeriodicFlusher::spawn(
                Arc::clone(&journal),
                Duration::from_millis(interval_ms.max(1)),
            )),
            JournalFlushStrategy::OnLifecycle => None,
        };
        Ok(Self {
            journal,
            results: Mutex::new(results),
            patches: Mutex::new(patches),
            flush_strategy: strategy,
            flusher,
        })
    }

    /// The path on disk where this journal is being written.
    pub fn path(&self) -> &Path {
        self.journal.path()
    }

    /// The number of recorded activity completions currently in memory.
    pub fn recorded_count(&self) -> usize {
        self.results.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// The flush policy this tracker was constructed with.
    pub fn flush_strategy(&self) -> JournalFlushStrategy {
        self.flush_strategy
    }

    /// Whether a background periodic flusher thread is running (desktop
    /// [`JournalFlushStrategy::Periodic`] profile only).
    pub fn has_periodic_flusher(&self) -> bool {
        self.flusher.as_ref().is_some_and(|f| f.handle.is_some())
    }

    /// How many times the periodic flusher has synced the journal. Always 0
    /// for [`JournalFlushStrategy::OnLifecycle`] trackers.
    pub fn periodic_flush_count(&self) -> u64 {
        self.flusher
            .as_ref()
            .map(|f| f.ticks.load(Ordering::SeqCst))
            .unwrap_or(0)
    }
}

impl WorkflowTracker for FileJournalTracker {
    fn is_activity_completed(
        &self,
        workflow_name: &str,
        activity_id: &str,
    ) -> impl std::future::Future<Output = Result<bool>> + Send {
        let key = (workflow_name.to_string(), activity_id.to_string());
        let hit = self
            .results
            .lock()
            .map(|m| m.contains_key(&key))
            .unwrap_or(false);
        async move { Ok(hit) }
    }

    fn load_activity_result(
        &self,
        workflow_name: &str,
        activity_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Value>>> + Send {
        let key = (workflow_name.to_string(), activity_id.to_string());
        let value = self.results.lock().ok().and_then(|m| m.get(&key).cloned());
        async move { Ok(value) }
    }

    fn on_activity_completed(
        &mut self,
        workflow_name: &str,
        activity_name: &str,
        activity_id: &str,
        result: &Value,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let entry = JournalEntry::ActivityCompleted {
            v: JOURNAL_SCHEMA_VERSION,
            workflow_name: workflow_name.to_string(),
            activity_name: activity_name.to_string(),
            activity_id: activity_id.to_string(),
            result: result.clone(),
        };
        let write = self.journal.append(&entry).map_err(anyhow::Error::from);
        if write.is_ok()
            && let Ok(mut m) = self.results.lock()
        {
            m.insert(
                (workflow_name.to_string(), activity_id.to_string()),
                result.clone(),
            );
        }
        async move { write }
    }

    fn load_workflow_patch(
        &self,
        workflow_name: &str,
        change_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<u32>>> + Send {
        let key = (workflow_name.to_string(), change_id.to_string());
        let value = self.patches.lock().ok().and_then(|m| m.get(&key).copied());
        async move { Ok(value) }
    }

    fn record_workflow_patch(
        &mut self,
        workflow_name: &str,
        change_id: &str,
        version: u32,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        let entry = JournalEntry::WorkflowPatch {
            v: JOURNAL_SCHEMA_VERSION,
            workflow_name: workflow_name.to_string(),
            change_id: change_id.to_string(),
            version,
        };
        let write = self.journal.append(&entry).map_err(anyhow::Error::from);
        if write.is_ok()
            && let Ok(mut m) = self.patches.lock()
        {
            m.insert((workflow_name.to_string(), change_id.to_string()), version);
        }
        async move { write }
    }
}

/// Mobile-aware suspend: delegates to the underlying [`FileJournal`]'s
/// [`Suspendable`] impl, which `sync_data`s the file so every recorded entry
/// is on the device before the OS suspends the app.
///
/// For [`JournalFlushStrategy::OnLifecycle`] (mobile) trackers this is *the*
/// durability point; for desktop trackers it is a defensive flush (appends
/// already sync). The call is synchronous and bounded by a single `fsync`,
/// comfortably inside [`SuspendDeadline::mobile_default`]. Idempotent.
impl Suspendable for FileJournalTracker {
    fn suspend(&self, deadline: SuspendDeadline) -> Result<(), SuspendError> {
        self.journal.suspend(deadline)
    }
}

/// Foregrounding hook: verify the journal handle survived the suspension.
///
/// The in-memory index is process state, so if we are still alive nothing
/// needs replaying (a killed process replays in [`FileJournalTracker::new`] /
/// [`FileJournalTracker::with_profile`] instead). What *can* break across an
/// OS suspension is the file handle itself, so `resume` performs a sync as a
/// health probe and surfaces any I/O failure to the host.
impl Resumable for FileJournalTracker {
    fn resume(&self) -> Result<(), ResumeError> {
        self.journal
            .sync()
            .map_err(|e| ResumeError::Other(format!("journal handle unhealthy after resume: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_journal_path() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("vox_wf_file_journal_{pid}_{n}.jsonl"))
    }

    #[tokio::test]
    async fn records_and_replays_a_single_completion() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        {
            let mut t = FileJournalTracker::new(&path).expect("create");
            t.on_activity_completed("wf", "act", "wf/act/1", &json!({"answer": 42}))
                .await
                .expect("record");
            assert!(t.is_activity_completed("wf", "wf/act/1").await.unwrap());
            assert_eq!(
                t.load_activity_result("wf", "wf/act/1").await.unwrap(),
                Some(json!({"answer": 42}))
            );
        }

        // Drop the tracker; re-open and verify the entry replays.
        let t2 = FileJournalTracker::new(&path).expect("re-create");
        assert!(t2.is_activity_completed("wf", "wf/act/1").await.unwrap());
        assert_eq!(
            t2.load_activity_result("wf", "wf/act/1").await.unwrap(),
            Some(json!({"answer": 42}))
        );

        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn records_multiple_activities_and_a_patch() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("create");
        for i in 0..5 {
            t.on_activity_completed("wf", "act", &format!("wf/act/{i}"), &json!({"i": i}))
                .await
                .unwrap();
        }
        t.record_workflow_patch("wf", "change-A", 3).await.unwrap();
        assert_eq!(t.recorded_count(), 5);
        assert_eq!(
            t.load_workflow_patch("wf", "change-A").await.unwrap(),
            Some(3)
        );

        drop(t);
        let t2 = FileJournalTracker::new(&path).expect("re-create");
        assert_eq!(t2.recorded_count(), 5);
        for i in 0..5 {
            assert_eq!(
                t2.load_activity_result("wf", &format!("wf/act/{i}"))
                    .await
                    .unwrap(),
                Some(json!({"i": i}))
            );
        }
        assert_eq!(
            t2.load_workflow_patch("wf", "change-A").await.unwrap(),
            Some(3)
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn default_constructor_is_desktop_profile_with_periodic_flusher() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::new(&path).expect("create");
        assert!(matches!(
            t.flush_strategy(),
            JournalFlushStrategy::Periodic { interval_ms: 5_000 }
        ));
        assert!(t.has_periodic_flusher());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn desktop_profile_has_periodic_flusher() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::with_profile(&path, RuntimeProfile::Desktop).expect("create");
        assert!(t.has_periodic_flusher());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn mobile_profile_has_no_periodic_flusher() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::with_profile(&path, RuntimeProfile::Mobile).expect("create");
        assert!(matches!(
            t.flush_strategy(),
            JournalFlushStrategy::OnLifecycle
        ));
        assert!(!t.has_periodic_flusher());
        assert_eq!(t.periodic_flush_count(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn mobile_suspend_flushes_durably_and_resume_restores_operation() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        let mut t =
            FileJournalTracker::with_profile(&path, RuntimeProfile::Mobile).expect("create");
        t.on_activity_completed("wf", "act", "wf/act/1", &json!({"answer": 42}))
            .await
            .expect("record");

        // Backgrounding: flush within the mobile deadline.
        t.suspend(SuspendDeadline::mobile_default())
            .expect("suspend");
        // Suspend must be idempotent (spec §10.3).
        t.suspend(SuspendDeadline::mobile_default())
            .expect("second suspend");

        // Foregrounding: resume restores normal operation — further records work.
        t.resume().expect("resume");
        t.on_activity_completed("wf", "act", "wf/act/2", &json!({"answer": 43}))
            .await
            .expect("record after resume");

        drop(t);
        let t2 = FileJournalTracker::with_profile(&path, RuntimeProfile::Mobile).expect("reopen");
        assert!(t2.is_activity_completed("wf", "wf/act/1").await.unwrap());
        assert!(t2.is_activity_completed("wf", "wf/act/2").await.unwrap());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn periodic_flusher_actually_ticks() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::with_flush_strategy(
            &path,
            JournalFlushStrategy::Periodic { interval_ms: 10 },
        )
        .expect("create");
        let deadline = std::time::Instant::now() + D_5S;
        while t.periodic_flush_count() == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(t.periodic_flush_count() >= 1, "flusher never ticked");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn suspend_succeeds_on_an_open_journal() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::new(&path).expect("create");
        t.suspend(SuspendDeadline::mobile_default())
            .expect("suspend");
        std::fs::remove_file(&path).ok();
    }
}
