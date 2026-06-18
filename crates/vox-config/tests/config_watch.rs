//! Phase 2D.1 — `ConfigWatch` / `ConfigSnapshot` reactive channel tests.

use vox_config::{ConfigSnapshot, ConfigWatch};

#[test]
fn rev_starts_at_zero() {
    let w = ConfigWatch::new();
    let rx = w.subscribe();
    assert_eq!(rx.borrow().rev, 0);
    assert!(rx.borrow().changed_keys.is_empty());
}

#[test]
fn bump_increments_rev_and_records_keys() {
    let w = ConfigWatch::new();
    let rx = w.subscribe();
    assert_eq!(rx.borrow().rev, 0);

    w.bump(&["VOX_WASM_SKILL_FUEL"]);

    assert_eq!(rx.borrow().rev, 1);
    assert_eq!(
        rx.borrow().changed_keys,
        vec!["VOX_WASM_SKILL_FUEL".to_string()]
    );
}

#[test]
fn subscribe_cloned_receivers_see_same_bump() {
    let w = ConfigWatch::new();
    let rx1 = w.subscribe();
    let rx2 = w.subscribe();

    w.bump(&["VOX_BUDGET_USD", "VOX_MESH_ENABLED"]);

    assert_eq!(rx1.borrow().rev, 1);
    assert_eq!(rx2.borrow().rev, 1);
    assert_eq!(rx1.borrow().changed_keys.len(), 2);
    assert_eq!(rx2.borrow().changed_keys, rx1.borrow().changed_keys);
}

#[test]
fn multiple_bumps_increment_rev_monotonically() {
    let w = ConfigWatch::new();
    let rx = w.subscribe();

    w.bump(&["key_a"]);
    let r1 = rx.borrow().rev;
    w.bump(&["key_b"]);
    let r2 = rx.borrow().rev;
    w.bump(&["key_c"]);
    let r3 = rx.borrow().rev;

    assert_eq!(r1, 1);
    assert_eq!(r2, 2);
    assert_eq!(r3, 3);
    assert_eq!(rx.borrow().changed_keys, vec!["key_c".to_string()]);
}

#[test]
fn empty_bump_increments_rev_with_empty_keys() {
    let w = ConfigWatch::new();
    let rx = w.subscribe();

    w.bump(&[]);

    assert_eq!(rx.borrow().rev, 1);
    assert!(rx.borrow().changed_keys.is_empty());
}

#[test]
fn config_snapshot_default_is_zero_rev() {
    let snap = ConfigSnapshot::default();
    assert_eq!(snap.rev, 0);
    assert!(snap.changed_keys.is_empty());
}
