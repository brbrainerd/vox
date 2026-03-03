//! MCP tools for the vox-skills marketplace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ServerState, ToolResult};

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
pub struct SkillParseParams {
    pub skill_md: String,
}

/// Response shape for skill info.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub tools: Vec<String>,
}

fn to_info(m: vox_skills::SkillManifest) -> SkillInfo {
    SkillInfo {
        id: m.id,
        name: m.name,
        version: m.version,
        category: m.category.to_string(),
        description: m.description,
        tools: m.tools,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn skill_install(state: &ServerState, params: SkillInstallParams) -> String {
    let bundle = match vox_skills::VoxSkillBundle::from_json(&params.bundle_json) {
        Ok(b) => b,
        Err(e) => return ToolResult::<String>::err(format!("Invalid bundle: {e}")).to_json(),
    };
    // Arc<SkillRegistry> — interior mutability, no Mutex needed
    match state.skill_registry.install(&bundle).await {
        Ok(res) => {
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
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

pub async fn skill_uninstall(state: &ServerState, params: SkillIdParams) -> String {
    match state.skill_registry.uninstall(&params.id).await {
        Ok(res) => {
            if res.was_installed {
                ToolResult::ok(format!("Skill '{}' uninstalled.", res.id)).to_json()
            } else {
                ToolResult::ok(format!("Skill '{}' was not installed.", res.id)).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

pub fn skill_list(state: &ServerState) -> String {
    let skills: Vec<SkillInfo> = state
        .skill_registry
        .list(None)
        .into_iter()
        .map(to_info)
        .collect();
    ToolResult::ok(skills).to_json()
}

pub fn skill_search(state: &ServerState, params: SkillSearchParams) -> String {
    let hits: Vec<SkillInfo> = state
        .skill_registry
        .search(&params.query)
        .into_iter()
        .map(to_info)
        .collect();
    if hits.is_empty() {
        ToolResult::ok(format!("No skills matching '{}'.", params.query)).to_json()
    } else {
        ToolResult::ok(hits).to_json()
    }
}

pub fn skill_parse(params: SkillParseParams) -> String {
    match vox_skills::parser::parse_skill_md(&params.skill_md) {
        Ok(bundle) => ToolResult::ok(to_info(bundle.manifest)).to_json(),
        Err(e) => ToolResult::<String>::err(format!("Parse error: {e}")).to_json(),
    }
}

pub fn skill_info(state: &ServerState, params: SkillIdParams) -> String {
    match state.skill_registry.get(&params.id) {
        Some(m) => ToolResult::ok(to_info(m)).to_json(),
        None => {
            ToolResult::<String>::err(format!("Skill '{}' not installed.", params.id)).to_json()
        }
    }
}
