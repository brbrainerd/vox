//! The shim must transparently delegate to the real cargo for non-build
//! subcommands and for invocations outside any vox worktree.

use std::process::Command;

fn shim_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_cargo"))
}

#[test]
fn non_build_subcommand_passes_through_to_real_cargo() {
    let out = Command::new(shim_bin()).arg("--version").output().unwrap();
    assert!(out.status.success(), "shim --version failed: {out:?}");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.starts_with("cargo "),
        "expected real cargo version, got: {s}"
    );
}

#[test]
fn outside_worktree_passes_through() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(shim_bin())
        .arg("--version")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
}
