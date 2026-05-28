//! Wave 1 Tauri migration: scaffold templates include `@tauri-apps/api`; init hints path is stable.

use serde_json::Value;
use vox_cli::commands::init::TAURI_PACKAGING_HINT_README_REL;
use vox_cli::templates;

#[test]
fn tauri_packaging_readme_path_is_project_root_not_target_generated() {
    assert_eq!(
        TAURI_PACKAGING_HINT_README_REL,
        "tauri-packaging/README.md",
        "must match vox_tauri_codegen output dir (project root / out_root + tauri-packaging/README.md)"
    );
    assert!(
        !TAURI_PACKAGING_HINT_README_REL.contains("target/"),
        "legacy init text incorrectly pointed under target/generated/"
    );
}

#[test]
fn spa_package_json_includes_tauri_apps_api_spa_and_tanstack_start() {
    for (label, json_str) in [
        ("spa", templates::package_json(false, false)),
        ("tanstack_start", templates::package_json(true, false)),
    ] {
        let v: Value = serde_json::from_str(&json_str).unwrap_or_else(|e| {
            panic!("{label} package_json must parse as JSON: {e}\n{json_str}")
        });
        let deps = v
            .get("dependencies")
            .and_then(|d| d.as_object())
            .unwrap_or_else(|| panic!("{label}: missing dependencies object in {v:?}"));
        let ver = deps
            .get("@tauri-apps/api")
            .unwrap_or_else(|| panic!("{label}: missing @tauri-apps/api in {deps:?}"));
        assert!(
            ver.as_str().is_some_and(|s| s.starts_with('^')),
            "{label}: @tauri-apps/api should be a semver range: {ver:?}"
        );
    }
}
