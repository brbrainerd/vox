//! Reactive notification for LLM/AI config changes.
//!
//! Writers call [`bump`] after mutating config; consumers (e.g. the GUI) register a
//! listener via [`on_change`]. This is std-only by design — `vox-config` is a low layer
//! and must not pull in an async runtime just to notify. The GUI adapts these callbacks
//! into a Tauri `vox://llm-config-changed` event. Listeners run synchronously on the
//! writer's thread, so they must be cheap and non-blocking (emitting an event qualifies).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// A single config-change notification.
#[derive(Debug, Clone)]
pub struct LlmConfigChange {
    /// Monotonic revision; advances by 1 per [`bump`].
    pub rev: u64,
    /// Keys changed in this bump (empty = a general reload, treat as "refetch all").
    pub changed: Vec<String>,
}

static REV: AtomicU64 = AtomicU64::new(0);

type Listener = std::sync::Arc<dyn Fn(&LlmConfigChange) + Send + Sync + 'static>;

fn listeners() -> &'static Mutex<Vec<Listener>> {
    static L: OnceLock<Mutex<Vec<Listener>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(Vec::new()))
}

/// Current monotonic revision (0 before any change).
#[must_use]
pub fn current_rev() -> u64 {
    REV.load(Ordering::SeqCst)
}

/// Register a change listener. Invoked synchronously on the writer's thread for every
/// subsequent [`bump`]. Listeners are never removed (process-lifetime subscriptions).
pub fn on_change(f: impl Fn(&LlmConfigChange) + Send + Sync + 'static) {
    listeners()
        .lock()
        .expect("snapshot listeners mutex poisoned")
        .push(std::sync::Arc::new(f));
}

/// Zero-cost static cache keyed on the snapshot revision.
///
/// Wrap a derived value in this cache to avoid recomputing it when the config has not
/// changed since the last call. The cache stores the revision at which the value was
/// computed; if the current revision matches, the cached value is returned without
/// calling `f`. On a rev mismatch (i.e., after a [`bump`]) `f` is called and the
/// result replaces the stored value.
///
/// # Usage
/// ```rust,ignore
/// static CACHE: SnapshotCache<String> = SnapshotCache::new();
/// let value = CACHE.get_or_init(|| expensive_computation());
/// ```
pub struct SnapshotCache<T: Clone + Send + Sync + 'static>(OnceLock<Mutex<(u64, Option<T>)>>);

impl<T: Clone + Send + Sync + 'static> Default for SnapshotCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static> SnapshotCache<T> {
    /// Create a new cache. Usable as a `static` initialiser.
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// Return the cached value if the current revision matches the stored revision,
    /// otherwise call `f`, store the result, and return a clone of it.
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> T {
        let rev = REV.load(Ordering::SeqCst);
        let inner = self.0.get_or_init(|| Mutex::new((u64::MAX, None)));
        let mut guard = inner.lock().expect("SnapshotCache mutex poisoned");
        if guard.0 == rev {
            // SAFETY: guard.0 == rev means we initialised with Some on a prior call
            // at the same rev; None only occurs on u64::MAX sentinel before first use.
            if let Some(ref cached) = guard.1 {
                return cached.clone();
            }
        }
        let value = f();
        *guard = (rev, Some(value.clone()));
        value
    }
}

/// Advance the revision and notify listeners. Call after any LLM/AI config write.
pub fn bump(changed_keys: &[&str]) {
    let rev = REV.fetch_add(1, Ordering::SeqCst) + 1;
    let change = LlmConfigChange {
        rev,
        changed: changed_keys.iter().map(|s| (*s).to_string()).collect(),
    };
    // Snapshot the listener list while holding the lock, then release the lock
    // BEFORE calling any listener. This prevents deadlock if a listener calls
    // bump() or on_change() re-entrantly.
    let snapshot: Vec<Listener> = listeners()
        .lock()
        .expect("snapshot listeners mutex poisoned")
        .clone();
    for l in &snapshot {
        l(&change);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[test]
    fn bump_advances_rev_and_delivers_changed_keys() {
        let seen: Arc<StdMutex<Vec<(u64, Vec<String>)>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        on_change(move |c| sink.lock().unwrap().push((c.rev, c.changed.clone())));

        let before = current_rev();
        bump(&["OPENROUTER_BASE_URL"]);
        let after = current_rev();

        // Monotonic advance (other tests in this binary may bump concurrently, so this
        // is `>`, not exactly +1; the per-bump +1 increment is exercised by the atomic).
        assert!(after > before, "rev must advance on bump");
        let g = seen.lock().unwrap();
        assert!(
            g.iter()
                .any(|(_, keys)| keys.iter().any(|k| k == "OPENROUTER_BASE_URL")),
            "listener must receive the changed key"
        );
    }

    #[test]
    fn snapshot_cache_returns_init_value() {
        let cache: SnapshotCache<String> = SnapshotCache::new();
        let val = cache.get_or_init(|| "hello".to_string());
        assert_eq!(val, "hello");
    }

    #[test]
    fn snapshot_cache_reuses_value_within_same_rev() {
        use std::sync::atomic::AtomicUsize;
        let cache: SnapshotCache<String> = SnapshotCache::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c1 = Arc::clone(&counter);
        let _v1 = cache.get_or_init(|| {
            c1.fetch_add(1, Ordering::SeqCst);
            "first".to_string()
        });
        let c2 = Arc::clone(&counter);
        let _v2 = cache.get_or_init(|| {
            c2.fetch_add(1, Ordering::SeqCst);
            "second".to_string()
        });
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "closure must be called only once within same rev"
        );
    }

    #[test]
    fn snapshot_cache_refreshes_on_bump() {
        let cache: SnapshotCache<String> = SnapshotCache::new();
        let v1 = cache.get_or_init(|| "before".to_string());
        assert_eq!(v1, "before");
        bump(&["test_key"]);
        let v2 = cache.get_or_init(|| "after".to_string());
        assert_eq!(v2, "after");
    }

    #[test]
    fn snapshot_cache_is_thread_safe() {
        use std::thread;
        let cache = Arc::new(SnapshotCache::<String>::new());
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let c = Arc::clone(&cache);
                thread::spawn(move || c.get_or_init(|| format!("thread-{}", i)))
            })
            .collect();
        for h in handles {
            let val = h.join().expect("thread panicked");
            assert!(val.starts_with("thread-"));
        }
    }

    #[test]
    fn empty_bump_signals_general_reload() {
        let seen: Arc<StdMutex<bool>> = Arc::new(StdMutex::new(false));
        let sink = Arc::clone(&seen);
        on_change(move |c| {
            if c.changed.is_empty() {
                *sink.lock().unwrap() = true;
            }
        });
        bump(&[]);
        assert!(
            *seen.lock().unwrap(),
            "empty bump must still notify (reload signal)"
        );
    }
}
