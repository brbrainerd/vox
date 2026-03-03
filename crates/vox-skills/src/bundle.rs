//! Skill bundle format — an in-memory or on-disk representation of a VoxSkill.
//!
//! A `VoxSkillBundle` contains:
//! - The parsed `SkillManifest`
//! - Raw SKILL.md content (instructions for the runtime)
//! - Optional inline tool implementations (JSON arrays of MCP tool specs)
//! - Optional asset bytes

use serde::{Deserialize, Serialize};

use crate::manifest::SkillManifest;
use crate::SkillError;

/// A fully-loaded skill bundle ready for installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxSkillBundle {
    pub manifest: SkillManifest,
    /// Full SKILL.md text (instructions + frontmatter)
    pub skill_md: String,
    /// MCP tool definitions in JSON (array of {name, description} objects)
    pub tools_json: Option<String>,
    /// Raw asset bytes keyed by relative path
    #[serde(skip)]
    pub assets: Vec<(String, Vec<u8>)>,
}

impl VoxSkillBundle {
    pub fn new(manifest: SkillManifest, skill_md: impl Into<String>) -> Self {
        Self {
            manifest,
            skill_md: skill_md.into(),
            tools_json: None,
            assets: Vec::new(),
        }
    }

    pub fn with_tools_json(mut self, json: impl Into<String>) -> Self {
        self.tools_json = Some(json.into());
        self
    }

    pub fn with_asset(mut self, path: impl Into<String>, data: Vec<u8>) -> Self {
        self.assets.push((path.into(), data));
        self
    }

    /// Compute the SHA-256 content hash over the manifest JSON + SKILL.md.
    pub fn content_hash(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let mut h = Sha3_256::new();
        let manifest_str = serde_json::to_string(&self.manifest).unwrap_or_default();
        h.update(manifest_str.as_bytes());
        h.update(self.skill_md.as_bytes());
        data_encoding::HEXLOWER.encode(&h.finalize())
    }

    /// Serialize the bundle to JSON (for registry storage or download).
    pub fn to_json(&self) -> Result<String, SkillError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize a bundle from JSON.
    pub fn from_json(json: &str) -> Result<Self, SkillError> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Re-export type alias for ergonomics.
pub type SkillBundle = VoxSkillBundle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{SkillCategory, SkillManifest};

    #[test]
    fn bundle_hash_is_stable() {
        let m = SkillManifest::new(
            "vox.compiler",
            "Compiler",
            "0.1.0",
            "vox",
            "Compiles Vox programs",
            SkillCategory::Compiler,
        );
        let bundle = VoxSkillBundle::new(m, "# Compiler Skill\nCompile a Vox file.");
        let h1 = bundle.content_hash();
        let h2 = bundle.content_hash();
        assert_eq!(h1, h2, "Hash must be deterministic");
        assert_eq!(h1.len(), 64, "SHA3-256 hex is 64 chars");
    }

    #[test]
    fn bundle_round_trip_json() {
        let m = SkillManifest::new(
            "vox.test-runner",
            "Test Runner",
            "1.2.3",
            "vox",
            "Runs test suites",
            SkillCategory::Testing,
        );
        let bundle = VoxSkillBundle::new(m, "# Test Runner\nRun all tests.");
        let json = bundle.to_json().expect("serialize");
        let parsed = VoxSkillBundle::from_json(&json).expect("deserialize");
        assert_eq!(parsed.manifest.id, "vox.test-runner");
        assert_eq!(parsed.manifest.version, "1.2.3");
    }
}
