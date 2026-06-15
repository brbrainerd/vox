//! Task 2.6 / R9: the silent-drop gate must FIRE on a new catch-all-swallow site and
//! stay silent on clean code — proving catch_all_swallow + cross_crate_dup are
//! gate-blocking (count-based, severity untouched).
use std::fs;
use std::path::Path;
use vox_audit::core_gates::run_silent_drop_gate;

fn write_file(p: &Path, body: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

#[test]
fn gate_fires_on_catch_all_swallow() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    write_file(
        &root.join("crates/a/src/bad.rs"),
        "enum Kind { A, B }\npub fn f(k: Kind) -> Option<i32> {\n    match k {\n        Kind::A => Some(1),\n        _ => None,\n    }\n}\n",
    );
    let res = run_silent_drop_gate(root, None);
    assert!(
        !res.ok,
        "gate must FAIL on a catch-all-swallow site; detail={:?}",
        res.detail
    );
}

#[test]
fn gate_silent_on_clean_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    write_file(
        &root.join("crates/a/src/clean.rs"),
        "enum Kind { A, B }\npub fn g(k: Kind) -> i32 {\n    match k {\n        Kind::A => 1,\n        Kind::B => 2,\n    }\n}\n",
    );
    let res = run_silent_drop_gate(root, None);
    assert!(res.ok, "clean workspace must PASS; detail={:?}", res.detail);
}
