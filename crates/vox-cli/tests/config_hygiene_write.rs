//! Tests for `vox ci config-hygiene --write` (Phase 5): auto-register stub rows
//! for unregistered env vars, and prune orphan rows whose env_var no longer
//! appears in source.

use std::path::PathBuf;
use vox_cli::commands::ci::config_hygiene::{WriteRegistryOpts, write_registry};

/// Build a minimal temp workspace:
///   <tmp>/crates/mypkg/src/lib.rs  — contains env reads
///   <tmp>/contracts/config/registry.v1.yaml — seed YAML
fn make_workspace(src: &str, registry_yaml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    // Create source file
    let src_dir = root.join("crates/mypkg/src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), src).unwrap();

    // Create registry
    let contracts = root.join("contracts/config");
    std::fs::create_dir_all(&contracts).unwrap();
    std::fs::write(contracts.join("registry.v1.yaml"), registry_yaml).unwrap();

    dir
}

fn read_registry(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("contracts/config/registry.v1.yaml")).unwrap()
}

const EMPTY_REGISTRY: &str = r#"schema_version: "1"
knobs: []
"#;

// ---------------------------------------------------------------------------

#[test]
fn write_appends_stub_row_for_new_api_key() {
    let src = r#"let k = std::env::var("NEW_API_KEY").ok();"#;
    let dir = make_workspace(src, EMPTY_REGISTRY);
    let root = dir.path().to_path_buf();

    let opts = WriteRegistryOpts { root: root.clone() };
    let result = write_registry(opts);
    assert!(result.is_ok(), "write_registry failed: {:?}", result.err());

    let yaml = read_registry(&root);
    assert!(
        yaml.contains("env_var: NEW_API_KEY"),
        "expected NEW_API_KEY row, got:\n{yaml}"
    );
    assert!(
        yaml.contains("secret: true"),
        "API_KEY suffix should be secret: true, got:\n{yaml}"
    );
    assert!(
        yaml.contains("status: declared"),
        "new row should have status: declared, got:\n{yaml}"
    );
}

#[test]
fn write_is_idempotent() {
    let src = r#"let v = std::env::var("VOX_SOME_KNOB").ok();"#;
    let dir = make_workspace(src, EMPTY_REGISTRY);
    let root = dir.path().to_path_buf();

    let opts1 = WriteRegistryOpts { root: root.clone() };
    write_registry(opts1).expect("first run");
    let after_first = read_registry(&root);

    let opts2 = WriteRegistryOpts { root: root.clone() };
    write_registry(opts2).expect("second run");
    let after_second = read_registry(&root);

    assert_eq!(
        after_first, after_second,
        "--write must be idempotent (second run must not change the file)"
    );
}

#[test]
fn write_prunes_orphan_row() {
    // Registry has a row for REMOVED_VAR that no longer appears in source.
    let registry_with_orphan = r#"schema_version: "1"
knobs:
  - name: removed_var
    env_var: REMOVED_VAR
    description: "was needed, now gone"
    owner_crate: vox-cli
    status: active
    secret: false
    bucket: third-party
    source: env
    since: "2026-01-01"
"#;
    // Source does NOT reference REMOVED_VAR.
    let src = r#"let x = 1;"#;
    let dir = make_workspace(src, registry_with_orphan);
    let root = dir.path().to_path_buf();

    let opts = WriteRegistryOpts { root: root.clone() };
    write_registry(opts).expect("write");

    let yaml = read_registry(&root);
    assert!(
        !yaml.contains("REMOVED_VAR"),
        "orphan row should have been pruned, got:\n{yaml}"
    );
}

#[test]
fn write_does_not_prune_deprecated_rows() {
    // Deprecated rows must be preserved even if env_var no longer in source.
    let registry_deprecated = r#"schema_version: "1"
knobs:
  - name: old_key
    env_var: OLD_DEPRECATED_KEY
    description: "kept for backward compat"
    owner_crate: vox-cli
    status: deprecated
    secret: false
    bucket: vox-knob
    source: env
    since: "2025-01-01"
"#;
    let src = r#"let x = 1;"#;
    let dir = make_workspace(src, registry_deprecated);
    let root = dir.path().to_path_buf();

    let opts = WriteRegistryOpts { root: root.clone() };
    write_registry(opts).expect("write");

    let yaml = read_registry(&root);
    assert!(
        yaml.contains("OLD_DEPRECATED_KEY"),
        "deprecated row must NOT be pruned, got:\n{yaml}"
    );
}

#[test]
fn write_infers_bucket_correctly() {
    // Use names that are NOT Clavis-managed to test bucket inference.
    let src = r#"
        let a = std::env::var("VOX_KNOB_ALPHA").ok();
        let b = std::env::var("THIRD_PARTY_TOKEN").ok();
    "#;
    let dir = make_workspace(src, EMPTY_REGISTRY);
    let root = dir.path().to_path_buf();

    let opts = WriteRegistryOpts { root: root.clone() };
    write_registry(opts).expect("write");

    let yaml = read_registry(&root);
    // Check that VOX_KNOB_ALPHA got bucket: vox-knob
    assert!(
        yaml.contains("vox-knob"),
        "VOX_ prefix → vox-knob, got:\n{yaml}"
    );
    assert!(
        yaml.contains("VOX_KNOB_ALPHA"),
        "VOX_KNOB_ALPHA missing from yaml:\n{yaml}"
    );

    // Check THIRD_PARTY_TOKEN got bucket: third-party and secret: true
    assert!(
        yaml.contains("THIRD_PARTY_TOKEN"),
        "THIRD_PARTY_TOKEN missing from yaml:\n{yaml}"
    );
    assert!(
        yaml.contains("third-party"),
        "non-VOX_ → third-party, got:\n{yaml}"
    );
    assert!(
        yaml.contains("secret: true"),
        "_TOKEN suffix → secret: true, got:\n{yaml}"
    );
}
