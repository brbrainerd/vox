//! `vox plugin list` — print all catalog entries with installed/available status.

use anyhow::Result;
use std::path::PathBuf;

/// Resolve the plugin install root:
/// `$VOX_PLUGINS_DIR` env override, else `<data_local_dir>/vox/plugins`.
///
/// Delegates to [`vox_plugin_host::resolve_plugins_root`] so the logic lives
/// in one place and `DefaultVoxHost` and the CLI always agree on the root.
pub fn plugins_root() -> PathBuf {
    vox_plugin_host::resolve_plugins_root()
}

/// Returns the versioned install dir for a given id: the NEWEST version
/// subdirectory under `<root>/<id>/`, or `<root>/<id>/<version>` when
/// `version` is known.
///
/// Multiple version directories can coexist under one plugin id (an upgrade
/// that doesn't prune the old version, a manual install alongside an
/// existing one). Picking "newest" deterministically, rather than whatever
/// `read_dir` happens to yield first, matters because every caller
/// (`vox plugin list/info/doctor/publish`, `plugin_bundle::apply`) treats
/// this as THE installed version — a filesystem/OS-order-dependent answer
/// would make those commands nondeterministic across runs and platforms.
///
/// Ordering: directory names that parse as [`semver::Version`] sort by
/// semver (newest first); names that don't parse sort after all semver
/// names (a plugin should always have a valid version dir, but a foreign or
/// half-written directory must not panic or silently win over a real one),
/// with ties broken lexicographically descending so the choice is still
/// deterministic among several non-semver names.
pub fn installed_version(root: &std::path::Path, id: &str) -> Option<String> {
    let id_dir = root.join(id);
    if !id_dir.is_dir() {
        return None;
    }
    let mut names: Vec<String> = std::fs::read_dir(&id_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort_by(|a, b| newest_first(a, b));
    names.into_iter().next()
}

/// Comparator for [`installed_version`]: semver-parseable names sort newest
/// first; a name that fails to parse sorts after every name that does.
fn newest_first(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => vb.cmp(&va),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => b.cmp(a),
    }
}

pub fn run() -> Result<()> {
    let root = plugins_root();
    let plugins = vox_plugin_catalog::all_plugins();

    // Header
    println!("{:<30} {:<11} {:<12} DESCRIPTION", "ID", "KIND", "STATUS");
    println!("{}", "-".repeat(90));

    for p in plugins {
        let kind = format!("{:?}", p.payload_kind).to_lowercase();
        let status = match installed_version(&root, &p.id) {
            Some(v) => format!("installed ({})", v),
            None => {
                // Check if this host OS/arch is covered by any artifact declared in the catalog.
                // For catalog entries we don't have full payload data, so just report "available".
                "available".to_string()
            }
        };
        println!("{:<30} {:<11} {:<12} {}", p.id, kind, status, p.description);
    }
    println!();
    println!("Install root: {}", root.display());
    Ok(())
}

#[cfg(test)]
mod installed_version_tests {
    use super::*;

    fn make_version_dir(root: &std::path::Path, id: &str, version: &str) {
        std::fs::create_dir_all(root.join(id).join(version)).unwrap();
    }

    #[test]
    fn no_id_dir_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(installed_version(tmp.path(), "nonexistent"), None);
    }

    #[test]
    fn empty_id_dir_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("some-plugin")).unwrap();
        assert_eq!(installed_version(tmp.path(), "some-plugin"), None);
    }

    #[test]
    fn single_version_dir_is_returned() {
        let tmp = tempfile::tempdir().unwrap();
        make_version_dir(tmp.path(), "some-plugin", "1.2.3");
        assert_eq!(
            installed_version(tmp.path(), "some-plugin"),
            Some("1.2.3".to_string())
        );
    }

    /// The regression this guards against: with multiple version dirs
    /// present, the result must be the same every time, not whatever
    /// `read_dir` happened to yield first on this run/platform.
    #[test]
    fn multiple_version_dirs_deterministically_pick_the_newest_semver() {
        let tmp = tempfile::tempdir().unwrap();
        // Insertion order deliberately does NOT match version order, so a
        // fix that just returns `read_dir`'s first entry would likely (and
        // nondeterministically) pick the wrong one.
        make_version_dir(tmp.path(), "some-plugin", "1.2.3");
        make_version_dir(tmp.path(), "some-plugin", "10.0.0");
        make_version_dir(tmp.path(), "some-plugin", "2.0.0");
        for _ in 0..5 {
            assert_eq!(
                installed_version(tmp.path(), "some-plugin"),
                Some("10.0.0".to_string()),
                "must deterministically pick the newest semver, every time"
            );
        }
    }

    #[test]
    fn non_semver_names_never_beat_a_real_version() {
        let tmp = tempfile::tempdir().unwrap();
        make_version_dir(tmp.path(), "some-plugin", "not-a-version");
        make_version_dir(tmp.path(), "some-plugin", "1.0.0");
        assert_eq!(
            installed_version(tmp.path(), "some-plugin"),
            Some("1.0.0".to_string())
        );
    }

    #[test]
    fn files_alongside_version_dirs_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        make_version_dir(tmp.path(), "some-plugin", "1.0.0");
        std::fs::write(
            tmp.path().join("some-plugin").join("README.txt"),
            b"not a version dir",
        )
        .unwrap();
        assert_eq!(
            installed_version(tmp.path(), "some-plugin"),
            Some("1.0.0".to_string())
        );
    }
}
