//! Per-skill MCP tool allowlist enforcement when a skill is active.

use vox_skills::SkillRegistry;

/// MCP tools that remain callable while a restricted skill is active.
fn is_skill_infrastructure_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "vox_skill_list"
            | "vox_skill_search"
            | "vox_skill_discover"
            | "vox_skill_use"
            | "vox_skill_info"
            | "vox_skill_parse"
            | "vox_skill_install"
            | "vox_skill_uninstall"
            | "vox_skill_add"
            | "vox_skill_remove"
            | "vox_skill_run"
            | "vox_workspace_mcp_refresh"
    ) || tool_name.starts_with("vox_chat_")
}

/// Returns `Some(denied_message)` when `tool_name` is not allowed for the active skill.
pub fn check_skill_tool_permission(
    registry: &SkillRegistry,
    active_skill_id: Option<&str>,
    tool_name: &str,
) -> Option<String> {
    if is_skill_infrastructure_tool(tool_name) {
        return None;
    }
    let skill_id = active_skill_id?;
    let manifest = registry.get(skill_id)?;
    if manifest.tools.is_empty() {
        return None;
    }
    if manifest.tools.iter().any(|t| t == tool_name) {
        return None;
    }
    Some(format!(
        "Tool '{tool_name}' is not in skill '{skill_id}' allowlist {:?}",
        manifest.tools
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_plugin_api::skill::LoadedSkill;
    use vox_skills::{SkillManifest, new_registry_arc};

    fn install_git_skill(reg: &vox_skills::SkillRegistry) {
        reg.install(LoadedSkill {
            plugin_id: "git-skill".to_string(),
            format_version: 1,
            manifest: SkillManifest {
                id: "git-skill".to_string(),
                name: "git-skill".to_string(),
                version: "1.0.0".to_string(),
                description: "git only".to_string(),
                tools: vec!["vox_git_status".to_string()],
                ..Default::default()
            },
            body: String::new(),
            exposed_tools: vec!["vox_git_status".to_string()],
        });
    }

    #[test]
    fn denies_tool_not_in_skill_allowlist() {
        let reg = new_registry_arc();
        install_git_skill(&reg);
        let msg = check_skill_tool_permission(&reg, Some("git-skill"), "vox_run_shell");
        assert!(msg.is_some());
    }

    #[test]
    fn allows_listed_tool() {
        let reg = new_registry_arc();
        install_git_skill(&reg);
        assert!(check_skill_tool_permission(&reg, Some("git-skill"), "vox_git_status").is_none());
    }

    #[test]
    fn allows_vox_skill_run_when_skill_active_with_restricted_tools() {
        let reg = new_registry_arc();
        install_git_skill(&reg);
        assert!(check_skill_tool_permission(&reg, Some("git-skill"), "vox_skill_run").is_none());
        assert!(check_skill_tool_permission(&reg, Some("git-skill"), "vox_skill_list").is_none());
        assert!(
            check_skill_tool_permission(&reg, Some("git-skill"), "vox_workspace_mcp_refresh")
                .is_none()
        );
    }

    #[test]
    fn still_denies_unlisted_domain_tool() {
        let reg = new_registry_arc();
        install_git_skill(&reg);
        assert!(check_skill_tool_permission(&reg, Some("git-skill"), "vox_run_shell").is_some());
    }
}
