//! End-to-end: copy noop-skill directory contents to a tempdir, discover,
//! lookup the skill in the registry, assert exposed_tools and body content.

use std::path::PathBuf;
use vox_plugin_host::discover;

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

#[test]
fn end_to_end_load_noop_skill() {
    let src = workspace_root()
        .join("crates")
        .join("vox-plugin-host")
        .join("tests")
        .join("fixtures")
        .join("noop-skill");

    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_dir = tmp.path().join("noop-skill").join("0.1.0");
    std::fs::create_dir_all(&plugin_dir).expect("mkdir");
    for f in ["Plugin.toml", "noop.skill.md"] {
        std::fs::copy(src.join(f), plugin_dir.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
    }

    let registry = discover(tmp.path()).expect("discover");
    let skill = registry.skills.lookup("noop-skill").expect("lookup");
    assert_eq!(skill.exposed_tools, vec!["noop_tool".to_string()]);
    assert!(skill.body.contains("Noop Skill"));
    assert_eq!(skill.format_version, 1);
}
