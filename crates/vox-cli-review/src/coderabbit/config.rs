//! Load `[review.coderabbit]` from `Vox.toml`.

use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

/// CodeRabbit config from `Vox.toml` `[review.coderabbit]`.
#[derive(Debug, Clone)]
pub struct CodeRabbitConfig {
    /// Tier: free, trial, oss, pro, enterprise.
    pub tier: Option<String>,
    /// Seconds to wait between PR triggers (rate limit).
    pub delay_between_prs_secs: Option<u64>,
    /// Max files per PR for batch/stack.
    pub max_files_per_pr: Option<u32>,
    /// Extra path prefixes to exclude from semantic batches (forward slashes).
    pub exclude_prefixes: Vec<String>,
    /// Paths matching these prefixes keep `*.md` / `*.txt` in review payloads (otherwise excluded
    /// by extension rules). Example: `["AGENTS.md", "docs/src/contributors/"]`.
    pub allow_markdown_prefixes: Vec<String>,
    /// Replace bundled `contracts/review/coderabbit-semantic-groups.v1.yaml` with this path (repo-relative).
    pub groups_config: Option<String>,
    /// When false, skip `cargo metadata` workspace crate rule injection.
    pub semantic_workspace_crates: bool,
    /// Fail `semantic-submit` planning if (unassigned files / included files) exceeds this (0.0–1.0).
    pub max_unassigned_ratio: Option<f64>,
    /// Use alphabetical chunk splits instead of path-prefix packing (matches pre-2026 behavior).
    pub legacy_chunk_split: bool,
}

impl Default for CodeRabbitConfig {
    fn default() -> Self {
        Self {
            tier: None,
            delay_between_prs_secs: None,
            max_files_per_pr: None,
            exclude_prefixes: Vec::new(),
            allow_markdown_prefixes: Vec::new(),
            groups_config: None,
            semantic_workspace_crates: true,
            max_unassigned_ratio: None,
            legacy_chunk_split: false,
        }
    }
}

#[derive(Debug, serde::Deserialize, Default)]
struct VoxTomlReview {
    coderabbit: Option<CoderabbitTomlSection>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct CoderabbitTomlSection {
    tier: Option<String>,
    delay_between_prs_secs: Option<u64>,
    max_files_per_pr: Option<u32>,
    /// Additional path prefixes to exclude (e.g. `"mens/data/"`).
    exclude_prefixes: Option<Vec<String>>,
    allow_markdown_prefixes: Option<Vec<String>>,
    groups_config: Option<String>,
    /// Default true when omitted.
    semantic_workspace_crates: Option<bool>,
    max_unassigned_ratio: Option<f64>,
    legacy_chunk_split: Option<bool>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct VoxTomlRoot {
    review: Option<VoxTomlReview>,
}

/// Load `[review.coderabbit]` from `Vox.toml` in the given directory.
///
/// When `delay_between_prs_secs` or `max_files_per_pr` are absent, they are populated from
/// the tier's defaults so callers don't have to hard-code tier math.
pub fn load_from_dir(path: &Path) -> CodeRabbitConfig {
    let toml_path = path.join("Vox.toml");
    let Ok(text) = read_utf8_path_capped(&toml_path) else {
        return CodeRabbitConfig::default();
    };
    let Ok(parsed) = toml::from_str::<VoxTomlRoot>(&text) else {
        return CodeRabbitConfig::default();
    };
    let Some(review) = parsed.review else {
        return CodeRabbitConfig::default();
    };
    let Some(cr) = review.coderabbit else {
        return CodeRabbitConfig::default();
    };

    // Auto-populate delay and max_files from the tier default when absent.
    let resolved_tier: Option<super::limits::CodeRabbitTier> =
        cr.tier.as_deref().and_then(|t| t.parse().ok());
    let delay = cr
        .delay_between_prs_secs
        .or_else(|| resolved_tier.map(|t| t.min_delay_between_prs_secs()));
    let max_files = cr
        .max_files_per_pr
        .or_else(|| resolved_tier.map(|t| t.recommended_max_files_per_pr() as u32));

    CodeRabbitConfig {
        tier: cr.tier,
        delay_between_prs_secs: delay,
        max_files_per_pr: max_files,
        exclude_prefixes: cr.exclude_prefixes.unwrap_or_default(),
        allow_markdown_prefixes: cr.allow_markdown_prefixes.unwrap_or_default(),
        groups_config: cr.groups_config,
        semantic_workspace_crates: cr.semantic_workspace_crates.unwrap_or(true),
        max_unassigned_ratio: cr.max_unassigned_ratio,
        legacy_chunk_split: cr.legacy_chunk_split.unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_exclude_prefixes_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            dir.path().join("Vox.toml"),
            r#"
[review.coderabbit]
tier = "pro"
exclude_prefixes = ["mens/data/", "tmp/"]
"#,
        )
        .expect("write Vox.toml");
        let c = load_from_dir(dir.path());
        assert_eq!(c.tier.as_deref(), Some("pro"));
        assert_eq!(c.exclude_prefixes.len(), 2);
        assert!(c.exclude_prefixes[0].contains("mens"));
    }

    #[test]
    fn missing_vox_toml_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let c = load_from_dir(dir.path());
        assert!(c.tier.is_none());
        assert!(c.exclude_prefixes.is_empty());
        assert!(c.allow_markdown_prefixes.is_empty());
        assert!(c.semantic_workspace_crates);
        assert!(!c.legacy_chunk_split);
    }

    #[test]
    fn load_allow_markdown_prefixes_from_vox_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            dir.path().join("Vox.toml"),
            r#"
[review.coderabbit]
allow_markdown_prefixes = ["AGENTS.md", "docs/policies/"]
"#,
        )
        .expect("write Vox.toml");
        let c = load_from_dir(dir.path());
        assert_eq!(c.allow_markdown_prefixes.len(), 2);
        assert_eq!(c.allow_markdown_prefixes[0], "AGENTS.md");
    }
}
