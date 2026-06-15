//! Pins baseline file line counts so edits cannot happen silently.
//!
//! When the baseline legitimately shrinks (a finding is registered) or grows (a new exemption),
//! update the corresponding const and commit the change with an explanation.
use std::fs;
use std::path::Path;

fn non_comment_lines(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .count()
}

fn workspace_root() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is crates/vox-cli; workspace root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist")
}

#[test]
fn config_hygiene_baseline_has_pinned_count() {
    let path = workspace_root().join("contracts/config/config-hygiene-baseline.txt");
    let count = non_comment_lines(&path);
    // Update this number when the baseline legitimately shrinks (registered) or grows (new exemption).
    assert_eq!(
        count, EXPECTED_HYGIENE_COUNT,
        "config-hygiene-baseline.txt changed — update EXPECTED_HYGIENE_COUNT if intentional"
    );
}
const EXPECTED_HYGIENE_COUNT: usize = 209;

#[test]
fn config_registry_baseline_has_pinned_count() {
    let path = workspace_root().join("contracts/config/config-registry-baseline.txt");
    let count = non_comment_lines(&path);
    assert_eq!(
        count, EXPECTED_REGISTRY_COUNT,
        "config-registry-baseline.txt changed — update EXPECTED_REGISTRY_COUNT if intentional"
    );
}
const EXPECTED_REGISTRY_COUNT: usize = 698;
