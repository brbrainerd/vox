//! Global (cross-worktree) build coordination.
//!
//! The per-worktree [`crate::queue`] only orders builds that share one target.
//! With many worktrees and agents on one machine the real problems are
//! machine-wide: too many concurrent `cargo` builds saturate CPU/RAM/IO and pile
//! up on cargo's global package-cache lock. This module adds a **machine-wide
//! concurrency cap** (an N-slot cross-process semaphore) plus a single global
//! log, all stored OUTSIDE any repo (`~/.vox/build-broker`) so concurrent agents'
//! git operations can't wipe it.

use anyhow::Result;
use fs2::FileExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::metrics::now_ms;

/// Root dir for global broker state. Outside any repo → survives git clean/checkout.
/// Overridable with `VOX_BROKER_HOME` (used by tests).
pub fn global_root() -> PathBuf {
    if let Some(d) = std::env::var_os("VOX_BROKER_HOME") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vox").join("build-broker")
}

/// Max concurrent cargo builds machine-wide. `VOX_BROKER_MAX_CONCURRENT` overrides;
/// otherwise ~1/3 of logical cores, clamped to [2, 8].
pub fn max_concurrent() -> usize {
    let raw = std::env::var("VOX_BROKER_MAX_CONCURRENT").ok();
    let par = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    max_concurrent_from(raw.as_deref(), par)
}

/// Pure core of [`max_concurrent`], split out so it's testable without mutating
/// the process environment (which would require `unsafe` under edition 2024).
pub fn max_concurrent_from(raw: Option<&str>, parallelism: usize) -> usize {
    if let Some(n) = raw
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n >= 1)
    {
        return n;
    }
    (parallelism / 3).clamp(2, 8)
}

/// A held global slot; dropping it (closing the file handle) frees the slot.
pub struct Slot {
    _f: std::fs::File,
}

/// Try to grab one free slot without blocking. Returns `(slot, busy_count)` where
/// `busy_count` is how many of the N slots were already taken, or `None` if all
/// N are busy.
pub fn try_acquire_slot(root: &Path, n: usize) -> Result<Option<(Slot, usize)>> {
    let slots_dir = root.join("slots");
    std::fs::create_dir_all(&slots_dir)?;
    let mut busy = 0;
    for i in 0..n.max(1) {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(slots_dir.join(format!("slot_{i}")))?;
        match f.try_lock_exclusive() {
            Ok(()) => return Ok(Some((Slot { _f: f }, busy))),
            Err(_) => busy += 1,
        }
    }
    Ok(None)
}

/// Acquire one of N slots, polling until one frees. Returns the held slot, the
/// time spent waiting (ms), and how many slots were busy when we got in.
pub fn acquire_slot(root: &Path, n: usize) -> Result<(Slot, u64, usize)> {
    let start = now_ms();
    loop {
        if let Some((slot, busy)) = try_acquire_slot(root, n)? {
            let waited = now_ms().saturating_sub(start) as u64;
            return Ok((slot, waited, busy));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Marker for an in-flight build identity; dropping it removes the marker.
pub struct Inflight {
    path: PathBuf,
}

impl Drop for Inflight {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Register this build's identity (`key`) and report whether another in-flight
/// build already shares it (a true coalescing opportunity, cross-worktree).
pub fn register_inflight(root: &Path, key: &str) -> Result<(Inflight, bool)> {
    let dir = root.join("inflight");
    std::fs::create_dir_all(&dir)?;
    let mut coalesce = false;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if std::fs::read_to_string(e.path())
                .map(|s| s.trim() == key)
                .unwrap_or(false)
            {
                coalesce = true;
                break;
            }
        }
    }
    let mine = dir.join(format!("{}-{}", std::process::id(), now_ms()));
    std::fs::write(&mine, key)?;
    Ok((Inflight { path: mine }, coalesce))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semaphore_caps_concurrency() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // N=2: take both slots, the 3rd try must fail until one is released.
        let (s1, b1) = try_acquire_slot(root, 2).unwrap().unwrap();
        assert_eq!(b1, 0);
        let (_s2, b2) = try_acquire_slot(root, 2).unwrap().unwrap();
        assert_eq!(b2, 1);
        assert!(
            try_acquire_slot(root, 2).unwrap().is_none(),
            "3rd must be blocked"
        );
        drop(s1);
        assert!(
            try_acquire_slot(root, 2).unwrap().is_some(),
            "freed slot reusable"
        );
    }

    #[test]
    fn inflight_detects_same_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (_a, c1) = register_inflight(root, "build|h1").unwrap();
        assert!(!c1); // first of its kind
        let (_b, c2) = register_inflight(root, "build|h1").unwrap();
        assert!(c2); // matches the in-flight one
        let (_d, c3) = register_inflight(root, "test|h2").unwrap();
        assert!(!c3); // different identity
    }

    #[test]
    fn inflight_marker_removed_on_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        {
            let (_g, _) = register_inflight(root, "k").unwrap();
            let count = std::fs::read_dir(root.join("inflight")).unwrap().count();
            assert_eq!(count, 1);
        }
        let count = std::fs::read_dir(root.join("inflight")).unwrap().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn max_concurrent_logic() {
        assert_eq!(max_concurrent_from(Some("5"), 24), 5); // explicit override
        assert_eq!(max_concurrent_from(None, 24), 8); // 24/3=8
        assert_eq!(max_concurrent_from(Some("0"), 24), 8); // invalid -> default
        assert_eq!(max_concurrent_from(Some("xx"), 24), 8); // unparseable -> default
        assert_eq!(max_concurrent_from(None, 3), 2); // clamp to min 2
    }
}
