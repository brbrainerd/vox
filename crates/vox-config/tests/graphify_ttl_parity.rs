//! Guard: the graphify staleness TTL has exactly one source of truth.
//!
//! `resolve_ttl_days` lets `VOX_GRAPHIFY_TTL_DAYS` override the contract value.
//! That is fine for an ad-hoc local run, but a workflow that pins it makes CI
//! enforce a different TTL from the one the GUI edits and the CLI reports.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // crates/vox-config/ -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn no_workflow_pins_the_graphify_ttl_env_var() {
    let dir = repo_root().join(".github/workflows");
    let mut scanned = 0usize;
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("yml") && ext != Some("yaml") {
            continue;
        }
        scanned += 1;
        let raw = std::fs::read_to_string(&path).expect("read workflow");
        for (i, line) in raw.lines().enumerate() {
            // A YAML comment cannot set an env var; only a real mapping entry can.
            if line.trim_start().starts_with('#') {
                continue;
            }
            if line.contains(vox_config::graphify::GRAPHIFY_TTL_DAYS_ENV) {
                offenders.push(format!("{}:{}", path.display(), i + 1));
            }
        }
    }
    // A scan that found nothing to read would assert "no offenders" vacuously.
    assert!(
        scanned > 0,
        "no workflow files found under {} — this guard scanned nothing",
        dir.display()
    );
    assert!(
        offenders.is_empty(),
        "workflows must not pin VOX_GRAPHIFY_TTL_DAYS — it overrides \
         ttl_days_default in {}, so CI would enforce a different staleness \
         window than the GUI and CLI report. Set the value in the contract \
         instead. Offenders: {offenders:?}",
        vox_config::graphify::CORPORA_REL_PATH,
    );
}

#[test]
fn contract_ttl_is_the_value_every_caller_resolves() {
    // The invariant the guard above protects: absent an env override, the
    // contract value is what every caller sees. Asserted through the pure
    // helper rather than by reading (or mutating) process env, which would
    // make this test order-dependent under the parallel test runner.
    let root = repo_root();
    let reg = vox_config::graphify::load_graphify_corpora(&root).expect("load contract");
    assert!(
        reg.ttl_days_default > 0,
        "contract ttl_days_default must be a real window"
    );
    assert!(
        !vox_config::graphify::ttl_env_override_active(None),
        "with the env var unset the contract must be in control"
    );
    assert!(
        vox_config::graphify::ttl_env_override_active(Some("7")),
        "a pinned env value must register as an override"
    );
}
