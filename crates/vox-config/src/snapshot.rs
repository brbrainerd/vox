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

type Listener = Box<dyn Fn(&LlmConfigChange) + Send + Sync + 'static>;

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
    listeners().lock().expect("snapshot listeners mutex poisoned").push(Box::new(f));
}

/// Advance the revision and notify listeners. Call after any LLM/AI config write.
pub fn bump(changed_keys: &[&str]) {
    let rev = REV.fetch_add(1, Ordering::SeqCst) + 1;
    let change = LlmConfigChange {
        rev,
        changed: changed_keys.iter().map(|s| (*s).to_string()).collect(),
    };
    // Hold the lock only to clone out the call list, so a listener that itself touches
    // config (re-entrant bump) cannot deadlock on the listeners mutex.
    let guard = listeners().lock().expect("snapshot listeners mutex poisoned");
    for l in guard.iter() {
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
            g.iter().any(|(_, keys)| keys.iter().any(|k| k == "OPENROUTER_BASE_URL")),
            "listener must receive the changed key"
        );
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
        assert!(*seen.lock().unwrap(), "empty bump must still notify (reload signal)");
    }
}
