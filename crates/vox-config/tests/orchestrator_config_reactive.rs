//! Tests for the reactive `vox://orchestrator-config-changed` event pipeline.
//!
//! These tests exercise the snapshot-invalidation mechanism that backs
//! `vox_gui::commands::orchestrator::spawn_orchestrator_config_watch`. The Tauri
//! `app_handle.emit()` call requires a live Tauri runtime, so we validate the
//! underlying contract in `vox_config::snapshot` instead:
//! `bump()` must advance the revision and deliver the changed keys to all
//! registered listeners — exactly the signal that `spawn_orchestrator_config_watch`
//! subscribes to.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use vox_config::snapshot;

/// Bumping the snapshot with orchestrator config keys advances the revision and
/// delivers those exact keys to all registered listeners.
#[test]
fn bump_delivers_orchestrator_keys_to_listener() {
    let received: Arc<Mutex<Vec<(u64, Vec<String>)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);

    snapshot::on_change(move |change| {
        sink.lock()
            .unwrap()
            .push((change.rev, change.changed.clone()));
    });

    let before = snapshot::current_rev();
    snapshot::bump(&["max_agents", "trust_auto_approve_min"]);
    let after = snapshot::current_rev();

    assert!(after > before, "revision must advance after bump");

    let g = received.lock().unwrap();
    let found = g.iter().any(|(_, keys)| {
        keys.iter().any(|k| k == "max_agents") && keys.iter().any(|k| k == "trust_auto_approve_min")
    });
    assert!(
        found,
        "listener must receive both orchestrator keys; got: {g:?}"
    );
}

/// Snapshot revisions increase monotonically across multiple bumps.
#[test]
fn orchestrator_snapshot_rev_monotonically_increases() {
    let counter = Arc::new(AtomicU64::new(0));
    let c = Arc::clone(&counter);

    snapshot::on_change(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    let r1 = snapshot::current_rev();
    snapshot::bump(&["scaling_enabled"]);
    let r2 = snapshot::current_rev();
    snapshot::bump(&["min_agents"]);
    let r3 = snapshot::current_rev();

    assert!(r2 > r1, "second bump must exceed first rev");
    assert!(r3 > r2, "third bump must exceed second rev");
}

/// An empty bump (general reload) still notifies all listeners — the
/// `spawn_orchestrator_config_watch` path must handle this case.
#[test]
fn empty_bump_notifies_orchestrator_listener() {
    let notified = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&notified);

    snapshot::on_change(move |change| {
        if change.changed.is_empty() {
            *flag.lock().unwrap() = true;
        }
    });

    snapshot::bump(&[]);
    assert!(
        *notified.lock().unwrap(),
        "empty bump must still fire the listener (general-reload signal)"
    );
}
