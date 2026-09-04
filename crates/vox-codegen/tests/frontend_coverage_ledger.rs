//! Drift guard: the frontend coverage ledger must list exactly the surface
//! directories present under `crates/vox-gui/ui/src/components/surfaces/`.
//! Adding or removing a surface without updating the ledger fails CI — this is
//! what keeps the 95-99% denominator honest (spec Sub-project A / F).
//!
//! Placed in `vox-codegen` rather than `vox-gui` because `vox-gui`'s build.rs
//! requires the Tauri release binary, making it non-runnable in dev builds. This
//! test is pure filesystem reads and has no dep on either crate's runtime code.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/vox-codegen → workspace root is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn surfaces_dir() -> PathBuf {
    workspace_root().join("crates/vox-gui/ui/src/components/surfaces")
}

fn ledger_path() -> PathBuf {
    workspace_root().join("docs/superpowers/ledgers/frontend-coverage-ledger.md")
}

/// Directory names directly under `components/surfaces/`.
fn filesystem_surfaces() -> BTreeSet<String> {
    fs::read_dir(surfaces_dir())
        .expect("read surfaces dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        // `__guards__`/`__tests__` are cross-surface infra, not surfaces themselves.
        .filter(|name| !(name.starts_with("__") && name.ends_with("__")))
        .collect()
}

/// Surface names parsed from the first column of the ledger's markdown table.
/// A row is `| Name | status | notes |`; header/separator rows and the summary
/// section are skipped by requiring the second column to be a known status.
fn ledger_surfaces() -> BTreeSet<String> {
    const STATUSES: [&str; 5] = [
        "expressible",
        "blocked:reactive-streams",
        "blocked:interop",
        "blocked:mobile",
        "blocked:other",
    ];
    let text = fs::read_to_string(ledger_path()).expect("read ledger");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        // Leading/trailing '|' produce empty first/last cells → cols[1], cols[2].
        if cols.len() >= 4 {
            let name = cols[1];
            let status = cols[2];
            if STATUSES.contains(&status) && !name.is_empty() {
                out.insert(name.to_string());
            }
        }
    }
    out
}

#[test]
fn ledger_matches_filesystem_surfaces() {
    let fs_set = filesystem_surfaces();
    let ledger_set = ledger_surfaces();

    let missing_from_ledger: Vec<_> = fs_set.difference(&ledger_set).collect();
    let stale_in_ledger: Vec<_> = ledger_set.difference(&fs_set).collect();

    assert!(
        missing_from_ledger.is_empty(),
        "surfaces present on disk but missing a ledger row: {missing_from_ledger:?} \
         — add rows to docs/superpowers/ledgers/frontend-coverage-ledger.md"
    );
    assert!(
        stale_in_ledger.is_empty(),
        "ledger rows naming non-existent surface dirs: {stale_in_ledger:?} \
         — remove or rename them in the ledger"
    );
}
