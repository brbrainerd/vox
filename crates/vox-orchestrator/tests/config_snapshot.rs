#![allow(unsafe_code)] // test-only std::env::set_var (edition 2024)
//! Integration tests for [`OrchestratorConfig::snapshot`].
//!
//! Tests run in their own process (integration tests), so static caches are not
//! polluted by other unit-test state. We serialize env-var mutations behind a mutex
//! to prevent cross-test interference when running `cargo test` with multiple threads.
// Rust 2024 made std::env::{set_var,remove_var} unsafe; serialized as noted above.
#![allow(unsafe_code)]

use std::sync::Mutex;
use std::time::Instant;

use vox_orchestrator::config::OrchestratorConfig;

/// Serialize all tests in this file that touch env-vars.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn snapshot_returns_env_override_not_just_defaults() {
    let _guard = ENV_MUTEX.lock().expect("env mutex");

    // Bump the snapshot cache so the previous cached value (from another test or
    // a prior call) is invalidated before we set the env var.
    vox_config::snapshot::bump(&["VOX_ORCHESTRATOR_MAX_AGENTS"]);

    let prev = std::env::var("VOX_ORCHESTRATOR_MAX_AGENTS").ok();
    // SAFETY: single-threaded env mutation guarded by ENV_MUTEX.
    unsafe {
        std::env::set_var("VOX_ORCHESTRATOR_MAX_AGENTS", "42");
    }

    // Bump again so the cache recomputes after the env change.
    vox_config::snapshot::bump(&["VOX_ORCHESTRATOR_MAX_AGENTS"]);

    let cfg = OrchestratorConfig::snapshot();
    assert_eq!(cfg.max_agents, 42, "env override must win over defaults");

    // Restore.
    unsafe {
        match prev {
            Some(v) => std::env::set_var("VOX_ORCHESTRATOR_MAX_AGENTS", v),
            None => std::env::remove_var("VOX_ORCHESTRATOR_MAX_AGENTS"),
        }
    }
    // Final bump to clean up for subsequent tests.
    vox_config::snapshot::bump(&["VOX_ORCHESTRATOR_MAX_AGENTS"]);
}

#[test]
fn snapshot_100_calls_under_10ms() {
    // Warm the cache with one call first.
    let _ = OrchestratorConfig::snapshot();

    let start = Instant::now();
    for _ in 0..100 {
        let _ = OrchestratorConfig::snapshot();
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 10,
        "100 cached snapshot() calls must complete in <10ms, took {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn snapshot_returns_sensible_defaults() {
    let cfg = OrchestratorConfig::snapshot();
    assert!(cfg.max_agents >= 1, "max_agents must be at least 1");
    assert!(cfg.lock_timeout_ms > 0, "lock_timeout_ms must be positive");
    assert!(
        cfg.bulletin_capacity > 0,
        "bulletin_capacity must be positive"
    );
}
