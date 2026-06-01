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
use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vox_journal::FileJournal;
use vox_runtime::{SuspendDeadline, SuspendError, Suspendable};

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

/// Append-only file-backed [`WorkflowTracker`] implementation.
#[derive(Debug)]
pub struct FileJournalTracker {
    journal: FileJournal<JournalEntry>,
    /// `(workflow_name, activity_id) -> result`
    results: Mutex<HashMap<(String, String), Value>>,
    /// `(workflow_name, change_id) -> version`
    patches: Mutex<HashMap<(String, String), u32>>,
}

impl FileJournalTracker {
    /// Open (or create) the file journal at `path` and replay any existing
    /// entries into the in-memory index.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let opened = FileJournal::<JournalEntry>::open(path)?;
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
        Ok(Self {
            journal: opened.journal,
            results: Mutex::new(results),
            patches: Mutex::new(patches),
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
        if write.is_ok() {
            if let Ok(mut m) = self.results.lock() {
                m.insert(
                    (workflow_name.to_string(), activity_id.to_string()),
                    result.clone(),
                );
            }
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
        if write.is_ok() {
            if let Ok(mut m) = self.patches.lock() {
                m.insert((workflow_name.to_string(), change_id.to_string()), version);
            }
        }
        async move { write }
    }
}

/// Mobile-aware suspend: delegates to the underlying [`FileJournal`]'s
/// [`Suspendable`] impl. Today every record-call already fsyncs so this is a
/// defensive flush.
impl Suspendable for FileJournalTracker {
    fn suspend(&self, deadline: SuspendDeadline) -> Result<(), SuspendError> {
        self.journal.suspend(deadline)
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
    fn suspend_succeeds_on_an_open_journal() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::new(&path).expect("create");
        t.suspend(SuspendDeadline::mobile_default())
            .expect("suspend");
        std::fs::remove_file(&path).ok();
    }
}
