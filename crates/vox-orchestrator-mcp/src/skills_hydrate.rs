//! Boot-time hydration of external agentskills.io skill roots into the registry.

use std::path::Path;
use std::sync::Arc;

use vox_config::paths::skill_search_roots;
use vox_plugin_host::external_skills::discover_external_skills;
use vox_skills::SkillRegistry;

/// Install skills discovered under standard interop roots (`.vox`/`.agents`/`.claude` × workspace+home).
pub async fn hydrate_external_skills(registry: &Arc<SkillRegistry>, workspace_root: &Path) {
    let roots = skill_search_roots(workspace_root);
    for ext in discover_external_skills(&roots) {
        match registry.install_bundle(&ext.bundle).await {
            Ok(res) if !res.already_installed => {
                tracing::info!(
                    skill = %res.id,
                    path = %ext.path.display(),
                    source = "discovered",
                    "hydrated external skill"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    skill = %ext.bundle.manifest.id,
                    path = %ext.path.display(),
                    error = %e,
                    "failed to hydrate external skill"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_skills::new_registry_arc;

    fn write_skill(root: &Path, dir: &str, name: &str) {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test skill {name}\n---\n\n# Body\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn boot_hydrates_external_agents_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let agents = tmp.path().join(".agents").join("skills");
        write_skill(&agents, "foo", "foo");
        let registry = new_registry_arc();
        hydrate_external_skills(&registry, tmp.path()).await;
        assert!(registry.get("foo").is_some());
    }
}
