//! Append-only JSON Lines file backing.
//!
//! See module docs in `lib.rs` for the crash-safety contract.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use vox_runtime::{SuspendDeadline, SuspendError, Suspendable};

/// Errors raised by [`FileJournal`].
#[derive(Debug, Error)]
pub enum JournalError {
    /// I/O failure (open, read, write, fsync, etc.).
    #[error("journal I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serde JSON failure.
    #[error("journal serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    /// The internal writer mutex was poisoned (typically by a panic in another
    /// thread while it held the lock).
    #[error("journal writer mutex poisoned")]
    Poisoned,
}

/// When [`FileJournal::append`] makes bytes durable on the device.
///
/// Picked at open time by the caller's runtime profile: desktop journals sync
/// on every append (the historical crash-safety contract), mobile journals
/// defer the `sync_data` to an explicit [`FileJournal::sync`] — typically
/// driven by an OS lifecycle hook via [`Suspendable::suspend`] — to honor
/// battery + I/O budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppendDurability {
    /// `sync_data` after every append. When `append` returns `Ok`, the bytes
    /// are on the device. Default; matches the historical behavior of
    /// [`FileJournal::open`].
    #[default]
    SyncEachAppend,
    /// Write (and flush to the OS) on every append, but defer the device
    /// `sync_data` until an explicit [`FileJournal::sync`] /
    /// [`Suspendable::suspend`]. Bytes survive a process kill (the OS has
    /// them) but need a `sync` to survive power loss.
    Deferred,
}

/// Append-only JSON Lines journal generic over the entry type `E`.
///
/// Open with [`FileJournal::open`] and a list of previously-recorded entries
/// is returned for the caller to fold back into in-memory state. Subsequent
/// [`FileJournal::append`] calls add new entries with the durability
/// contract described in the module docs.
///
/// `E` must implement [`Serialize`] for writing and [`DeserializeOwned`] for
/// replay. Most callers pick a `#[serde(tag = "kind")]`-tagged enum with a
/// version field on each variant so future entry shapes can extend the
/// file format without breaking older readers.
#[derive(Debug)]
pub struct FileJournal<E> {
    path: PathBuf,
    /// Append handle. `Mutex` guarantees `append` calls don't interleave
    /// bytes; on POSIX each `write_all` of a short line is atomic up to
    /// `PIPE_BUF`, but the mutex makes the guarantee explicit at the API
    /// level and gives us a hook for future buffering.
    writer: Mutex<File>,
    durability: AppendDurability,
    _entry: std::marker::PhantomData<E>,
}

/// Outcome of opening a journal — the live handle plus every entry already
/// on disk for the caller to replay into its own in-memory state.
#[derive(Debug)]
pub struct Opened<E> {
    /// The live journal handle.
    pub journal: FileJournal<E>,
    /// Entries successfully parsed from the existing file. Malformed lines
    /// are skipped (a `tracing::warn` is emitted for each) and preserved on
    /// disk in case a future schema knows how to read them.
    pub replayed: Vec<E>,
}

impl<E> FileJournal<E>
where
    E: Serialize + DeserializeOwned,
{
    /// Open (or create) the journal at `path` and return both the handle and
    /// every previously-recorded entry.
    ///
    /// Uses [`AppendDurability::SyncEachAppend`]; see
    /// [`FileJournal::open_with_durability`] for the deferred (mobile) mode.
    pub fn open(path: impl Into<PathBuf>) -> Result<Opened<E>, JournalError> {
        Self::open_with_durability(path, AppendDurability::SyncEachAppend)
    }

    /// Open (or create) the journal at `path` with an explicit
    /// [`AppendDurability`] policy.
    pub fn open_with_durability(
        path: impl Into<PathBuf>,
        durability: AppendDurability,
    ) -> Result<Opened<E>, JournalError> {
        let path: PathBuf = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        // Touch the file so the replay open works on first run.
        let _touch = OpenOptions::new().create(true).append(true).open(&path)?;

        let replayed = Self::replay(&path)?;

        let writer = OpenOptions::new().create(true).append(true).open(&path)?;

        let journal = Self {
            path,
            writer: Mutex::new(writer),
            durability,
            _entry: std::marker::PhantomData,
        };
        Ok(Opened { journal, replayed })
    }

    /// The path on disk where this journal is being written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append `entry` to the journal; durability per [`AppendDurability`].
    ///
    /// Crash-safety contract under [`AppendDurability::SyncEachAppend`]
    /// (the default): when this call returns `Ok`, the bytes are on the
    /// device. If the process dies between two `append` calls, the file
    /// contains every entry that returned `Ok` and zero partial lines.
    ///
    /// Under [`AppendDurability::Deferred`] the bytes are handed to the OS
    /// (process-kill safe) but only reach the device on the next
    /// [`FileJournal::sync`] / [`Suspendable::suspend`].
    pub fn append(&self, entry: &E) -> Result<(), JournalError> {
        let line = serde_json::to_string(entry)?;
        let mut f = self.writer.lock().map_err(|_| JournalError::Poisoned)?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
        f.flush()?;
        if self.durability == AppendDurability::SyncEachAppend {
            // `sync_data` is cheaper than `sync_all` (skips metadata flush)
            // and is the correct durability primitive for append-only logs.
            f.sync_data()?;
        }
        Ok(())
    }

    /// Re-read the entire file. Mostly useful for tests that want to verify
    /// what's been persisted independently of the in-memory state.
    pub fn replay_all(&self) -> Result<Vec<E>, JournalError> {
        Self::replay(&self.path)
    }

    fn replay(path: &Path) -> Result<Vec<E>, JournalError> {
        let f = File::open(path)?;
        let r = BufReader::new(f);
        let mut out: Vec<E> = Vec::new();
        for (line_no, line) in r.lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!(
                        "vox_journal: I/O error reading line {line_no}: {e}; halting replay"
                    );
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<E>(&line) {
                Ok(entry) => out.push(entry),
                Err(e) => {
                    tracing::warn!(
                        "vox_journal: line {line_no} failed to parse: {e}; line preserved on disk but skipped in memory"
                    );
                }
            }
        }
        Ok(out)
    }
}

/// Durability operations that don't depend on the entry type `E`. Kept in an
/// unbounded impl so [`Suspendable`] (also unbounded) can flush the journal.
impl<E> FileJournal<E> {
    /// Force everything appended so far onto the device (`sync_data`).
    ///
    /// This is the durability point for [`AppendDurability::Deferred`]
    /// journals; it is a cheap no-op-safe call for sync-each-append ones.
    pub fn sync(&self) -> Result<(), JournalError> {
        let f = self.writer.lock().map_err(|_| JournalError::Poisoned)?;
        f.sync_data()?;
        Ok(())
    }
}

/// Mobile-aware suspend: re-sync the file handle so any in-flight bytes are
/// durable before the OS suspends the app. For
/// [`AppendDurability::SyncEachAppend`] journals this is a defensive flush;
/// for [`AppendDurability::Deferred`] (the mobile profile) it is *the*
/// durability point — `suspend` must succeed before backgrounding or
/// un-synced appends can be lost on power-off. Idempotent: calling it
/// repeatedly is safe.
impl<E> Suspendable for FileJournal<E> {
    fn suspend(&self, _deadline: SuspendDeadline) -> Result<(), SuspendError> {
        match self.sync() {
            Ok(()) => Ok(()),
            Err(JournalError::Poisoned) => {
                Err(SuspendError::Other("journal writer poisoned".to_string()))
            }
            Err(e) => Err(SuspendError::FlushFailed {
                message: e.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    fn temp_path() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("vox_journal_test_{pid}_{n}.jsonl"))
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Entry {
        id: String,
        value: u32,
    }

    #[test]
    fn open_create_append_replay_roundtrip() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);

        // Open a fresh journal.
        let Opened { journal, replayed } = FileJournal::<Entry>::open(&path).expect("open");
        assert!(replayed.is_empty());

        // Append five entries.
        for i in 0..5 {
            journal
                .append(&Entry {
                    id: format!("e{i}"),
                    value: i,
                })
                .expect("append");
        }

        // Drop the handle and re-open.
        drop(journal);
        let Opened {
            journal: _,
            replayed,
        } = FileJournal::<Entry>::open(&path).expect("re-open");
        assert_eq!(replayed.len(), 5);
        for (i, e) in replayed.iter().enumerate() {
            assert_eq!(e.id, format!("e{i}"));
            assert_eq!(e.value as usize, i);
        }

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn malformed_lines_are_skipped_at_replay() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);

        // Hand-craft a file with: good, junk, good.
        std::fs::write(
            &path,
            "{\"id\":\"a\",\"value\":1}\n\
             not json at all\n\
             {\"id\":\"b\",\"value\":2}\n",
        )
        .unwrap();

        let Opened {
            journal: _,
            replayed,
        } = FileJournal::<Entry>::open(&path).expect("open");
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].id, "a");
        assert_eq!(replayed[1].id, "b");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn blank_lines_are_ignored() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);

        std::fs::write(&path, "\n{\"id\":\"x\",\"value\":99}\n\n\n").unwrap();

        let Opened {
            journal: _,
            replayed,
        } = FileJournal::<Entry>::open(&path).expect("open");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].id, "x");
        assert_eq!(replayed[0].value, 99);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn suspend_succeeds_on_an_open_journal() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let Opened {
            journal,
            replayed: _,
        } = FileJournal::<Entry>::open(&path).expect("open");
        journal
            .suspend(SuspendDeadline::mobile_default())
            .expect("suspend");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn deferred_durability_appends_replay_after_sync_and_suspend() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let Opened {
            journal,
            replayed: _,
        } = FileJournal::<Entry>::open_with_durability(&path, AppendDurability::Deferred)
            .expect("open deferred");
        journal
            .append(&Entry {
                id: "m".into(),
                value: 7,
            })
            .expect("append");
        // Explicit durability point.
        journal.sync().expect("sync");
        // Lifecycle durability point; idempotent.
        journal
            .suspend(SuspendDeadline::mobile_default())
            .expect("suspend");
        journal
            .suspend(SuspendDeadline::mobile_default())
            .expect("suspend twice");

        drop(journal);
        let Opened {
            journal: _,
            replayed,
        } = FileJournal::<Entry>::open(&path).expect("re-open");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].id, "m");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn replay_all_returns_current_disk_state() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let Opened {
            journal,
            replayed: _,
        } = FileJournal::<Entry>::open(&path).expect("open");
        journal
            .append(&Entry {
                id: "a".into(),
                value: 1,
            })
            .unwrap();
        journal
            .append(&Entry {
                id: "b".into(),
                value: 2,
            })
            .unwrap();
        let entries = journal.replay_all().expect("replay_all");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "a");
        assert_eq!(entries[1].id, "b");
        std::fs::remove_file(&path).ok();
    }
}
