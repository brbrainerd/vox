//! Contributor-mode detection (spec §9.1 exemption).
//!
//! Spec §9.1's rule: no runtime code path may invoke `cargo`, and no
//! user-facing remediation may reference a repo-relative path, **unless it is
//! gated behind an explicit "contributor mode" detected from the presence of
//! the workspace itself.**
//!
//! This module is that detector. It must work without invoking `cargo` —
//! an installed user has no Rust toolchain, so a detector that shells out to
//! `cargo` to decide whether cargo exists would be circular. Instead it walks
//! up the directory tree from a start path looking for a `Cargo.toml` that
//! declares a `[workspace]` table and has a sibling `crates/vox-cli/`
//! directory (i.e. this repository's workspace root, not just any crate's
//! manifest).

use std::path::{Path, PathBuf};

/// Walk up from `start` looking for the Vox workspace root: a directory
/// containing both a `Cargo.toml` with a `[workspace]` table and a
/// `crates/vox-cli/` subdirectory.
///
/// Returns `None` once the filesystem root is reached without a match.
/// Takes an explicit start path (rather than reading `cwd` itself) so it is
/// testable without mutating process-global state.
pub fn locate_workspace_root_from(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        let crates_vox_cli = d.join("crates").join("vox-cli");
        if crates_vox_cli.is_dir() {
            if let Ok(contents) = std::fs::read_to_string(&manifest) {
                if contents.contains("[workspace]") {
                    return Some(d.to_path_buf());
                }
            }
        }
        dir = d.parent();
    }
    None
}

/// Locate the Vox workspace root starting from the current working directory.
pub fn locate_workspace_root() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| locate_workspace_root_from(&cwd))
}

/// True when running from inside a Vox source checkout (a "contributor" —
/// audience B, who has a checkout and therefore a Rust toolchain, by
/// definition). False for an installed end user (audience A/C) with no
/// workspace above their current directory.
#[must_use]
pub fn is_contributor_mode() -> bool {
    locate_workspace_root().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_workspace_root_from_inside_this_worktree() {
        let here = std::env::current_dir().expect("cwd");
        assert!(
            locate_workspace_root_from(&here).is_some(),
            "expected to detect the Vox workspace root from {}",
            here.display()
        );
    }

    #[test]
    fn returns_none_for_a_bare_temp_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // A bare temp dir (no Cargo.toml, no crates/vox-cli/ above it) must not
        // be misdetected as a workspace root. Note: this assumes the OS temp
        // root itself isn't nested inside a Vox checkout, which holds on every
        // supported CI/dev environment.
        assert_eq!(locate_workspace_root_from(tmp.path()), None);
    }

    #[test]
    fn requires_both_workspace_table_and_crates_vox_cli_sibling() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Cargo.toml without [workspace] table, even with a crates/vox-cli/ dir,
        // must not match.
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")
            .expect("write Cargo.toml");
        std::fs::create_dir_all(tmp.path().join("crates").join("vox-cli"))
            .expect("mkdir crates/vox-cli");
        assert_eq!(locate_workspace_root_from(tmp.path()), None);
    }

    #[test]
    fn matches_when_both_conditions_hold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .expect("write Cargo.toml");
        std::fs::create_dir_all(tmp.path().join("crates").join("vox-cli"))
            .expect("mkdir crates/vox-cli");
        assert_eq!(
            locate_workspace_root_from(tmp.path()),
            Some(tmp.path().to_path_buf())
        );
    }
}
