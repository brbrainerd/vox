use vox_plugin_api::skill::{LoadedSkill, SkillManifest};

#[test]
fn loaded_skill_round_trip_construct_and_read() {
    let manifest = SkillManifest {
        id: "skill-foo".to_string(),
        name: "Foo".to_string(),
        version: "0.1.0".to_string(),
        description: "Foo skill.".to_string(),
        tools: vec!["foo_tool".to_string()],
        ..Default::default()
    };
    let skill = LoadedSkill {
        plugin_id: manifest.id.clone(),
        format_version: 1,
        manifest: manifest.clone(),
        body: "# Foo".to_string(),
        exposed_tools: manifest.tools.clone(),
    };
    assert_eq!(skill.plugin_id, "skill-foo");
    assert_eq!(skill.format_version, 1);
    assert_eq!(skill.exposed_tools.len(), 1);
    assert_eq!(skill.body, "# Foo");
}
