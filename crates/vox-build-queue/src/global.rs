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

/// Slots reserved for a build domain the filesystem semaphore cannot see (a
/// containerised CI runner sharing this host's CPU but not its mount
/// namespace). `VOX_BROKER_RESERVED_SLOTS` overrides; unset, unparseable, or
/// negative is treated as 0 reserved slots.
pub fn reserved_slots() -> usize {
    let raw = std::env::var("VOX_BROKER_RESERVED_SLOTS").ok();
    reserved_slots_from(raw.as_deref())
}

/// Pure core of [`reserved_slots`], split out for the same reason as
/// [`max_concurrent_from`].
pub fn reserved_slots_from(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.parse::<usize>().ok()).unwrap_or(0)
}

/// The effective machine-wide concurrency cap: [`max_concurrent`] reduced by
/// [`reserved_slots`], floored at 1 (never 0 — `acquire_slot` loops until a
/// slot frees, and a cap of 0 slots never frees one).
pub fn effective_max_concurrent() -> usize {
    let max_raw = std::env::var("VOX_BROKER_MAX_CONCURRENT").ok();
    let reserved_raw = std::env::var("VOX_BROKER_RESERVED_SLOTS").ok();
    let par = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    effective_max_concurrent_from(max_raw.as_deref(), reserved_raw.as_deref(), par)
}

/// Pure core of [`effective_max_concurrent`]. The reservation is applied
/// *after* `VOX_BROKER_MAX_CONCURRENT` — an explicit override is still
/// subject to it, because the reserved slots are physically in use by a
/// kernel this semaphore can't see regardless of what the override says.
pub fn effective_max_concurrent_from(
    max_raw: Option<&str>,
    reserved_raw: Option<&str>,
    parallelism: usize,
) -> usize {
    let base = max_concurrent_from(max_raw, parallelism);
    let reserved = reserved_slots_from(reserved_raw);
    base.saturating_sub(reserved).max(1)
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

/// Count how many of the N slots are currently held, WITHOUT disturbing them:
/// each slot is probed with a non-blocking lock attempt and immediately
/// released if acquired (never held past the probe). Unlike
/// [`try_acquire_slot`] this never returns a slot to the caller, so calling it
/// (e.g. from a read-only status viewer) can't itself perturb the count it's
/// trying to measure.
///
/// This is inherently a **sample, not a snapshot**: another process may take
/// or release a slot between this probing two different slot files, so the
/// count can be stale the instant it's returned.
pub fn probe_busy_slots(root: &Path, n: usize) -> Result<usize> {
    let slots_dir = root.join("slots");
    if !slots_dir.is_dir() {
        return Ok(0);
    }
    let mut busy = 0;
    for i in 0..n.max(1) {
        let path = slots_dir.join(format!("slot_{i}"));
        if !path.is_file() {
            continue; // never claimed yet -> free
        }
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;
        match f.try_lock_exclusive() {
            Ok(()) => {
                let _ = f.unlock();
            }
            Err(_) => busy += 1,
        }
    }
    Ok(busy)
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
    fn semaphore_caps_concurrency_with_reservation() {
        // Same shape as `semaphore_caps_concurrency`, but exercising the
        // scenario the reservation exists for: an effective cap of 1 (base 3,
        // reserved 2) must serialize -- hold a slot, prove the second
        // acquisition is blocked, release, and prove it succeeds again. This
        // is the deliberate proof the brief asks for in place of a timing race.
        let n = effective_max_concurrent_from(Some("3"), Some("2"), 24);
        assert_eq!(n, 1);
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (s1, b1) = try_acquire_slot(root, n).unwrap().unwrap();
        assert_eq!(b1, 0);
        assert!(
            try_acquire_slot(root, n).unwrap().is_none(),
            "reserved-down cap of 1 must block a second acquisition"
        );
        drop(s1);
        assert!(
            try_acquire_slot(root, n).unwrap().is_some(),
            "freed slot reusable"
        );
    }

    #[test]
    fn probe_busy_slots_never_holds_what_it_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(probe_busy_slots(root, 2).unwrap(), 0, "nothing held yet");

        let (_s1, _) = try_acquire_slot(root, 2).unwrap().unwrap();
        assert_eq!(probe_busy_slots(root, 2).unwrap(), 1);

        // The probe must not itself hold the free slot: a real acquire must
        // still succeed right after probing.
        let (_s2, busy) = try_acquire_slot(root, 2).unwrap().unwrap();
        assert_eq!(busy, 1);
        assert_eq!(probe_busy_slots(root, 2).unwrap(), 2);
    }

    #[test]
    fn probe_busy_slots_on_missing_root_is_zero() {
        let tmp = tempfile::tempdir().unwrap();
        // Root exists but `slots/` was never created -- broker never ran.
        assert_eq!(probe_busy_slots(tmp.path(), 4).unwrap(), 0);
    }

    #[test]
    fn reserved_slots_logic() {
        assert_eq!(reserved_slots_from(None), 0);
        assert_eq!(reserved_slots_from(Some("0")), 0);
        assert_eq!(reserved_slots_from(Some("3")), 3);
        assert_eq!(reserved_slots_from(Some("-1")), 0, "negative -> ignored");
        assert_eq!(reserved_slots_from(Some("xx")), 0, "unparseable -> ignored");
    }

    #[test]
    fn effective_max_concurrent_logic() {
        // No reservation: falls through to the base cap unchanged.
        assert_eq!(effective_max_concurrent_from(None, None, 24), 8);
        assert_eq!(effective_max_concurrent_from(None, Some("0"), 24), 8);
        // Normal reservation: base 8, reserve 3 -> 5.
        assert_eq!(effective_max_concurrent_from(None, Some("3"), 24), 5);
        // Reservation exceeding the base cap floors at 1, never 0.
        assert_eq!(effective_max_concurrent_from(None, Some("99"), 24), 1);
        // Reservation applies AFTER an explicit override too.
        assert_eq!(effective_max_concurrent_from(Some("4"), Some("1"), 24), 3);
        assert_eq!(effective_max_concurrent_from(Some("4"), Some("10"), 24), 1);
        // Unparseable reservation is ignored (treated as 0).
        assert_eq!(
            effective_max_concurrent_from(Some("4"), Some("nope"), 24),
            4
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
