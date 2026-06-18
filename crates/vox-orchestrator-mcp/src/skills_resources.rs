//! SEP-2640-style `skill://` resource index and reads.

use serde_json::json;
use vox_skills::SkillRegistry;

const SKILL_INDEX_URI: &str = "skill://index.json";

/// Build the skill index JSON document for `skill://index.json`.
pub fn build_skill_index(registry: &SkillRegistry) -> serde_json::Value {
    let skills: Vec<serde_json::Value> = registry
        .list(None)
        .into_iter()
        .map(|m| {
            json!({
                "name": m.name,
                "description": m.description,
                "type": "skill-md",
                "url": format!("skill://{}/SKILL.md", m.id),
            })
        })
        .collect();
    json!({ "skills": skills })
}

/// List MCP resources for installed skills plus the index.
pub fn list_skill_resources(registry: &SkillRegistry) -> Vec<(String, String)> {
    let mut out = vec![(
        SKILL_INDEX_URI.to_string(),
        "Index of installed skills (SEP-2640)".to_string(),
    )];
    for m in registry.list(None) {
        out.push((format!("skill://{}/SKILL.md", m.id), m.description.clone()));
    }
    out
}

/// Read a `skill://` resource URI.
pub fn read_skill_resource(
    registry: &SkillRegistry,
    uri: &str,
) -> Result<(String, String), String> {
    if uri == SKILL_INDEX_URI {
        let body = serde_json::to_string_pretty(&build_skill_index(registry))
            .map_err(|e| e.to_string())?;
        return Ok(("application/json".to_string(), body));
    }
    let rest = uri
        .strip_prefix("skill://")
        .ok_or_else(|| format!("not a skill:// uri: {uri}"))?;
    let (skill_id, rel) = rest
        .split_once('/')
        .ok_or_else(|| format!("invalid skill uri: {uri}"))?;
    if rel != "SKILL.md" {
        return Err(format!("unsupported skill resource path: {rel}"));
    }
    let body = registry.lookup(skill_id).map_err(|e| e.to_string())?.body;
    Ok(("text/markdown".to_string(), body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_skills::{SkillManifest, VoxSkillBundle, new_registry_arc};

    #[tokio::test]
    async fn skill_index_json_lists_installed_skills() {
        let registry = new_registry_arc();
        let bundle = VoxSkillBundle::new(
            SkillManifest {
                id: "demo".to_string(),
                name: "demo".to_string(),
                version: "1.0.0".to_string(),
                description: "Demo skill".to_string(),
                ..Default::default()
            },
            "---\nname: demo\ndescription: Demo\n---\n",
        );
        registry.install_bundle(&bundle).await.unwrap();
        let index = build_skill_index(&registry);
        assert_eq!(index["skills"][0]["type"], "skill-md");
        assert!(
            index["skills"][0]["url"]
                .as_str()
                .unwrap()
                .starts_with("skill://")
        );
    }
}
