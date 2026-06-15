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
        // vox-arch-check: allow dynlib-ext
        format!("{}.dll", crate_name.replace('-', "_"))
    } else if cfg!(target_os = "macos") {
        // vox-arch-check: allow dynlib-ext
        format!("lib{}.dylib", crate_name.replace('-', "_"))
    } else {
        // vox-arch-check: allow dynlib-ext
        format!("lib{}.so", crate_name.replace('-', "_"))
    }
}

/// Returns the path to the built dylib, building it on-demand if absent or stale.
///
/// The fixture is excluded from the workspace Cargo.toml so it does not
/// participate in the main workspace build. We build it lazily here — tests
/// run after cargo releases the compile lock, so the nested cargo invocation
/// does not deadlock.
///
/// **Cache key:** file existence *plus* staleness relative to the workspace root
/// `Cargo.toml`.  The fixtures depend on `vox-plugin-api` via a workspace path
/// dep and that crate's version is inherited from `workspace.package.version` in
/// the root `Cargo.toml`.  When the workspace is bumped (e.g. 0.5 → 0.6) the
/// root `Cargo.toml` is modified, making any cached dylib from the previous
/// version stale.  Without this check the tests silently load a dylib compiled
/// against the old ABI and fail with "expected AbiMismatch, got InitFailed"
/// instead of the expected pass/fail outcome.
fn built_dylib(crate_name: &str, fixture_rel: &str) -> PathBuf {
    let root = workspace_root();
    let filename = dylib_filename(crate_name);

    // Modification time of the workspace root Cargo.toml — updated on every
    // version bump.  If we can't read it we fall back to the existence-only check.
    let workspace_toml_mtime = root
        .join("Cargo.toml")
        .metadata()
        .and_then(|m| m.modified())
        .ok();

    for profile in ["debug", "release"] {
        let p = root.join("target").join(profile).join(&filename);
        if p.exists() {
            // Invalidate if the workspace Cargo.toml is newer than the dylib.
            let stale = workspace_toml_mtime
                .and_then(|ws_t| {
                    p.metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .map(|dylib_t| ws_t > dylib_t)
                })
                .unwrap_or(false);
            if !stale {
                return p;
            }
            // Stale (workspace version bumped) — fall through to rebuild.
        }
    }

    // Not yet built (or stale) — build it now.  Tests execute after compilation,
    // so this nested cargo does not contend with the outer cargo's target-dir lock.
    let manifest_path = root.join(fixture_rel).join("Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let target_dir = root.join("target");
    let status = Command::new(cargo)
        .args(["build", "--manifest-path", manifest_path.to_str().unwrap()])
        .env("CARGO_TARGET_DIR", target_dir.to_str().unwrap())
        .status()
        .expect("failed to spawn cargo");
    assert!(
        status.success(),
        "fixture build failed: {}",
        manifest_path.display()
    );

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
