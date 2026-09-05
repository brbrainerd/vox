#![allow(unsafe_code)] // test-only std::env::set_var (unsafe on edition 2024)

//! Plan acceptance check for P6-1: with `VOX_HOME` and `VOX_DATA_DIR` pointed at
//! temp dirs, every `vox-config` path resolver this crate owns must resolve
//! *inside* one of those two roots — never outside them, never to the real
//! `$HOME`.
//!
//! This file has exactly one `#[test]` on purpose: `std::env::set_var` is process-
//! global and, under edition 2024, `unsafe`. A single test per binary means no other
//! test in this process can observe or race the mutation (integration test files are
//! separate processes under `cargo test`, so this does not affect `cross_module_smoke.rs`
//! or any other test file).
//!
//! ## What this proves
//!
//! - `paths::dot_vox_user_dir()` (and everything built on it: `script_cache_dir` for
//!   both the native and wasi targets) resolves under `VOX_HOME`, not `$HOME`.
//! - `paths::data_dir()` and everything built on it (`default_db_path`, `state_dir`,
//!   `config_dir`) resolve under `VOX_DATA_DIR`, and the directories those functions
//!   create (`state_dir`/`config_dir` call `create_dir_all`) land only inside that
//!   root — nothing appears in the process's real home directory as a side effect of
//!   calling them.
//!
//! ## What this does NOT prove (the documented partial-relocation limitation)
//!
//! `dot_vox_user_dir` is one of at least two independent `~/.vox` resolvers in this
//! workspace (see its rustdoc in `paths.rs`). This test only exercises the resolver
//! this crate owns. It does **not** exercise, and cannot prove anything about:
//!
//! - `vox-secrets`'s own `$HOME/.vox` resolver (`auth_json.rs::vox_dir()`), which does
//!   not consult `VOX_HOME` and is not called from this crate.
//! - The roughly thirty other home-relative `.vox` joins in `vox-cli`, `vox-gui`,
//!   `vox-cli-core`, `vox-runtime`, `vox-plugin-host`, and `voxup`, none of which this
//!   crate depends on or can invoke.
//!
//! So a green run here means "the part of the tree that goes through
//! `vox_config::paths` is contained" — not "nothing Vox writes touches the real
//! `$HOME/.vox`". That stronger claim is explicitly out of scope for this task.

use std::path::Path;

#[test]
fn vox_home_and_data_dir_contain_every_resolver_this_crate_owns() {
    let home_root = tempfile::tempdir().expect("tempdir for VOX_HOME");
    let data_root = tempfile::tempdir().expect("tempdir for VOX_DATA_DIR");

    // Safety: this is the only #[test] in this binary (a separate process under
    // `cargo test`), so no other test can observe or race this mutation.
    unsafe {
        std::env::set_var("VOX_HOME", home_root.path());
        std::env::set_var("VOX_DATA_DIR", data_root.path());
    }

    // --- VOX_HOME-rooted resolvers ---
    let dot_vox = vox_config::paths::dot_vox_user_dir();
    assert_eq!(dot_vox, home_root.path());

    let native_cache = vox_config::paths::script_cache_dir(false);
    assert!(
        native_cache.starts_with(home_root.path()),
        "script_cache_dir(false) = {native_cache:?} escaped VOX_HOME"
    );
    assert!(native_cache.ends_with("script-cache"));

    let wasi_cache = vox_config::paths::script_cache_dir(true);
    assert!(
        wasi_cache.starts_with(home_root.path()),
        "script_cache_dir(true) = {wasi_cache:?} escaped VOX_HOME"
    );
    assert!(wasi_cache.ends_with("script-cache-wasi"));

    // dot_vox_user_dir() itself must not have created anything (documented as a pure
    // resolver — callers create_dir_all before writing).
    assert!(
        !dot_vox.join("script-cache").exists(),
        "dot_vox_user_dir()/script_cache_dir() must not create directories as a side effect"
    );

    // --- VOX_DATA_DIR-rooted resolvers ---
    let data_dir = vox_config::paths::data_dir().expect("VOX_DATA_DIR is set");
    assert_eq!(data_dir, data_root.path());
    assert!(data_dir.starts_with(data_root.path()));

    let db_path = vox_config::paths::default_db_path().expect("data_dir resolved");
    assert!(
        db_path.starts_with(data_root.path()),
        "default_db_path() = {db_path:?} escaped VOX_DATA_DIR"
    );

    let state_dir = vox_config::paths::state_dir().expect("data_dir resolved");
    assert!(state_dir.starts_with(data_root.path()));
    assert!(
        state_dir.is_dir(),
        "state_dir() must create the directory under VOX_DATA_DIR"
    );

    let config_dir = vox_config::paths::config_dir().expect("data_dir resolved");
    assert!(config_dir.starts_with(data_root.path()));
    assert!(
        config_dir.is_dir(),
        "config_dir() must create the directory under VOX_DATA_DIR"
    );

    // Containment, stated the other way round: every resolved/created path is a
    // descendant of one of the two roots, never a sibling or ancestor escape (e.g. via
    // a "../" component or an unrelated absolute path).
    for p in [&dot_vox, &native_cache, &wasi_cache] {
        assert!(is_contained_in(p, home_root.path()));
    }
    for p in [&data_dir, &db_path, &state_dir, &config_dir] {
        assert!(is_contained_in(p, data_root.path()));
    }
}

fn is_contained_in(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}
