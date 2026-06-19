//! Fair FIFO queue lock scoped to one worktree.
//!
//! Backed by a lock directory under `<worktree>/.vox/build-queue/<hash>/`:
//! - `counter` — monotonic ticket source (locked while bumped).
//! - `t<seq>`  — one outstanding-ticket marker per waiter, used for position.
//! - `run.lock` — the exclusive run lock; the OS arbitrates a single runner.
//!
//! Mutual exclusion is guaranteed by the OS exclusive lock on `run.lock`. The
//! ticket markers provide the *position display* and waiter accounting. Strict
//! cross-process FIFO ordering is best-effort (the OS wakes one waiter); ticket
//! numbers approximate arrival order, which is sufficient for the UX + metrics
//! goals of the daemonless broker.

use anyhow::Result;
use fs2::FileExt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::metrics::now_ms;

/// Markers older than this are treated as stale (the owning process crashed
/// before its `Ticket` Drop ran) and reaped during scans. Generous so a genuine
/// long build never has its marker reaped out from under it.
const STALE_MS: u128 = 4 * 60 * 60 * 1000; // 4 hours

/// Stable 16-hex-digit hash of a path, used as the per-worktree queue subdir.
/// Shared by the shim and `build_service` so both address the same queue (DRY).
pub fn hash_path(p: &Path) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// A fair FIFO queue scoped to one worktree.
pub struct FairQueue {
    dir: PathBuf,
}

/// Held while a build runs; dropping releases the run lock and ticket marker.
pub struct Ticket {
    seq: u64,
    dir: PathBuf,
    _lock: std::fs::File,
}

impl FairQueue {
    pub fn new(queue_root: &Path, worktree_hash: &str) -> Result<Self> {
        let dir = queue_root.join(worktree_hash);
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Claim the next monotonic ticket number (arrival order) and record an
    /// outstanding marker (`t<seq>` containing `<ts_ms>\n<key>`) so `position`
    /// can count waiters ahead and `coalesce_opportunity` can match command
    /// identity. `key` is the combined argv+env fingerprint of this invocation.
    pub fn take_ticket(&self, key: &str) -> Result<u64> {
        use std::io::{Read, Seek, SeekFrom, Write};
        let counter = self.dir.join("counter");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&counter)?;
        // Hold a single locked handle and read/write through it; opening a second
        // handle to the same file while locked fails on Windows (os error 33).
        f.lock_exclusive()?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let next = s.trim().parse::<u64>().unwrap_or(0) + 1;
        f.set_len(0)?;
        f.seek(SeekFrom::Start(0))?;
        f.write_all(next.to_string().as_bytes())?;
        f.flush()?;
        // Write the marker BEFORE releasing the counter lock so counter-advance
        // and marker-existence are atomic w.r.t. other waiters (review #3).
        std::fs::write(
            self.dir.join(format!("t{next}")),
            format!("{}\n{}", now_ms(), key),
        )?;
        FileExt::unlock(&f)?;
        Ok(next)
    }

    /// Live (non-stale) markers as `(seq, key)`, reaping stale ones in passing.
    fn live_markers(&self) -> Vec<(u64, String)> {
        let now = now_ms();
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for e in rd.flatten() {
            let name = e.file_name();
            let Some(seq) = name
                .to_str()
                .and_then(|n| n.strip_prefix('t'))
                .and_then(|n| n.parse::<u64>().ok())
            else {
                continue;
            };
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            let mut lines = content.lines();
            let ts: u128 = lines
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let key = lines.next().unwrap_or("").to_string();
            if ts != 0 && now.saturating_sub(ts) > STALE_MS {
                let _ = std::fs::remove_file(e.path()); // reap crashed owner's marker
                continue;
            }
            out.push((seq, key));
        }
        out
    }

    /// How many live tickets are ahead of `seq` (lower-numbered).
    pub fn position(&self, seq: u64) -> usize {
        self.live_markers().iter().filter(|(n, _)| *n < seq).count()
    }

    /// Whether another live invocation shares this command identity (`key`),
    /// i.e. a true coalescing opportunity (not merely queue contention).
    pub fn coalesce_opportunity(&self, seq: u64, key: &str) -> bool {
        self.live_markers()
            .iter()
            .any(|(n, k)| *n != seq && k == key)
    }

    /// Block until this caller holds the exclusive run lock, then return a
    /// `Ticket` whose drop releases the lock and removes the ticket marker.
    pub fn acquire(&self, seq: u64) -> Result<Ticket> {
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.dir.join("run.lock"))?;
        lock.lock_exclusive()?;
        Ok(Ticket {
            seq,
            dir: self.dir.clone(),
            _lock: lock,
        })
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.dir.join(format!("t{}", self.seq)));
        // `_lock` unlocks when the file handle closes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_are_monotonic() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let a = q.take_ticket("k").unwrap();
        let b = q.take_ticket("k").unwrap();
        assert_eq!(b, a + 1);
    }

    #[test]
    fn position_counts_outstanding_ahead() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let first = q.take_ticket("k").unwrap();
        let second = q.take_ticket("k").unwrap();
        assert_eq!(q.position(second), 1);
        assert_eq!(q.position(first), 0);
    }

    #[test]
    fn coalesce_only_on_matching_key() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let a = q.take_ticket("build|hash1").unwrap();
        let _b = q.take_ticket("build|hash1").unwrap(); // same identity
        let c = q.take_ticket("test|hash2").unwrap(); // different identity
        assert!(q.coalesce_opportunity(a, "build|hash1"));
        assert!(!q.coalesce_opportunity(c, "test|hash2"));
    }

    #[test]
    fn stale_marker_is_reaped() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let live = q.take_ticket("k").unwrap();
        // Forge a stale marker (ts well past the staleness window) as if a prior
        // process crashed without running Drop.
        std::fs::write(tmp.path().join("abc").join("t1000"), "1\nk").unwrap();
        // position for our live ticket must not count the stale phantom.
        assert_eq!(q.position(live), 0);
        assert!(!tmp.path().join("abc").join("t1000").exists()); // reaped
    }

    #[test]
    fn acquire_then_drop_releases() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let n = q.take_ticket("k").unwrap();
        {
            let _t = q.acquire(n).unwrap();
            assert_eq!(q.position(n), 0);
        }
        assert!(!tmp.path().join("abc").join(format!("t{n}")).exists());
    }

    #[test]
    fn hash_path_is_stable_and_hex() {
        let p = Path::new("/some/worktree");
        let h = hash_path(p);
        assert_eq!(h.len(), 16);
        assert_eq!(h, hash_path(p));
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
