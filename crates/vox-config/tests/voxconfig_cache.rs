//! Track A — VoxConfig::load() SnapshotCache integration tests.
//!
//! Proves that the cache eliminates redundant TOML I/O within a snapshot rev,
//! and that a bump() causes re-evaluation on the next call.

#![allow(unsafe_code)]

use std::sync::Mutex;
use std::time::Instant;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn voxconfig_load_100_calls_under_10ms() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);

    // Warm the cache.
    let _ = vox_config::VoxConfig::load();

    let start = Instant::now();
    for _ in 0..100 {
        let _ = vox_config::VoxConfig::load();
    }
    let elapsed = start.elapsed();

    println!("100 cached VoxConfig::load() calls: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 10,
        "100 cached VoxConfig::load() calls took {elapsed:?} — expected <10ms (cache not wired?)"
    );
}

#[test]
fn voxconfig_load_re_reads_budget_after_bump() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("VOX_BUDGET_USD");

    unsafe {
        std::env::set_var("VOX_BUDGET_USD", "7.77");
    }
    vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);
    let v1 = vox_config::VoxConfig::load().daily_budget_usd;
    assert!((v1 - 7.77).abs() < 1e-6, "expected 7.77, got {v1}");

    unsafe {
        std::env::set_var("VOX_BUDGET_USD", "13.13");
    }
    vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);
    let v2 = vox_config::VoxConfig::load().daily_budget_usd;
    assert!(
        (v2 - 13.13).abs() < 1e-6,
        "must re-read after bump — expected 13.13, got {v2}"
    );

    unsafe {
        std::env::remove_var("VOX_BUDGET_USD");
    }
    vox_config::snapshot::bump(&["VOX_BUDGET_USD"]);
}
