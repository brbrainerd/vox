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
            Ok(res) if !res.already_installed => {
                installed += 1;
                // See Cargo.toml: `vox-gamify` is now a declared feature
                // (`vox-gamify = ["dep:vox-gamify"]`). Before that it named no
                // feature — `dep:vox-gamify` suppresses the implicit one — so
                // this skill_published event was never emitted. Kept gated so
                // the dependency stays optional.
                #[cfg(feature = "vox-gamify")]
                {
                    if let Ok(db) = vox_db::Codex::connect_default().await {
                        let ev = serde_json::json!({
                            "type": "skill_published",
                            "source": "vox-skills",
                            "payload": { "skill_name": ext.bundle.manifest.id.clone() },
                        });
                        let _ = vox_gamify::event_router::route_event_auto_user(&db, &ev).await;
                    }
                }
            }
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
    async fn install_external_skills_ingests_bundled_assets_root() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .canonicalize()
            .expect("repo root");
        let assets_brainstorming = repo_root.join("assets/skills/brainstorming/SKILL.md");
        assert!(
            assets_brainstorming.is_file(),
            "bundled brainstorming skill must exist at {}",
            assets_brainstorming.display()
        );

        let registry = vox_skills::new_registry_arc();
        let _ = install_external_skills(registry.as_ref(), &repo_root).await;
        let listed = registry.list(None);
        assert!(
            listed.iter().any(|m| m.id == "brainstorming"),
            "assets/skills brainstorming should be installed from bundled root"
        );
    }

    #[tokio::test]
    async fn install_external_skills_is_idempotent() {
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".agents/skills/tdd");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: tdd\ndescription: RED-GREEN-REFACTOR\n---\n\n# Body\n",
        )
        .unwrap();

        let registry = vox_skills::new_registry_arc();
        // First pass installs the workspace skill (and any host home skills).
        let n1 = install_external_skills(registry.as_ref(), ws.path()).await;
        assert!(n1 >= 1);
        assert!(registry.list(None).iter().any(|m| m.id == "tdd"));
        // Second pass: everything is already installed at the same version, so
        // install_bundle reports already_installed and nothing is counted.
        let n2 = install_external_skills(registry.as_ref(), ws.path()).await;
        assert_eq!(n2, 0, "re-discovery installs nothing new (idempotent)");
    }
}
