//! Phase 5.4 — counting-shim harness: proves that N hot-path accessor calls
//! return consistent values within a single snapshot revision, and that
//! values update after a bump.
//!
//! Integration smoke-test only; the per-call reuse guarantee is unit-tested
//! in `snapshot.rs::snapshot_cache_reuses_value_within_same_rev`. This suite
//! verifies the caches are wired into the real accessors end-to-end.

#![allow(unsafe_code)] // Rust 2024: env mutation is unsafe.

use std::sync::Mutex;

/// Serialize env-mutating tests within this file.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Call accessor N times and assert all results are identical.
fn assert_stable<T: PartialEq + std::fmt::Debug>(n: usize, f: impl Fn() -> T) -> T {
    let first = f();
    for _ in 1..n {
        let v = f();
        assert_eq!(
            v, first,
            "accessor returned different values within same rev"
        );
    }
    first
}

#[test]
fn openrouter_base_url_stable_across_6_calls() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://counting-test.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

    let val = assert_stable(6, vox_config::inference::openrouter_base_url);
    assert_eq!(val, "https://counting-test.example/api");

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}

#[test]
fn openai_base_url_stable_across_6_calls() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("VOX_OPENAI_BASE_URL");
    let _ = vox_config::toml_config::unset_user_config_value("OPENAI_BASE_URL");
    unsafe {
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::set_var("VOX_OPENAI_BASE_URL", "https://openai-counting.example/v1");
    }
    vox_config::snapshot::bump(&["VOX_OPENAI_BASE_URL", "OPENAI_BASE_URL"]);

    let val = assert_stable(6, vox_config::inference::openai_compatible_base_url);
    assert_eq!(val, "https://openai-counting.example/v1");

    unsafe {
        std::env::remove_var("VOX_OPENAI_BASE_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("VOX_OPENAI_BASE_URL");
    vox_config::snapshot::bump(&["VOX_OPENAI_BASE_URL"]);
}

#[test]
fn local_ollama_base_url_stable_across_6_calls() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OLLAMA_URL");
    let _ = vox_config::toml_config::unset_user_config_value("POPULI_URL");
    unsafe {
        std::env::remove_var("OLLAMA_URL");
        std::env::remove_var("POPULI_URL");
        std::env::remove_var("VOX_POPULI_LOCAL_OLLAMA_URL");
        std::env::set_var("OLLAMA_URL", "http://counting-ollama:11434");
    }
    vox_config::snapshot::bump(&["OLLAMA_URL"]);

    let val = assert_stable(6, vox_config::inference::local_ollama_populi_base_url);
    assert_eq!(val, "http://counting-ollama:11434");

    unsafe {
        std::env::remove_var("OLLAMA_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("OLLAMA_URL");
    vox_config::snapshot::bump(&["OLLAMA_URL"]);
}

#[test]
fn multi_accessor_cascade_returns_consistent_results_across_6_calls() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://cascade-test.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

    // Simulate 6 cascade stages each calling multiple accessors.
    let (mut or_urls, mut oa_urls) = (Vec::new(), Vec::new());
    for _ in 0..6 {
        or_urls.push(vox_config::inference::openrouter_chat_completions_url());
        oa_urls.push(vox_config::inference::openai_chat_completions_url());
    }

    assert!(
        or_urls.windows(2).all(|w| w[0] == w[1]),
        "openrouter url inconsistent: {:?}",
        or_urls
    );
    assert!(
        oa_urls.windows(2).all(|w| w[0] == w[1]),
        "openai url inconsistent: {:?}",
        oa_urls
    );
    assert!(
        or_urls[0].contains("cascade-test.example"),
        "unexpected url: {}",
        or_urls[0]
    );

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}

#[test]
fn base_url_re_reads_after_bump() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");

    // Phase 1: set + bump + warm cache.
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://phase1.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
    let v1 = vox_config::inference::openrouter_base_url();
    assert_eq!(v1, "https://phase1.example/api");

    // Phase 2: change env + bump → must see new value.
    unsafe {
        std::env::set_var("OPENROUTER_BASE_URL", "https://phase2.example/api");
    }
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
    let v2 = vox_config::inference::openrouter_base_url();
    assert_eq!(v2, "https://phase2.example/api", "must re-read after bump");

    unsafe {
        std::env::remove_var("OPENROUTER_BASE_URL");
    }
    let _ = vox_config::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
    vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);
}
