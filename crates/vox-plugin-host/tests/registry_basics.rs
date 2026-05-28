use vox_plugin_api::skill::{LoadedSkill, SkillManifest};
use vox_plugin_host::{Registry, SkillRegistry};

fn fake_skill(id: &str) -> LoadedSkill {
    LoadedSkill {
        plugin_id: id.to_string(),
        format_version: 1,
        manifest: SkillManifest {
            id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: "fake".to_string(),
            tools: vec![],
            ..Default::default()
        },
        body: "# fake".to_string(),
        exposed_tools: vec![],
    }
}

#[test]
fn skill_registry_install_and_lookup() {
    let reg = SkillRegistry::new();
    reg.install(fake_skill("foo"));
    let found = reg.lookup("foo").expect("should find");
    assert_eq!(found.plugin_id, "foo");
    assert!(reg.list_ids().contains(&"foo".to_string()));
}

#[test]
fn skill_registry_lookup_missing_returns_error() {
    let reg = SkillRegistry::new();
    let err = reg.lookup("nope").expect_err("should fail");
    assert_eq!(err.skill_id, "nope");
}

#[test]
fn registry_starts_empty() {
    let reg = Registry::new();
    assert!(reg.list_ids().is_empty());
    assert!(!reg.has("anything"));
}
