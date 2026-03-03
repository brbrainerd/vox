//! SKILL.md format parser — extracts frontmatter + body from a SKILL.md file.
//!
//! SKILL.md format:
//! ```markdown
//! ---
//! id: "vox.compiler"
//! name: "Compiler"
//! version: "0.1.0"
//! author: "vox"
//! description: "Compiles Vox programs"
//! category: "compiler"
//! tools:
//!   - "vox_compile"
//!   - "vox_check"
//! ---
//!
//! # Compiler Skill
//!
//! ... instructions ...
//! ```

use crate::bundle::VoxSkillBundle;
use crate::manifest::{SkillCategory, SkillManifest, SkillPermission};
use crate::SkillError;

/// Parse a full SKILL.md file into a `VoxSkillBundle`.
pub fn parse_skill_md(content: &str) -> Result<VoxSkillBundle, SkillError> {
    // Split frontmatter from body
    let (frontmatter, _body) = split_frontmatter(content)?;

    // Parse frontmatter as TOML
    let raw: toml::Value =
        toml::from_str(&frontmatter).map_err(|e| SkillError::Toml(e.to_string()))?;

    let id = raw["id"]
        .as_str()
        .ok_or_else(|| SkillError::InvalidManifest("missing id".into()))?
        .to_string();
    let name = raw["name"]
        .as_str()
        .ok_or_else(|| SkillError::InvalidManifest("missing name".into()))?
        .to_string();
    let version = raw["version"]
        .as_str()
        .ok_or_else(|| SkillError::InvalidManifest("missing version".into()))?
        .to_string();
    let author = raw
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let description = raw
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let category_str = raw
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("custom");
    let category = parse_category(category_str);

    // Optional fields
    let tools: Vec<String> = raw
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let tags: Vec<String> = raw
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let dependencies: Vec<String> = raw
        .get("dependencies")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let permissions: Vec<SkillPermission> = raw
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(parse_permission))
                .collect()
        })
        .unwrap_or_default();

    let homepage = raw
        .get("homepage")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut manifest = SkillManifest::new(id, name, version, author, description, category);
    manifest.tools = tools;
    manifest.tags = tags;
    manifest.dependencies = dependencies;
    manifest.permissions = permissions;
    manifest.homepage = homepage;

    Ok(VoxSkillBundle::new(manifest, content.to_string()))
}

fn split_frontmatter(content: &str) -> Result<(String, String), SkillError> {
    let content = content.trim();
    if !content.starts_with("---") {
        return Err(SkillError::InvalidManifest(
            "SKILL.md must start with --- frontmatter".into(),
        ));
    }
    // Skip the first "---"
    let after_first = &content[3..];
    // Find the closing "---"
    if let Some(end) = after_first.find("\n---") {
        let frontmatter = after_first[..end].trim().to_string();
        let body = after_first[end + 4..].trim().to_string();
        Ok((frontmatter, body))
    } else {
        Err(SkillError::InvalidManifest(
            "SKILL.md frontmatter is not properly closed with ---".into(),
        ))
    }
}

fn parse_category(s: &str) -> SkillCategory {
    match s {
        "compiler" => SkillCategory::Compiler,
        "testing" => SkillCategory::Testing,
        "documentation" => SkillCategory::Documentation,
        "deployment" => SkillCategory::Deployment,
        "refactor" => SkillCategory::Refactor,
        "analysis" => SkillCategory::Analysis,
        "git" => SkillCategory::Git,
        "database" => SkillCategory::Database,
        "web_search" => SkillCategory::WebSearch,
        "communication" => SkillCategory::Communication,
        "security" => SkillCategory::Security,
        "monitoring" => SkillCategory::Monitoring,
        other => SkillCategory::Custom(other.to_string()),
    }
}

fn parse_permission(s: &str) -> Option<SkillPermission> {
    match s {
        "read_files" => Some(SkillPermission::ReadFiles),
        "write_files" => Some(SkillPermission::WriteFiles),
        "shell_exec" => Some(SkillPermission::ShellExec),
        "network" => Some(SkillPermission::Network),
        "db_read" => Some(SkillPermission::DbRead),
        "db_write" => Some(SkillPermission::DbWrite),
        "secrets" => Some(SkillPermission::Secrets),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::SkillCategory;

    const TEST_SKILL_MD: &str = r#"---
id = "vox.test-runner"
name = "Test Runner"
version = "1.0.0"
author = "vox-team"
description = "Runs the Vox test suite"
category = "testing"
tools = ["vox_run_tests", "vox_test_all"]
tags = ["testing", "ci"]
permissions = ["read_files", "shell_exec"]
---

# Test Runner Skill

Run all tests in the workspace using `cargo test`.
"#;

    #[test]
    fn parse_valid_skill_md() {
        let bundle = parse_skill_md(TEST_SKILL_MD).expect("parse");
        assert_eq!(bundle.manifest.id, "vox.test-runner");
        assert_eq!(bundle.manifest.version, "1.0.0");
        assert_eq!(bundle.manifest.category, SkillCategory::Testing);
        assert_eq!(bundle.manifest.tools, vec!["vox_run_tests", "vox_test_all"]);
        assert_eq!(bundle.manifest.tags, vec!["testing", "ci"]);
        assert!(bundle
            .manifest
            .permissions
            .contains(&SkillPermission::ReadFiles));
        assert!(bundle
            .manifest
            .permissions
            .contains(&SkillPermission::ShellExec));
    }

    #[test]
    fn missing_frontmatter_is_error() {
        let result = parse_skill_md("# Just a heading\nNo frontmatter.");
        assert!(result.is_err());
    }
}
