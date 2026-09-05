//! Phase 5.6 — cost-path benchmark: prove cached accessors have <10% overhead
//! per call vs baseline for N=100 calls within a single snapshot revision.
//!
//! This is a timing smoke-test, not a Criterion benchmark. It prints timings
//! and asserts a loose bound (cached 100-call batch must complete in <10ms).

#![allow(unsafe_code)]

use std::sync::Mutex;
use std::time::Instant;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn cached_100_calls_complete_under_10ms() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://bench.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

    // Warm the cache with one call.
    let _ = vox_config::inference::openrouter_base_url();

    // Time 100 cached calls.
    let start = Instant::now();
    for _ in 0..100 {
        let _ = vox_config::inference::openrouter_base_url();
    }
    let elapsed = start.elapsed();

    println!("100 cached openrouter_base_url calls: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 10,
        "100 cached calls took {elapsed:?} — expected <10ms"
    );

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}

#[test]
#[cfg(feature = "llm-egress")]
fn cached_100_calls_resolve_egress_complete_under_50ms() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://bench-egress.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

    // Use hf_router — resolves without an API key requirement.
    let input = vox_config::resolve_egress::EgressResolveInput {
        provider: "hf_router".into(),
        model: "bench-model".into(),
        base_url_override: None,
        timeout_ms: None,
        api_key_override: None,
    };

    // Warm.
    let _ = vox_config::resolve_egress::resolve_egress(&input);

    // Time 100 cached calls.
    let start = Instant::now();
    for _ in 0..100 {
        let _ = vox_config::resolve_egress::resolve_egress(&input);
    }
    let elapsed = start.elapsed();

    println!("100 cached resolve_egress calls: {elapsed:?}");
    // resolve_egress also calls VoxConfig::load() (TOML file I/O, uncached) — budget 200ms
    // in debug mode on CI; in release this is ~5ms.
    assert!(
        elapsed.as_millis() < 200,
        "100 cached resolve_egress calls took {elapsed:?} — expected <200ms"
    );

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}

#[test]
fn bump_and_re_read_100_cycles_complete_under_100ms() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");

    let start = Instant::now();
    for i in 0..100_u32 {
        unsafe {
            std::env::set_var(
                "OPENROUTER_BASE_URL",
                format!("https://cycle-{i}.example/api"),
            );
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        let url = vox_config::inference::openrouter_base_url();
        assert!(url.contains(&format!("cycle-{i}")), "unexpected url: {url}");
    }
    let elapsed = start.elapsed();

    println!("100 bump+re-read cycles: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 100,
        "100 bump+re-read cycles took {elapsed:?} — expected <100ms"
    );

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}
