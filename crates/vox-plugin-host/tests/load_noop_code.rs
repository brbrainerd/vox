//! End-to-end: build noop-code (auto-built on demand), copy artifact + manifest
//! to a tempdir, discover, load, exercise the trait object.

use std::path::PathBuf;
use std::process::Command;
use vox_plugin_host::{Loader, discover};

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/vox-plugin-host -> crates/
    p.pop(); // crates/ -> repo root
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

/// Returns the path to the built dylib, building it on-demand if absent or stale.
///
/// The fixture is excluded from the workspace so it does not participate in
/// the main workspace build. Tests run after cargo releases the compile lock,
/// so a nested cargo invocation here does not deadlock.
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

    // Build on-demand.
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

    for profile in ["debug", "release"] {
        let p = root.join("target").join(profile).join(&filename);
        if p.exists() {
            return p;
        }
    }
    panic!("fixture dylib not found after build: {filename}");
}

#[test]
fn end_to_end_load_noop_code() {
    let dylib_src = built_dylib(
        "vox-plugin-noop-code",
        "crates/vox-plugin-host/tests/fixtures/noop-code",
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path().join("noop-code").join("0.1.0");
    std::fs::create_dir_all(&plugin_dir).expect("mkdir");

    let manifest_src = workspace_root()
        .join("crates")
        .join("vox-plugin-host")
        .join("tests")
        .join("fixtures")
        .join("noop-code")
        .join("Plugin.toml");
    std::fs::copy(&manifest_src, plugin_dir.join("Plugin.toml")).expect("copy manifest");
    let dylib_dest = plugin_dir.join(dylib_src.file_name().unwrap());
    std::fs::copy(&dylib_src, &dylib_dest).expect("copy dylib");

    let registry = discover(tmp.path()).expect("discover");
    assert!(
        registry.has("noop-code"),
        "expected noop-code in registry, got {:?}",
        registry.list_ids()
    );

    let loaded = Loader::load("noop-code", "0.1.0", &dylib_dest).expect("load");
    assert_eq!(loaded.plugin.id().as_str(), "noop-code");
    let _ = loaded.plugin.shutdown();
}
