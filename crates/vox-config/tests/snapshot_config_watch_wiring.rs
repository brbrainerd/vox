//! Phase 2D — `snapshot::bump` must also advance the process-wide `ConfigWatch` rev.

use vox_config::{config_watch, snapshot};

#[test]
fn snapshot_bump_advances_config_watch_rev() {
    let rx = config_watch::global().subscribe();
    let before = rx.borrow().rev;

    snapshot::bump(&["VOX_WASM_SKILL_FUEL"]);

    assert!(
        rx.borrow().rev > before,
        "config_watch rev must advance when snapshot::bump is called"
    );
    assert_eq!(
        rx.borrow().changed_keys,
        vec!["VOX_WASM_SKILL_FUEL".to_string()]
    );
}

#[test]
fn snapshot_bump_still_notifies_callback_listeners() {
    use std::sync::{Arc, Mutex};

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    snapshot::on_change(move |c| sink.lock().unwrap().extend(c.changed.clone()));

    snapshot::bump(&["OPENROUTER_BASE_URL"]);

    let keys = seen.lock().unwrap();
    assert!(
        keys.iter().any(|k| k == "OPENROUTER_BASE_URL"),
        "snapshot callback listeners must still fire unchanged"
    );
}
