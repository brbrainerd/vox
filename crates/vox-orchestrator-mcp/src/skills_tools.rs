//! MCP tools for the vox-skills marketplace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_SKILL_BUNDLE: &str =
    "Validate `bundle_json` against the vox-skills bundle schema (id, manifest, files).";
const REM_SKILL_INSTALL: &str =
    "Check disk permissions, bundle hash conflicts, and that the skill id is not corrupted.";
const REM_SKILL_MD: &str =
    "Ensure `skill_md` matches the SKILL.md frontmatter/body format documented for vox-skills.";
const REM_SKILL_ID: &str = "Run `skill_list` / `skill_search` and pass an installed skill `id`.";

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillInstallParams {
    pub bundle_json: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillIdParams {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillSearchParams {
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillRunParams {
    pub id: String,
    pub command: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillParseParams {
    pub skill_md: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillAddParams {
    /// Git URL or local path to a skill (or repo of skills).
    pub source: String,
    /// Install into `~/.vox/skills` instead of the workspace `.vox/skills`.
    #[serde(default)]
    pub global: bool,
    /// Optional: install only the skill whose `name` matches.
    #[serde(default)]
    pub skill: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillRemoveParams {
    pub id: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub tools: Vec<String>,
    /// Provenance tag: `"plugin:<id>"`, `"bundle"`, `"openclaw:<node>"`, or
    /// `"local"`. Lets the UI distinguish marketplace-installed skills from
    /// locally-authored / plugin-bundled ones.
    pub source: String,
    /// Permissions the skill requested at install time (e.g. `"network"`).
    pub permissions: Vec<String>,
    /// Free-form discovery tags from the manifest.
    pub tags: Vec<String>,
}

fn to_info_with_source(m: vox_skills::SkillManifest, source: String) -> SkillInfo {
    SkillInfo {
        id: m.id,
        name: m.name,
        version: m.version,
        category: m.category.to_string(),
        description: m.description,
        tools: m.tools,
        source,
        permissions: m.permissions.iter().map(|p| format!("{p:?}")).collect(),
        tags: m.tags,
    }
}

fn manifest_source_label(state: &ServerState, id: &str) -> String {
    match state.skill_registry.lookup(id) {
        Ok(loaded) => {
            if loaded.plugin_id != id && !loaded.plugin_id.is_empty() {
                format!("plugin:{}", loaded.plugin_id)
            } else {
                "installed".to_string()
            }
        }
        Err(_) => "local".to_string(),
    }
}

fn to_info(state: &ServerState, m: vox_skills::SkillManifest) -> SkillInfo {
    let id = m.id.clone();
    to_info_with_source(m, manifest_source_label(state, &id))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn skill_install(state: &ServerState, params: SkillInstallParams) -> String {
    let bundle = match vox_skills::VoxSkillBundle::from_json(&params.bundle_json) {
        Ok(b) => b,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("Invalid bundle: {e}"),
                REM_SKILL_BUNDLE,
            )
            .to_json();
        }
    };
    // Arc<SkillRegistry> — interior mutability, no Mutex needed
    match state.skill_registry.install_bundle(&bundle).await {
        Ok(res) => {
            state.rebuild_skill_search_index();
            if res.already_installed {
                ToolResult::ok(format!(
                    "Skill '{}' already installed at {}",
                    res.id, res.version
                ))
                .to_json()
            } else {
                ToolResult::ok(format!(
                    "Installed '{}' v{} (hash: {})",
                    res.id,
                    res.version,
                    &res.hash[..12]
                ))
                .to_json()
            }
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SKILL_INSTALL).to_json()
        }
    }
}

pub async fn skill_uninstall(state: &ServerState, params: SkillIdParams) -> String {
    match state.skill_registry.uninstall(&params.id).await {
        Ok(res) => {
            if res.was_installed {
                state.rebuild_skill_search_index();
            }
            if res.was_installed {
                ToolResult::ok(format!("Skill '{}' uninstalled.", res.id)).to_json()
            } else {
                ToolResult::ok(format!("Skill '{}' was not installed.", res.id)).to_json()
            }
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SKILL_ID).to_json()
        }
    }
}

pub fn skill_list(state: &ServerState) -> String {
    let skills: Vec<SkillInfo> = state
        .skill_registry
        .list(None)
        .into_iter()
        .map(|m| to_info(state, m))
        .collect();
    ToolResult::ok(skills).to_json()
}

pub fn skill_search(state: &ServerState, params: SkillSearchParams) -> String {
    let hits = state.skill_search_index.read().search(&params.query, 10);
    let manifests: Vec<SkillInfo> = hits
        .iter()
        .filter_map(|h| state.skill_registry.get(&h.id).map(|m| to_info(state, m)))
        .collect();
    if manifests.is_empty() {
        ToolResult::ok(format!("No skills matching '{}'.", params.query)).to_json()
    } else {
        ToolResult::ok(manifests).to_json()
    }
}

pub fn skill_parse(params: SkillParseParams) -> String {
    match vox_skills::parser::parse_skill_md(&params.skill_md) {
        Ok(bundle) => {
            ToolResult::ok(to_info_with_source(bundle.manifest, "parsed".to_string())).to_json()
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("Parse error: {e}"), REM_SKILL_MD)
                .to_json()
        }
    }
}

pub fn skill_info(state: &ServerState, params: SkillIdParams) -> String {
    match state.skill_registry.get(&params.id) {
        Some(m) => ToolResult::ok(to_info(state, m)).to_json(),
        None => ToolResult::<String>::err_with_remediation(
            format!("Skill '{}' not installed.", params.id),
            REM_SKILL_ID,
        )
        .to_json(),
    }
}

/// Tier-2 progressive disclosure: full SKILL.md body for one installed skill.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillUseResponse {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Resolve a pinned skill by id or name and set [`ServerState::active_skill_id`].
pub fn activate_skill_for_id_or_name(state: &ServerState, id_or_name: &str) -> bool {
    let manifest = state
        .skill_registry
        .list(None)
        .into_iter()
        .find(|m| m.id == id_or_name || m.name == id_or_name);
    match manifest {
        Some(m) => {
            tracing::info!(skill = %m.id, source = "activate", "skill_activated");
            *state.active_skill_id.write() = Some(m.id);
            true
        }
        None => false,
    }
}

/// `vox_skill_use { id }` — return the full SKILL.md body for an installed
/// skill, matched by id or name. This is the tier-2 step of agentskills.io
/// progressive disclosure: tool-calling models load the body on demand after
/// seeing the tier-1 catalog in the system prompt.
pub fn skill_use(state: &ServerState, params: SkillIdParams) -> String {
    let manifest = state
        .skill_registry
        .list(None)
        .into_iter()
        .find(|m| m.id == params.id || m.name == params.id);
    match manifest {
        Some(m) => {
            activate_skill_for_id_or_name(state, &m.id);
            let body = state
                .skill_registry
                .lookup(&m.id)
                .ok()
                .map(|s| s.body)
                .unwrap_or_default();
            tracing::info!(skill = %m.id, source = "tool", "skill_activated");
            ToolResult::ok(SkillUseResponse {
                name: m.name,
                description: m.description,
                body,
            })
            .to_json()
        }
        None => ToolResult::<String>::err_with_remediation(
            format!("Skill '{}' not installed.", params.id),
            REM_SKILL_ID,
        )
        .to_json(),
    }
}

/// One discovered (possibly not-yet-installed) skill under a standard root.
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Directory containing the SKILL.md (display/install source).
    pub path: String,
    /// True when a skill with this id is already in the registry.
    pub installed: bool,
    /// Ecosystem root the skill lives under: `bundled|cursor|claude|agents|vox|unknown`.
    pub source_root: String,
    /// True only when the skill is under a `.vox/skills` root (safe to delete).
    pub removable: bool,
    /// Best-effort license signal: name of a LICENSE file in the dir, else "".
    pub license: String,
}

/// `vox_skill_discover` — list bare SKILL.md skills found under the standard
/// interop roots (`.vox`/`.agents`/`.claude` × workspace+home), each tagged
/// with whether it is already installed. Backs the GUI "Discovered" tab.
pub fn skill_discover(state: &ServerState) -> String {
    let ws_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let roots = vox_config::paths::skill_search_roots(&ws_root);
    let installed: std::collections::HashSet<String> = state
        .skill_registry
        .list(None)
        .into_iter()
        .map(|m| m.id)
        .collect();
    let items: Vec<DiscoveredSkill> =
        vox_plugin_host::external_skills::discover_external_skills(&roots)
            .into_iter()
            .map(|ext| {
                let id = ext.bundle.manifest.id;
                DiscoveredSkill {
                    installed: installed.contains(&id),
                    name: ext.bundle.manifest.name,
                    description: ext.bundle.manifest.description,
                    source_root: vox_plugin_host::user_install::source_root_label(&ext.path)
                        .to_string(),
                    removable: vox_plugin_host::user_install::is_removable(&ext.path),
                    license: vox_plugin_host::user_install::license_hint(&ext.path),
                    path: ext.path.display().to_string(),
                    id,
                }
            })
            .collect();
    ToolResult::ok(items).to_json()
}

/// `vox_skill_add` — install skill(s) from a git URL or local path into the user
/// root (`.vox/skills`, or `~/.vox/skills` when `global`). Validates frontmatter;
/// never runs `scripts/`. Re-discovers so the new skills load without restart.
pub async fn skill_add(state: &ServerState, params: SkillAddParams) -> String {
    let ws_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let source = params.source.clone();
    let global = params.global;
    let filter = params.skill.clone();
    let result = tokio::task::spawn_blocking(move || {
        vox_plugin_host::user_install::install_to_user_root(
            &source,
            &ws_root,
            global,
            filter.as_deref(),
        )
    })
    .await;
    match result {
        Ok(Ok(installed)) => {
            // Load the freshly installed skills into the registry.
            let roots = vox_config::paths::skill_search_roots(
                &state
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(".")),
            );
            for ext in vox_plugin_host::external_skills::discover_external_skills(&roots) {
                let _ = state.skill_registry.install_bundle(&ext.bundle).await;
            }
            state.rebuild_skill_search_index();
            let names: Vec<String> = installed.into_iter().map(|s| s.name).collect();
            ToolResult::ok(serde_json::json!({ "installed": names })).to_json()
        }
        Ok(Err(e)) => ToolResult::<String>::err_with_remediation(
            e,
            "Pass a valid git URL or local path; the source must contain a SKILL.md with valid frontmatter.",
        )
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("add task failed: {e}")).to_json(),
    }
}

/// `vox_skill_remove` — delete a user-installed skill's directory (ownership-scoped
/// to `.vox/skills`), then drop its registry row. Bundled / other-tool skills are
/// read-only and refused.
pub async fn skill_remove(state: &ServerState, params: SkillRemoveParams) -> String {
    let ws_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let roots = vox_config::paths::skill_search_roots(&ws_root);
    let id = params.id.clone();
    let removed =
        tokio::task::spawn_blocking(move || vox_plugin_host::user_install::remove_user_skill(&id, &roots))
            .await;
    match removed {
        Ok(Ok(path)) => {
            let _ = state.skill_registry.uninstall(&params.id).await;
            state.rebuild_skill_search_index();
            ToolResult::ok(format!("Removed '{}' ({})", params.id, path.display())).to_json()
        }
        Ok(Err(e)) => ToolResult::<String>::err_with_remediation(
            e,
            "Only skills under .vox/skills are removable; bundled and other-tool skills are read-only.",
        )
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("remove task failed: {e}")).to_json(),
    }
}

/// `vox_skill_run` — execute a skill script in the sandbox (parity with CLI `vox skill run`).
pub async fn skill_run(state: &ServerState, params: SkillRunParams) -> String {
    if state.skill_registry.get(&params.id).is_none() {
        return ToolResult::<String>::err_with_remediation(
            format!("Skill '{}' not installed.", params.id),
            REM_SKILL_ID,
        )
        .to_json();
    }
    let command = params.command;
    let outcome = tokio::task::spawn_blocking(move || {
        let runner = vox_skills::sandbox::SandboxedSkillRunner::detect()?;
        let limits = vox_openclaw_runtime::manifest::ResourceLimits::default();
        runner.run(&command, &limits)
    })
    .await;
    match outcome {
        Ok(Ok(out)) => ToolResult::ok(serde_json::json!({
            "exit_code": out.exit_code,
            "stdout": out.stdout,
            "stderr": out.stderr,
        }))
        .to_json(),
        Ok(Err(e)) => ToolResult::<String>::err(format!("sandbox run failed: {e}")).to_json(),
        Err(e) => ToolResult::<String>::err(format!("sandbox task failed: {e}")).to_json(),
    }
}

#[cfg(test)]
mod provenance_tests {
    use std::path::Path;

    use vox_plugin_host::user_install::{is_removable, license_hint, source_root_label};

    #[test]
    fn discovered_fields_derive_from_path() {
        // A .vox/skills skill is removable and labelled "vox".
        let dir = Path::new("/ws/.vox/skills/mine");
        assert_eq!(source_root_label(dir), "vox");
        assert!(is_removable(dir));
        // license_hint returns "" for a path with no LICENSE file.
        assert_eq!(license_hint(dir), "");
    }
}
