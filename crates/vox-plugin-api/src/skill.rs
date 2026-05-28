//! Plain-Rust types for the skill side of plugin loading.
//! Skill payloads do not cross a dylib boundary, so no abi_stable here.
//!
//! `SkillManifest` is the canonical unified type from `vox-plugin-types`.
//! It supersedes the former slim 5-field struct that lived here; construction
//! sites that only need the core fields can use `..Default::default()` for the rest.

// Re-export the canonical unified type (D-17: one SkillManifest shape).
pub use vox_plugin_types::skill_manifest::{SkillCategory, SkillManifest, SkillPermission};

#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub plugin_id: String,
    pub format_version: u32,
    pub manifest: SkillManifest,
    pub body: String,
    pub exposed_tools: Vec<String>,
}
