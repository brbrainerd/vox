//! Parity between `assets/skills/<name>/SKILL.md` on disk and `[[skill-bundle]]`
//! rows in `catalog.toml`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use vox_plugin_catalog::all_skill_bundles;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn bundled_skill_dirs_on_disk() -> BTreeSet<String> {
    let root = repo_root().join("assets/skills");
    let mut names = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return names;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && path.join("SKILL.md").is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            names.insert(name.to_string());
        }
    }
    names
}

#[test]
fn every_assets_skill_has_catalog_skill_bundle_entry() {
    let on_disk = bundled_skill_dirs_on_disk();
    assert!(
        !on_disk.is_empty(),
        "assets/skills must contain at least one SKILL.md directory"
    );

    let catalog_ids: BTreeSet<&str> = all_skill_bundles().iter().map(|e| e.id.as_str()).collect();

    for name in &on_disk {
        assert!(
            catalog_ids.contains(name.as_str()),
            "assets/skills/{name} missing [[skill-bundle]] in catalog.toml"
        );
    }
}

#[test]
fn every_catalog_skill_bundle_has_assets_skill_md() {
    let on_disk = bundled_skill_dirs_on_disk();
    for entry in all_skill_bundles() {
        assert!(
            on_disk.contains(&entry.id),
            "catalog skill-bundle '{}' has no assets/skills/{}/SKILL.md",
            entry.id,
            entry.id
        );
        assert_eq!(
            entry.bundle_path,
            format!("assets/skills/{}", entry.id),
            "skill-bundle '{}' bundle-path must match id",
            entry.id
        );
    }
}
