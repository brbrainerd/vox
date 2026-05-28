//! P2-T1 acceptance: Bundle round-trip, cache hit, and idempotent put via BundleStore.

use std::sync::Arc;
use vox_package::bundle::{Bundle, BundleRef, BundleStore};

#[test]
fn bundle_round_trip_by_fn_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let bundle = Bundle {
        fn_hash: [0xABu8; 64],
        deps: vec![],
        bytes: Arc::new(b"compiled-form-of-workflow".to_vec()),
        manifest: serde_json::json!({
            "kind": "workflow",
            "name": "my::workflow",
            "vox_version": env!("CARGO_PKG_VERSION"),
        }),
    };

    let bundle_ref = store.put(&bundle).expect("put");
    assert_eq!(bundle_ref.fn_hash, [0xABu8; 64]);

    let loaded = store.lookup(&bundle_ref).expect("lookup").expect("hit");
    assert_eq!(loaded.fn_hash, bundle.fn_hash);
    assert_eq!(loaded.bytes.as_ref(), bundle.bytes.as_ref());
}

#[test]
fn bundle_lookup_miss_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let unknown = BundleRef {
        fn_hash: [0x77u8; 64],
    };
    let result = store.lookup(&unknown).expect("lookup ok");
    assert!(result.is_none(), "miss should return None, not error");
}

#[test]
fn put_is_idempotent_for_same_fn_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let bundle = Bundle {
        fn_hash: [0x42u8; 64],
        deps: vec![],
        bytes: Arc::new(b"bytes-v1".to_vec()),
        manifest: serde_json::json!({}),
    };

    let _ = store.put(&bundle).expect("put 1");
    let r2 = store.put(&bundle).expect("put 2 — must not error");
    assert_eq!(r2.fn_hash, bundle.fn_hash);
}
