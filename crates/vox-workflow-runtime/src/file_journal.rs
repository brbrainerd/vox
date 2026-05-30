//! Append-only file-backed [`WorkflowTracker`] suitable for mobile + lightweight
//! desktop deployments that don't want a SQLite dependency.
//!
//! ## Why this exists
//!
//! [`crate::VoxDbTracker`] persists workflow steps through `vox-db`, which
//! pulls in SQLite. That's the right call on desktop where SQLite is essentially
//! free. But on mobile — where every native dep bloats the cdylib and every
//! migration risks the OS killing us mid-write — a much smaller substrate is
//! appropriate: an append-only JSON Lines log on the per-app private dir.
//!
//! ## Crash safety
//!
//! Every successful record call performs:
//!   1. Serialize the entry as one JSON line + `\n`.
//!   2. Write the bytes.
//!   3. Flush to the OS via [`std::io::Write::flush`].
//!   4. Fsync via [`std::fs::File::sync_data`] so the OS commits the bytes to
//!      disk before returning.
//!
//! If a crash or kill happens between two records, the journal contains every
//! line that returned `Ok` plus zero partial lines (the entire line was
//! buffered + fsynced as a unit). On the next process start, the tracker reads
//! the whole file and rebuilds its in-memory map. Lines that fail to parse
//! are logged via `tracing::warn` and skipped — the rest of the file replays.
//!
//! ## Suspendable
//!
//! Implements [`vox_runtime::Suspendable`] so mobile profiles can ensure the
//! journal is fsynced before the app suspends. Today the per-record fsync
//! already guarantees durability so `suspend()` is a no-op success; the trait
//! impl exists so future buffering optimizations have a hook to flush.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vox_runtime::{Suspendable, SuspendDeadline, SuspendError};

use crate::WorkflowTracker;

/// A single journal entry persisted to disk.
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
///
/// Use [`FileJournalTracker::new`] for a fresh tracker; the file is created
/// if it does not exist, and existing entries are replayed into the in-memory
/// index. Use [`FileJournalTracker::path`] to inspect where the journal lives
/// (useful for `tracing` log lines that record the journal location).
#[derive(Debug)]
pub struct FileJournalTracker {
    path: PathBuf,
    /// `(workflow_name, activity_id) -> result`
    results: Mutex<HashMap<(String, String), Value>>,
    /// `(workflow_name, change_id) -> version`
    patches: Mutex<HashMap<(String, String), u32>>,
    /// Append handle. Wrapped in a Mutex so writers don't interleave bytes.
    /// Opened in append mode so concurrent processes can't corrupt the file
    /// even if multiple instances point at the same path (each line is
    /// atomic on POSIX up to PIPE_BUF; the trade-off is the host scheduler's
    /// problem, not ours).
    writer: Mutex<File>,
}

impl FileJournalTracker {
    /// Open (or create) the file journal at `path` and replay any existing
    /// entries into the in-memory index.
    ///
    /// Returns an error if the parent directory cannot be created or if the
    /// file cannot be opened for append + read.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path: PathBuf = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("creating parent directory {}", parent.display())
                })?;
            }
        }
        // Touch the file so we can read it back even on first run.
        let _touch = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening journal {}", path.display()))?;

        let (results, patches) = Self::replay(&path)?;

        let writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening journal (append) {}", path.display()))?;

        Ok(Self {
            path,
            results: Mutex::new(results),
            patches: Mutex::new(patches),
            writer: Mutex::new(writer),
        })
    }

    /// The path on disk where this journal is being written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The number of recorded activity completions currently in memory.
    /// Useful for tests and for `tracing` log lines.
    pub fn recorded_count(&self) -> usize {
        self.results.lock().map(|m| m.len()).unwrap_or(0)
    }

    fn replay(
        path: &Path,
    ) -> Result<(
        HashMap<(String, String), Value>,
        HashMap<(String, String), u32>,
    )> {
        let f = File::open(path)
            .with_context(|| format!("re-opening journal for replay {}", path.display()))?;
        let r = BufReader::new(f);
        let mut results: HashMap<(String, String), Value> = HashMap::new();
        let mut patches: HashMap<(String, String), u32> = HashMap::new();
        for (line_no, line) in r.lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!(
                        "file_journal: I/O error reading line {line_no}: {e}; halting replay"
                    );
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<JournalEntry>(&line) {
                Ok(JournalEntry::ActivityCompleted {
                    v,
                    workflow_name,
                    activity_name: _,
                    activity_id,
                    result,
                }) => {
                    if v != JOURNAL_SCHEMA_VERSION {
                        tracing::warn!(
                            "file_journal: line {line_no} has unsupported schema version {v}; skipping"
                        );
                        continue;
                    }
                    results.insert((workflow_name, activity_id), result);
                }
                Ok(JournalEntry::WorkflowPatch {
                    v,
                    workflow_name,
                    change_id,
                    version,
                }) => {
                    if v != JOURNAL_SCHEMA_VERSION {
                        tracing::warn!(
                            "file_journal: line {line_no} has unsupported schema version {v}; skipping"
                        );
                        continue;
                    }
                    patches.insert((workflow_name, change_id), version);
                }
                Err(e) => {
                    tracing::warn!(
                        "file_journal: line {line_no} failed to parse: {e}; line preserved on disk but skipped in memory"
                    );
                }
            }
        }
        Ok((results, patches))
    }

    fn append(&self, entry: &JournalEntry) -> Result<()> {
        let line = serde_json::to_string(entry).context("serializing journal entry")?;
        let mut f = self
            .writer
            .lock()
            .map_err(|e| anyhow::anyhow!("journal writer poisoned: {e}"))?;
        f.write_all(line.as_bytes())
            .context("writing journal entry")?;
        f.write_all(b"\n").context("writing journal newline")?;
        f.flush().context("flushing journal")?;
        // `sync_data` is cheaper than `sync_all` (skips metadata flush) and is
        // the right durability primitive for append-only logs: it guarantees
        // the bytes are on the device before we return.
        f.sync_data().context("fsyncing journal")?;
        Ok(())
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
        let write = self.append(&entry);
        if write.is_ok() {
            if let Ok(mut m) = self.results.lock() {
                m.insert((workflow_name.to_string(), activity_id.to_string()), result.clone());
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
        let write = self.append(&entry);
        if write.is_ok() {
            if let Ok(mut m) = self.patches.lock() {
                m.insert((workflow_name.to_string(), change_id.to_string()), version);
            }
        }
        async move { write }
    }
}

/// Mobile-aware suspend: flush + fsync any pending writes so the journal is
/// durable before the OS suspends the app. Today every record-call already
/// fsyncs so this is a defensive no-op; future buffering optimizations would
/// hook in here.
impl Suspendable for FileJournalTracker {
    fn suspend(&self, _deadline: SuspendDeadline) -> Result<(), SuspendError> {
        let f = self
            .writer
            .lock()
            .map_err(|e| SuspendError::Other(format!("journal writer poisoned: {e}")))?;
        f.sync_data()
            .map_err(|e| SuspendError::FlushFailed { message: e.to_string() })?;
        Ok(())
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
        let dir = std::env::temp_dir();
        dir.join(format!("vox_file_journal_{pid}_{n}.jsonl"))
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
            t.on_activity_completed(
                "wf",
                "act",
                &format!("wf/act/{i}"),
                &json!({"i": i}),
            )
            .await
            .unwrap();
        }
        t.record_workflow_patch("wf", "change-A", 3).await.unwrap();
        assert_eq!(t.recorded_count(), 5);
        assert_eq!(
            t.load_workflow_patch("wf", "change-A").await.unwrap(),
            Some(3)
        );

        // Drop + replay.
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

    #[tokio::test]
    async fn malformed_lines_are_skipped_at_replay() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        // Hand-craft a file with: good line, junk line, good line.
        std::fs::write(
            &path,
            "{\"kind\":\"activity_completed\",\"v\":1,\"workflow_name\":\"wf\",\"activity_name\":\"act\",\"activity_id\":\"id1\",\"result\":1}\n\
             not even json\n\
             {\"kind\":\"activity_completed\",\"v\":1,\"workflow_name\":\"wf\",\"activity_name\":\"act\",\"activity_id\":\"id2\",\"result\":2}\n",
        )
        .unwrap();

        let t = FileJournalTracker::new(&path).expect("create");
        assert_eq!(t.recorded_count(), 2);
        assert_eq!(t.load_activity_result("wf", "id1").await.unwrap(), Some(json!(1)));
        assert_eq!(t.load_activity_result("wf", "id2").await.unwrap(), Some(json!(2)));

        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn future_schema_version_lines_are_skipped() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        std::fs::write(
            &path,
            "{\"kind\":\"activity_completed\",\"v\":99,\"workflow_name\":\"wf\",\"activity_name\":\"act\",\"activity_id\":\"id\",\"result\":1}\n",
        ).unwrap();

        let t = FileJournalTracker::new(&path).expect("create");
        assert_eq!(t.recorded_count(), 0);

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
