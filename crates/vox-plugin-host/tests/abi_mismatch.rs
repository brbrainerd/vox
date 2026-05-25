//! End-to-end: load the deliberately-mismatched bad-abi dylib, assert the
//! loader returns AbiMismatch and the plugin_abi field is the bad value.

use std::path::PathBuf;
use std::process::Command;
use vox_plugin_host::{Loader, errors::LoadError};

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn dylib_filename(crate_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{}.dll", crate_name.replace('-', "_"))
    } else if cfg!(target_os = "macos") {
        format!("lib{}.dylib", crate_name.replace('-', "_"))
    } else {
        format!("lib{}.so", crate_name.replace('-', "_"))
    }
}

/// Returns the path to the built dylib, building it on-demand if absent.
///
/// The fixture is excluded from the workspace Cargo.toml so it does not
/// participate in the main workspace build. We build it lazily here — tests
/// run after cargo releases the compile lock, so the nested cargo invocation
/// does not deadlock.
fn built_dylib(crate_name: &str, fixture_rel: &str) -> PathBuf {
    let root = workspace_root();
    let filename = dylib_filename(crate_name);
    for profile in ["debug", "release"] {
        let p = root.join("target").join(profile).join(&filename);
        if p.exists() {
            return p;
        }
    }
    // Not yet built — build it now.  Tests execute after compilation, so this
    // nested cargo does not contend with the outer cargo's target-dir lock.
    let manifest_path = root.join(fixture_rel).join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let target_dir = root.join("target");
    let status = Command::new(cargo)
        .args(["build", "--manifest-path", manifest_path.to_str().unwrap()])
        .env("CARGO_TARGET_DIR", target_dir.to_str().unwrap())
        .status()
        .expect("failed to spawn cargo");
    assert!(status.success(), "fixture build failed: {}", manifest_path.display());

    // Return whichever profile was just produced.
    for profile in ["debug", "release"] {
        let p = root.join("target").join(profile).join(&filename);
        if p.exists() {
            return p;
        }
    }
    panic!("fixture dylib not found after build: {filename}");
}

#[test]
fn rejects_mismatched_abi() {
    let dylib = built_dylib(
        "vox-plugin-noop-code-bad-abi",
        "crates/vox-plugin-host/tests/fixtures/noop-code-bad-abi",
    );
    let result = Loader::load("noop-bad-abi", "0.1.0", &dylib);
    match result {
        Err(LoadError::AbiMismatch(e)) => {
            assert_eq!(e.plugin_abi, 999_999);
            assert_eq!(e.host_abi, 12);
        }
        Ok(_) => panic!("expected AbiMismatch, got Ok"),
        Err(other) => panic!("expected AbiMismatch, got {other:?}"),
    }
}
