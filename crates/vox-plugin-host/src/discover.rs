//! discover() walks an install root for Plugin.toml manifests, parses each,
//! and populates a Registry. For skill payloads (and the skill side of
//! composite payloads), the SKILL.md body is eagerly read and registered.
//! For code payloads, the dylib path is recorded but NOT loaded — actual
//! dlopen happens lazily via Loader::load().

#![allow(clippy::result_large_err)]

use crate::errors::LoadError;
use crate::registry::{PluginEntry, Registry};
use crate::telemetry;
use std::path::Path;
use vox_plugin_api::manifest::{PluginManifest, PluginPayload, SkillPayload};
use vox_plugin_api::skill::{LoadedSkill, SkillManifest};

pub fn discover(root: &Path) -> Result<Registry, LoadError> {
    let registry = Registry::new();
    if !root.is_dir() {
        return Ok(registry);
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Plugin.toml")
    {
        let path = entry.path();
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipping plugin: failed to read manifest");
                continue;
            }
        };
        let manifest: PluginManifest = match toml::from_str(&raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipping plugin: failed to parse manifest");
                continue;
            }
        };
        let install_dir = path.parent().unwrap().to_path_buf();

        // Skill side: eagerly parse and register if present.
        let skill_to_register: Option<&SkillPayload> = match &manifest.plugin.payload {
            PluginPayload::Skill(s) => Some(s),
            PluginPayload::Composite(c) => Some(&c.skill),
            PluginPayload::Code(_) => None,
        };

        if let Some(skill_payload) = skill_to_register {
            let skill_md_path = install_dir.join(&skill_payload.skill_md);
            let body = match std::fs::read_to_string(&skill_md_path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        plugin_id = %manifest.plugin.id,
                        skill_md_path = %skill_md_path.display(),
                        error = %e,
                        "skill plugin '{}' references missing or unreadable SKILL.md '{}': {}",
                        manifest.plugin.id, skill_md_path.display(), e
                    );
                    // Skip registering this skill — better to surface the missing payload
                    // than to register an empty-body skill that an agent might invoke.
                    continue;
                }
            };
            let exposed_tools = skill_payload.tools.exposes.clone();
            registry.skills.install(LoadedSkill {
                plugin_id: manifest.plugin.id.clone(),
                format_version: skill_payload.format_version,
                manifest: SkillManifest {
                    id: manifest.plugin.id.clone(),
                    name: manifest.plugin.name.clone(),
                    version: manifest.plugin.version.clone(),
                    description: manifest.plugin.description.clone(),
                    tools: exposed_tools.clone(),
                    ..Default::default()
                },
                body,
                exposed_tools,
            });
        }

        let payload_kind = match &manifest.plugin.payload {
            PluginPayload::Code(_) => "code",
            PluginPayload::Skill(_) => "skill",
            PluginPayload::Composite(_) => "composite",
        };
        let abi_or_format = match &manifest.plugin.payload {
            PluginPayload::Code(c) => c.abi_version,
            PluginPayload::Skill(s) => s.format_version,
            PluginPayload::Composite(c) => c.code.abi_version,
        };
        telemetry::discovered(
            &manifest.plugin.id,
            &manifest.plugin.version,
            payload_kind,
            abi_or_format,
        );

        registry.record(PluginEntry {
            id: manifest.plugin.id.clone(),
            version: manifest.plugin.version.clone(),
            install_dir,
            payload: manifest.plugin.payload,
        });
    }
    Ok(registry)
}
