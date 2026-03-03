//! Skill manifest types — the metadata schema for a VoxSkill.

use serde::{Deserialize, Serialize};

/// A complete skill manifest (equivalent to OpenClaw's skill.json / SKILL.md frontmatter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillManifest {
    /// Unique skill identifier, e.g. "vox.compiler"
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Semver version string
    pub version: String,
    /// Author or publisher identifier
    pub author: String,
    /// Short description for marketplace display
    pub description: String,
    /// Primary skill category
    pub category: SkillCategory,
    /// Minimum permissions the skill requires
    #[serde(default)]
    pub permissions: Vec<SkillPermission>,
    /// List of MCP tool IDs this skill exposes
    #[serde(default)]
    pub tools: Vec<String>,
    /// Skills this skill depends on (by id)
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Optional homepage or repository URL
    pub homepage: Option<String>,
    /// Optional registry source (defaults to CLAWHUB_BASE)
    pub registry: Option<String>,
    /// SHA-256 content hash of the bundle (set at publish time)
    pub hash: Option<String>,
    /// Optional tags for search indexing
    #[serde(default)]
    pub tags: Vec<String>,
}

impl SkillManifest {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        author: impl Into<String>,
        description: impl Into<String>,
        category: SkillCategory,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: author.into(),
            description: description.into(),
            category,
            permissions: Vec::new(),
            tools: Vec::new(),
            dependencies: Vec::new(),
            homepage: None,
            registry: None,
            hash: None,
            tags: Vec::new(),
        }
    }
}

/// Skill category for marketplace browsing and filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    Compiler,
    Testing,
    Documentation,
    Deployment,
    Refactor,
    Analysis,
    Git,
    Database,
    WebSearch,
    Communication,
    Security,
    Monitoring,
    Custom(String),
}

impl std::fmt::Display for SkillCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(s) => write!(f, "custom:{s}"),
            _ => write!(f, "{:?}", self),
        }
    }
}

/// Permissions a skill may require at install time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillPermission {
    /// Read files from the workspace
    ReadFiles,
    /// Write files to the workspace
    WriteFiles,
    /// Execute shell commands
    ShellExec,
    /// Make HTTP calls to external services
    Network,
    /// Access VoxDB (read)
    DbRead,
    /// Access VoxDB (write)
    DbWrite,
    /// Access secrets/environment variables
    Secrets,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let m = SkillManifest::new(
            "test.skill",
            "Test Skill",
            "1.0.0",
            "vox",
            "A test skill",
            SkillCategory::Testing,
        );
        let json = serde_json::to_string(&m).expect("serialize");
        let parsed: SkillManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, "test.skill");
        assert_eq!(parsed.category, SkillCategory::Testing);
    }
}
