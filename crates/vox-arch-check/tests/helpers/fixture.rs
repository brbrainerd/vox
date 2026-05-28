//! Builds a minimal synthetic workspace for arch-check integration tests.
//!
//! Wraps [`vox_test_harness::synthetic_workspace::SyntheticWorkspaceBuilder`]
//! with the extras that arch-check expects on top of a generic workspace:
//! `docs/src/architecture/layers.toml` and `where-things-live.md`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use vox_test_harness::synthetic_workspace::{
    MemberSpec, SyntheticWorkspace, SyntheticWorkspaceBuilder,
};

const LONG_DESCRIPTION: &str =
    "A fixture crate for testing the arch-check rules with enough characters.";

pub struct ArchCheckFixture {
    ws: SyntheticWorkspace,
}

impl ArchCheckFixture {
    /// Minimal clean workspace — both crates pass all checks.
    pub fn clean() -> Self {
        Self::new_with(LONG_DESCRIPTION)
    }

    /// Workspace with a description violation on vox-beta (too short).
    pub fn with_description_violation() -> Self {
        Self::new_with("short")
    }

    fn new_with(beta_description: &str) -> Self {
        let layers_toml = "[crates.vox-alpha]\nlayer = 0\n\n[crates.vox-beta]\nlayer = 1\n";
        let wtl_md = format!(
            "# WTL\n\n`crates/vox-alpha/` — {LONG_DESCRIPTION}\n`crates/vox-beta/` — {beta_description}\n"
        );
        let ws = SyntheticWorkspaceBuilder::new()
            .member(MemberSpec::library("vox-alpha").with_description(LONG_DESCRIPTION))
            .member(
                MemberSpec::library("vox-beta")
                    .with_description(beta_description)
                    .with_dep("vox-alpha"),
            )
            .with_git_stub()
            .with_changelog("0.1.0", "2020-01-01")
            .with_extra_file("docs/src/architecture/layers.toml", layers_toml)
            .with_extra_file("docs/src/architecture/where-things-live.md", wtl_md)
            .build()
            .expect("build arch-check fixture");
        Self { ws }
    }

    pub fn root(&self) -> &Path {
        self.ws.root()
    }

    /// Path to the built vox-arch-check binary. Relies on cargo/nextest having
    /// built it before tests run (which both do for binary targets in the same
    /// crate). We do NOT call `cargo build` here — each fixture test runs in
    /// its own process under nextest, and parallel `cargo build` calls race
    /// for the same target/debug/*.exe lock on Windows.
    pub fn binary() -> PathBuf {
        static BIN: OnceLock<PathBuf> = OnceLock::new();
        BIN.get_or_init(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let ws_root = manifest_dir.parent().unwrap().parent().unwrap();
            let exe = if cfg!(windows) {
                ws_root.join("target/debug/vox-arch-check.exe")
            } else {
                ws_root.join("target/debug/vox-arch-check")
            };
            assert!(
                exe.exists(),
                "vox-arch-check binary not found at {} (nextest should have built it)",
                exe.display()
            );
            exe
        })
        .clone()
    }
}
