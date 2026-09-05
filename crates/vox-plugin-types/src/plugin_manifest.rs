//! PluginManifest — typed deserialization of Plugin.toml files.
//!
//! See docs/src/reference/plugin-manifest.md for the canonical schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginHeader {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Lifecycle status: alpha → beta → stable → deprecated.
    #[serde(default)]
    pub status: Option<PluginStatus>,
    /// Broad category for marketplace browsing (e.g. "ml-backend", "hardware", "mesh").
    #[serde(default)]
    pub category: Option<String>,
    /// Free-form tags for search/filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Extension-point IDs this plugin satisfies (mirrors catalog `extension-points`).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// ID of the plugin this supersedes (enables graceful migration paths).
    #[serde(default)]
    pub replaces: Option<String>,
    pub host: HostRequirement,
    pub payload: PluginPayload,
}

/// Lifecycle stage of a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Alpha,
    Beta,
    Stable,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HostRequirement {
    pub min_vox_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum PluginPayload {
    Code(CodePayload),
    Skill(SkillPayload),
    Composite(CompositePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CodePayload {
    pub abi_version: u32,
    #[serde(default)]
    pub provides: PayloadProvides,
    #[serde(default)]
    pub requires: PayloadRequires,
    #[serde(default)]
    pub artifacts: BTreeMap<String, String>,
    /// Hex-lowercase SHA3-256 of each artifact's file bytes, keyed by the same
    /// target-triple as `artifacts`. Populated at install time by
    /// `install_from_path` (crates/vox-cli/src/commands/plugin/install.rs)
    /// after the archive is extracted; absent on plugins installed before
    /// this field existed. `#[serde(default)]` so old manifests without it
    /// still parse — `load_code_plugin` treats an absent entry as "no
    /// checksum recorded yet" and proceeds, not as tampering.
    #[serde(default)]
    pub artifacts_sha3: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadProvides {
    #[serde(default)]
    pub extension_points: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadRequires {
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub arch: Vec<String>,
    #[serde(default)]
    pub native_libs: Vec<NativeLib>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativeLib {
    pub name: String,
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillPayload {
    pub format_version: u32,
    pub skill_md: String,
    #[serde(default)]
    pub tools: SkillTools,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillTools {
    #[serde(default)]
    pub exposes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompositePayload {
    pub code: CodePayload,
    pub skill: SkillPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Plugin.toml` installed before `artifacts_sha3` existed carries no
    /// `artifacts-sha3` table at all. `#[serde(default)]` must let it keep
    /// deserializing rather than becoming a hard parse failure — the field is
    /// meant to be a purely additive, migration-safe addition.
    #[test]
    fn code_payload_without_artifacts_sha3_still_deserializes() {
        let toml_src = r#"
            abi-version = 1

            [artifacts]
            "macos-aarch64" = "libdemo.dylib"
        "#;
        let payload: CodePayload = toml::from_str(toml_src).expect("must parse without the field");
        assert_eq!(
            payload.artifacts.get("macos-aarch64").map(String::as_str),
            Some("libdemo.dylib")
        );
        assert!(
            payload.artifacts_sha3.is_empty(),
            "absent table must default to empty, not error"
        );
    }
}
