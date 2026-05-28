use std::fs;
use std::path::Path;
use tempfile::tempdir;
use vox_orchestrator::{AffinityGroupRegistry, load_from_config};
use vox_test_harness::synthetic_workspace::{MemberSpec, SyntheticWorkspaceBuilder};

#[test]
fn test_default_affinity_resolution() {
    let reg = AffinityGroupRegistry::defaults();

    let p1 = Path::new("crates/vox-package/src/main.rs");
    let g1 = reg.resolve(p1).unwrap();
    assert_eq!(g1.name, "pm-group");

    let p2 = Path::new("crates/vox-compiler/src/lexer/mod.rs");
    let g2 = reg.resolve(p2).unwrap();
    assert_eq!(g2.name, "lexer-parser-group");
}

#[test]
fn test_config_load_override() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("Vox.toml");
    fs::write(
        &config_path,
        r#"
[[affinity_groups]]
name = "custom"
patterns = ["custom/**", "src/extra/*.rs"]
"#,
    )
    .unwrap();

    let reg = load_from_config(&config_path).unwrap();
    assert_eq!(reg.groups().len(), 1);
    assert_eq!(reg.groups()[0].name, "custom");

    assert!(reg.resolve(Path::new("custom/file.vox")).is_some());
    assert!(reg.resolve(Path::new("src/extra/mod.rs")).is_some());
    assert!(reg.resolve(Path::new("src/other.rs")).is_none());
}

#[test]
fn test_detect_from_repository_layout_cargo() {
    // Uses vox_test_harness::synthetic_workspace::SyntheticWorkspaceBuilder so
    // we don't reinvent the temp-dir + crates/<name>/Cargo.toml scaffolding.
    // The builder also writes a workspace root Cargo.toml + Cargo.lock, which
    // is fine for detect_from_repository_layout (it only inspects crates/).
    let ws = SyntheticWorkspaceBuilder::new()
        .member(MemberSpec::library("alpha"))
        .member(MemberSpec::library("beta"))
        .build()
        .expect("build synthetic workspace");

    let reg = AffinityGroupRegistry::detect_from_repository_layout(ws.root());

    assert!(reg.find_by_name("alpha-group").is_some());
    assert!(reg.find_by_name("beta-group").is_some());
}
