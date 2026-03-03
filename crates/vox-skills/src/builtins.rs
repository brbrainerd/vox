//! Built-in skills that are always available in the Vox skill registry.
//!
//! These are embedded at compile time via `include_str!` so the registry
//! works even without a filesystem. They are installed on first startup.

use crate::bundle::VoxSkillBundle;
use crate::parser::parse_skill_md;
use crate::registry::SkillRegistry;
use crate::SkillError;

/// All built-in skill SKILL.md contents.
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("vox.compiler", include_str!("../skills/compiler.skill.md")),
    ("vox.testing", include_str!("../skills/testing.skill.md")),
    ("vox.memory", include_str!("../skills/memory.skill.md")),
    ("vox.git", include_str!("../skills/git.skill.md")),
    (
        "vox.orchestrator",
        include_str!("../skills/orchestrator.skill.md"),
    ),
    ("vox.v0", include_str!("../skills/v0.skill.md")),
];

/// Install all built-in skills into the registry if they are not already present.
pub async fn install_builtins(registry: &SkillRegistry) -> Result<usize, SkillError> {
    let mut installed = 0usize;
    for (id, content) in BUILTIN_SKILLS {
        // Don't overwrite if already installed at same version
        let bundle = parse_skill_md(content)?;
        if registry.get(id).is_none() {
            let result = registry.install(&bundle).await?;
            if !result.already_installed {
                installed += 1;
                tracing::debug!(skill = %id, "Built-in skill auto-installed");
            }
        }
    }
    Ok(installed)
}

/// Return all built-in skill bundles without installing them.
pub fn builtin_bundles() -> Result<Vec<VoxSkillBundle>, SkillError> {
    BUILTIN_SKILLS
        .iter()
        .map(|(_, content)| parse_skill_md(content))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_parse() {
        let bundles = builtin_bundles().expect("parse all built-ins");
        assert_eq!(bundles.len(), 6);
        let ids: Vec<_> = bundles.iter().map(|b| b.manifest.id.as_str()).collect();
        assert!(ids.contains(&"vox.compiler"));
        assert!(ids.contains(&"vox.testing"));
        assert!(ids.contains(&"vox.memory"));
        assert!(ids.contains(&"vox.git"));
        assert!(ids.contains(&"vox.orchestrator"));
        assert!(ids.contains(&"vox.v0"));
    }

    #[tokio::test]
    async fn install_builtins_into_empty_registry() {
        let reg = SkillRegistry::new();
        let count = install_builtins(&reg).await.expect("install");
        assert_eq!(count, 6);
        assert!(reg.get("vox.compiler").is_some());
        assert!(reg.get("vox.memory").is_some());
    }

    #[tokio::test]
    async fn install_builtins_is_idempotent() {
        let reg = SkillRegistry::new();
        install_builtins(&reg).await.expect("first install");
        let count = install_builtins(&reg).await.expect("second install");
        assert_eq!(count, 0, "Already installed, should be idempotent");
    }
}
