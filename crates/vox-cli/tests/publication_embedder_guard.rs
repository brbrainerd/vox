#![allow(unsafe_code)] // test-only std::env::set_var (edition 2024)
//! Guard: online prior-art fetch must fail fast when embedder is required but missing.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

const EMBEDDER_GUARD_CALLSITES: &[&str] = &[
    "crates/vox-cli/src/commands/db/publication/decision.rs",
    "crates/vox-cli/src/commands/db/publication/discovery.rs",
    "crates/vox-cli/src/commands/db/publication/discovery_watch.rs",
];

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

struct EnvVarGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        // SAFETY: test-only; env mutation is scoped to this guard's lifetime.
        unsafe { std::env::set_var(key, value) };
        Self { key, prior }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

#[test]
fn online_novelty_requires_embedder_when_env_flag_set() {
    let _lock = ENV_TEST_LOCK.lock().expect("env test lock");
    let _guard = EnvVarGuard::set("VOX_SCIENTIA_REQUIRE_EMBEDDER", "1");
    let err = vox_publisher::scientia_semantic::require_embedder_for_online_novelty(false, false)
        .unwrap_err();
    assert!(err.to_string().contains("VOX_SCIENTIA_REQUIRE_EMBEDDER"));
}

#[test]
fn offline_novelty_skips_embedder_guard() {
    let _lock = ENV_TEST_LOCK.lock().expect("env test lock");
    let _guard = EnvVarGuard::set("VOX_SCIENTIA_REQUIRE_EMBEDDER", "1");
    vox_publisher::scientia_semantic::require_embedder_for_online_novelty(true, false)
        .expect("offline bypasses embedder guard");
}

#[test]
fn embedder_guard_inactive_when_env_unset() {
    let _lock = ENV_TEST_LOCK.lock().expect("env test lock");
    let _guard = EnvVarGuard::set("VOX_SCIENTIA_REQUIRE_EMBEDDER", "0");
    vox_publisher::scientia_semantic::require_embedder_for_online_novelty(false, false)
        .expect("flag off bypasses embedder guard");
}

#[test]
fn embedder_guard_wired_at_online_novelty_callsites() {
    let root = workspace_root();
    let needle = "require_embedder_for_online_novelty";
    let mut missing = Vec::new();

    for suffix in EMBEDDER_GUARD_CALLSITES {
        let path = root.join(suffix);
        let contents = std::fs::read_to_string(&path).expect("read callsite source");
        if !contents.contains(needle) {
            missing.push(*suffix);
        }
    }

    assert!(
        missing.is_empty(),
        "require_embedder_for_online_novelty must be called from online novelty surfaces; missing in:\n{}",
        missing.join("\n")
    );
}

#[test]
fn discovery_watch_entrypoint_calls_embedder_guard_before_assessment() {
    let root = workspace_root();
    let path = root.join("crates/vox-cli/src/commands/db/publication/discovery_watch.rs");
    let contents = std::fs::read_to_string(&path).expect("read discovery_watch source");
    let fn_start = contents
        .find("pub async fn discovery_watch")
        .expect("discovery_watch entrypoint");
    let guard = contents[fn_start..]
        .find("require_embedder_for_online_novelty")
        .expect("discovery_watch must call require_embedder_for_online_novelty");
    let assessment = contents[fn_start..]
        .find("uniqueness_signal_for_commit")
        .expect("discovery_watch assesses code uniqueness");
    assert!(
        guard < assessment,
        "discovery_watch must fail fast on missing embedder before code-uniqueness assessment"
    );
}
