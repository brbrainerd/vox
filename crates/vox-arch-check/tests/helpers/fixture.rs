//! Builds a minimal synthetic workspace for arch-check integration tests.
//!
//! Creates a temp dir with:
//! - `Cargo.toml` (workspace with 2 members: vox-alpha, vox-beta)
//! - `crates/vox-alpha/` and `crates/vox-beta/` with minimal source
//! - `docs/src/architecture/layers.toml` and `where-things-live.md`
//! - `CHANGELOG.md` with a released version for the staleness rule
//! - `.git/` stub so git commands don't error

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tempfile::TempDir;

pub struct ArchCheckFixture {
    pub dir: TempDir,
}

impl ArchCheckFixture {
    /// Minimal clean workspace — both crates pass all checks.
    pub fn clean() -> Self {
        let dir = TempDir::new().expect("tempdir");
        Self::write_all(dir.path(), false);
        Self { dir }
    }

    /// Workspace with a description violation on vox-beta (too short).
    pub fn with_description_violation() -> Self {
        let dir = TempDir::new().expect("tempdir");
        Self::write_all(dir.path(), true);
        Self { dir }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
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

    fn write_all(root: &Path, short_description: bool) {
        use std::fs;
        let long_desc = "A fixture crate for testing the arch-check rules with enough characters.";
        let beta_desc = if short_description {
            "short"
        } else {
            long_desc
        };

        // Workspace Cargo.toml
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/vox-alpha\", \"crates/vox-beta\"]\nresolver = \"2\"\n").unwrap();

        // Cargo.lock stub (needed for cache key)
        fs::write(root.join("Cargo.lock"), "# workspace lock\n").unwrap();

        // CHANGELOG.md with one released version (for staleness rule)
        fs::write(
            root.join("CHANGELOG.md"),
            "## [0.1.0] - 2020-01-01\n\n- initial\n",
        )
        .unwrap();

        // .git stub — prevents git commands from walking up to the real repo
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

        // docs/src/architecture/
        fs::create_dir_all(root.join("docs/src/architecture")).unwrap();
        fs::write(
            root.join("docs/src/architecture/layers.toml"),
            "[crates.vox-alpha]\nlayer = 0\n\n[crates.vox-beta]\nlayer = 1\n",
        )
        .unwrap();
        fs::write(
            root.join("docs/src/architecture/where-things-live.md"),
            format!(
                "# WTL\n\n`crates/vox-alpha/` — {long_desc}\n`crates/vox-beta/` — {beta_desc}\n"
            ),
        )
        .unwrap();

        // vox-alpha
        fs::create_dir_all(root.join("crates/vox-alpha/src")).unwrap();
        fs::write(root.join("crates/vox-alpha/Cargo.toml"), format!("[package]\nname = \"vox-alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"{long_desc}\"\n")).unwrap();
        fs::write(
            root.join("crates/vox-alpha/src/lib.rs"),
            "//! Alpha crate.\npub fn alpha() {}\n",
        )
        .unwrap();

        // vox-beta
        fs::create_dir_all(root.join("crates/vox-beta/src")).unwrap();
        fs::write(root.join("crates/vox-beta/Cargo.toml"), format!("[package]\nname = \"vox-beta\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"{beta_desc}\"\n\n[dependencies]\nvox-alpha = {{ path = \"../vox-alpha\" }}\n")).unwrap();
        fs::write(
            root.join("crates/vox-beta/src/lib.rs"),
            "//! Beta crate.\npub fn beta() {}\n",
        )
        .unwrap();
    }
}
