use std::path::{Path, PathBuf};

use super::{
    collect_legacy_script_glue_violations, nested_cargo_target_dir, resolve_mens_gate_manifest_path,
};
use crate::commands::ci::constants::FEATURE_SETS;

#[test]
fn feature_sets_include_script_execution_lane() {
    assert!(
        FEATURE_SETS.contains(&"script-execution"),
        "CI feature matrix must compile the script-execution lane"
    );
    assert!(
        FEATURE_SETS.contains(&"script-execution,stub-check"),
        "CI feature matrix must include a mixed script-execution + stub-check build"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — feature matrix lane until oratio dep stable in CI"]
fn feature_sets_include_populi_oratio_lane() {
    assert!(
        FEATURE_SETS.contains(&"oratio"),
        "CI feature matrix must compile the oratio (Oratio STT) lane"
    );
}

#[test]
fn canonical_mens_gate_manifest_exists_in_repo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let resolved = resolve_mens_gate_manifest_path(&root);
    assert!(
        resolved.ends_with(PathBuf::from("scripts/populi/gates.yaml")),
        "expected canonical mens gate manifest path to resolve first, got {}",
        resolved.display()
    );
    assert!(
        resolved.is_file(),
        "missing gate manifest: {}",
        resolved.display()
    );
}

#[test]
fn mens_gate_manifest_resolution_uses_legacy_fallback() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    std::fs::create_dir_all(root.join("scripts/mens")).expect("mkdir scripts/mens");
    std::fs::write(root.join("scripts/mens/gates.yaml"), "profiles: {}\n")
        .expect("write legacy gates");
    let resolved = resolve_mens_gate_manifest_path(root);
    assert!(
        resolved.ends_with(PathBuf::from("scripts/mens/gates.yaml")),
        "expected legacy fallback, got {}",
        resolved.display()
    );
}

#[test]
fn nested_cargo_target_uses_os_temp_nested_ci() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    let nested = nested_cargo_target_dir(root);
    assert!(
        nested.ends_with(PathBuf::from("nested-ci")),
        "unexpected nested target path: {}",
        nested.display()
    );
    let hash_dir = nested.parent().expect("hash segment");
    let vox_targets = hash_dir.parent().expect("vox-targets");
    assert_eq!(
        vox_targets.file_name().and_then(|s| s.to_str()),
        Some("vox-targets"),
        "expected …/vox-targets/<hash>/nested-ci, got {}",
        nested.display()
    );
}

#[test]
fn legacy_script_glue_scan_flags_stray_shell_under_scripts() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    std::fs::create_dir_all(root.join("scripts/ci")).expect("mkdir scripts/ci");
    std::fs::write(root.join("scripts/ci/bad_helper.sh"), "#!/bin/sh\necho\n").expect("write sh");
    let v = collect_legacy_script_glue_violations(root).expect("scan");
    assert_eq!(v.len(), 1, "{v:?}");
}

#[test]
fn legacy_script_glue_scan_respects_bootstrap_allowlist() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    std::fs::create_dir_all(root.join("scripts/windows")).expect("mkdir scripts/windows");
    std::fs::write(root.join("scripts/windows/vox-dev.ps1"), "forwarder\n").expect("write ps1");
    let v = collect_legacy_script_glue_violations(root).expect("scan");
    assert!(v.is_empty(), "{v:?}");
}
