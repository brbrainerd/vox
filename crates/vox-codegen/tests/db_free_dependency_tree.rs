//! Guards the `vox-codegen` -> `vox-sql` -> `vox-db` coupling that Task 2 of
//! P7 ("Lean Core & Persona Split") severed.
//!
//! `vox-codegen` only needs `vox-sql`'s pure dialect/DDL surface (`BackendKind`,
//! `SqlDialect`, `build::*`) to emit dialect-correct SQL strings — it never
//! opens a connection. `vox-sql` gates its live-backend runtime (and the
//! `vox-db`/`turso`/`sqlx` dependencies that come with it) behind a
//! default-ON `runtime` Cargo feature so that `vox-codegen` (and therefore
//! `vox-langtool`, which is advertised as a DB-free CLI for writing in the
//! Vox language) can opt out of it while keeping the `postgres`/`mysql`
//! dialect selectors it does need. See spec §9, audience A, and
//! `crates/vox-sql/Cargo.toml` for the feature declaration.
//!
//! `cargo tree -p X` roots feature resolution at package `X`; it is not
//! workspace-unified, so `-p` is a valid tool for an absence assertion even
//! though other workspace members (e.g. `vox-cli`) enable `vox-sql`'s default
//! `runtime` feature elsewhere.

use std::process::Command;

/// Runs `cargo tree` for the given package/args, rooted at the workspace
/// manifest. Returns `None` (after `eprintln!`) if `cargo` could not be
/// spawned at all, so this test degrades gracefully in an environment with no
/// cargo rather than panicking on an unrelated infrastructure gap.
fn run_cargo_tree(args: &[&str]) -> Option<String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // Workspace root is one level up from `crates/vox-codegen`.
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(std::path::Path::parent)
        .expect("CARGO_MANIFEST_DIR should be crates/vox-codegen under the workspace root");

    let output = match Command::new(env!("CARGO"))
        .current_dir(workspace_root)
        .arg("tree")
        .args(args)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            eprintln!("skipping db-free dependency tree test: could not spawn `cargo tree`: {err}");
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "`cargo tree {args:?}` did not succeed (status: {:?}); stderr:\n{stderr}",
            output.status.code()
        );
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn vox_langtool_dependency_tree_excludes_vox_db() {
    let Some(stdout) = run_cargo_tree(&["-p", "vox-langtool", "-e", "normal"]) else {
        return;
    };
    assert!(
        !stdout.contains("vox-db"),
        "vox-langtool must not transitively depend on vox-db (it is advertised as a \
         DB-free CLI for writing in the Vox language); full `cargo tree -p vox-langtool \
         -e normal` output:\n{stdout}"
    );
    eprintln!(
        "OK: vox-langtool dependency tree ({} bytes) contains no vox-db",
        stdout.len()
    );
}

#[test]
fn vox_codegen_dependency_tree_excludes_vox_db() {
    let Some(stdout) = run_cargo_tree(&["-p", "vox-codegen", "-e", "normal"]) else {
        return;
    };
    assert!(
        !stdout.contains("vox-db"),
        "vox-codegen must not transitively depend on vox-db; it only needs vox-sql's pure \
         dialect surface (BackendKind, SqlDialect, build::*), not a live database backend; \
         full `cargo tree -p vox-codegen -e normal` output:\n{stdout}"
    );
    eprintln!(
        "OK: vox-codegen dependency tree ({} bytes) contains no vox-db",
        stdout.len()
    );
}

/// Positive control: without this, a typo in "vox-db" above would make both
/// negative assertions vacuously true. `vox-cli` still depends on `vox-sql`
/// with its default (`runtime`-on) features, so the edge must still exist
/// there.
#[test]
fn vox_cli_dependency_tree_still_reaches_vox_db() {
    let Some(stdout) = run_cargo_tree(&["-p", "vox-cli", "-e", "normal"]) else {
        return;
    };
    assert!(
        stdout.contains("vox-db"),
        "vox-cli must still reach vox-db via vox-sql's default `runtime` feature, or this \
         test suite's negative assertions are vacuous; full `cargo tree -p vox-cli -e normal` \
         output:\n{stdout}"
    );
    eprintln!(
        "OK: vox-cli dependency tree ({} bytes) contains vox-db",
        stdout.len()
    );
}
