//! SP4 batch 3 e2e test: with the vox-plugin-skill-compiler directory
//! installed at a known plugins root, the orchestrator's skill registry
//! must contain the compiler skill (registered via the plugin-host bridge).
//!
//! The skill id registered comes from parsing the SKILL.md frontmatter via
//! `vox_plugin_host::skill_parser::parse_skill_md` (canonical location after SP6),
//! which reads `id = "vox.compiler"`.
//! The plugin-host discover step uses `manifest.plugin.id = "skill-compiler"`
//! as the lookup key, but the bridge re-parses the body so vox-skills ends up
//! with the frontmatter id "vox.compiler".

use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
#[ignore = "pending SP6 plugin_skills_bridge implementation — owner: orchestrator sunset: 2026-12-31"]
async fn compiler_skill_loaded_via_plugin_bridge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // vox-plugin-host discover() walks for Plugin.toml; layout: <root>/<any-dir>/Plugin.toml
    let plugin_dir = tmp.path().join("skill-compiler").join("0.1.0");
    std::fs::create_dir_all(&plugin_dir).expect("mkdir plugin_dir");

    // Copy the in-tree skill plugin to the tempdir install layout.
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates parent")
        .join("vox-plugin-skill-compiler");
    for f in ["Plugin.toml", "compiler.skill.md"] {
        std::fs::copy(src.join(f), plugin_dir.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
    }

    // Build a fresh registry. install_builtins is a no-op and removed.
    let registry: Arc<vox_skills::SkillRegistry> = vox_skills::new_registry_arc();

    assert!(
        registry.get("vox.compiler").is_none(),
        "vox.compiler should NOT be in a fresh registry"
    );

    // Bridge: discover plugins from tempdir and install into registry.
    // Parsing is done via vox_plugin_host::skill_parser::parse_skill_md internally.
    vox_orchestrator::mcp_tools::plugin_skills_bridge::install_discovered_skills(
        &registry,
        tmp.path(),
    )
    .await;

    // The bridge parses the SKILL.md frontmatter, which declares id = "vox.compiler".
    let manifest = registry.get("vox.compiler");
    eprintln!(
        "registry.get(\"vox.compiler\") = {:?}",
        manifest.as_ref().map(|m| &m.id)
    );
    assert!(
        manifest.is_some(),
        "expected vox.compiler in registry after bridge; bridge must have parsed SKILL.md frontmatter"
    );
    let m = manifest.unwrap();
    assert_eq!(m.id, "vox.compiler");
    assert_eq!(m.version, "0.1.0");
}
