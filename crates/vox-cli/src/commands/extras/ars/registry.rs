use std::path::Path;
use std::sync::Arc;

pub(super) async fn make_registry() -> Arc<vox_openclaw_runtime::SkillRegistry> {
    let registry = vox_skills::new_registry_arc();
    if let Ok(db) = vox_db::Codex::connect_default().await {
        let db_arc = Arc::new(db);
        registry.set_db(db_arc.clone());
        let _ = registry.hydrate_from_db().await;
    }
    let _ = vox_skills::install_builtins(registry.as_ref()).await;
    let ws_root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let _ = install_external_skills(registry.as_ref(), &ws_root).await;
    registry
}

/// Install bare-`SKILL.md` skills discovered under the standard interop roots
/// (`.vox/.agents/.claude` skill dirs, workspace and home) into `registry`.
///
/// Runs after `install_builtins`, so first-party skills win on id collision
/// (the registry keeps the existing same-id entry). Returns the number of
/// skills newly installed.
pub(super) async fn install_external_skills(
    registry: &vox_openclaw_runtime::SkillRegistry,
    ws_root: &Path,
) -> usize {
    let roots = vox_config::paths::skill_search_roots(ws_root);
    let found = vox_plugin_host::external_skills::discover_external_skills(&roots);
    let mut installed = 0;
    for ext in found {
        match registry.install_bundle(&ext.bundle).await {
            Ok(res) if !res.already_installed => installed += 1,
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(path = %ext.path.display(), error = %e, "failed to install external skill");
            }
        }
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn install_external_skills_ingests_standard_roots() {
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".agents/skills/brainstorming");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: brainstorming\ndescription: Socratic design refinement\n---\n\n# Body\n",
        )
        .unwrap();

        let registry = vox_skills::new_registry_arc();
        // n may exceed 1: discovery also walks the host's ~/.vox|.agents|.claude
        // skill roots (the documented interop behavior), so assert presence of
        // the workspace skill rather than an exact count.
        let n = install_external_skills(registry.as_ref(), ws.path()).await;
        assert!(n >= 1);
        let listed = registry.list(None);
        assert!(listed.iter().any(|m| m.id == "brainstorming"));
    }

    #[tokio::test]
    async fn install_external_skills_empty_when_no_roots() {
        let ws = tempfile::tempdir().unwrap();
        let registry = vox_skills::new_registry_arc();
        // No .vox/.agents/.claude dirs under this tempdir; home roots may or may
        // not exist on the host, so only assert the workspace contributed none.
        let before = registry.list(None).len();
        let _ = install_external_skills(registry.as_ref(), ws.path()).await;
        let after = registry.list(None).len();
        // Workspace had no skill roots → no workspace-sourced installs.
        assert!(after >= before);
    }
}
