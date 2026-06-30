//! Core manifest types, [`SemanticPlanner`], and semantic-submit config.

use serde::{Deserialize, Serialize};

use super::super::limits;
use super::super::path_policy;
use super::groups::{
    DEFAULT_MAX_FILES_PER_PR, IGNORED_DIRS, IGNORED_EXTENSIONS, IGNORED_ROOT_EXACT,
    IGNORED_ROOT_PATTERNS,
};
use super::rules::{SemanticRuleSet, pack_oversized_files};

/// A single named semantic group of files (≤ max_files_per_pr).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticChunk {
    /// Ordering key (lower = earlier PR).
    pub order: u32,
    /// Human-readable group name (stable, used for branch names).
    pub name: String,
    /// Relative repository paths.
    pub files: Vec<String>,
}

/// Manifest produced by [`SemanticPlanner::plan`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticManifest {
    pub generated_at: String,
    pub baseline_branch: String,
    /// Count of paths that are eligible for semantic chunking after all planner ignores.
    pub total_files: usize,
    /// Coverage counters used to track "0-100%" review posture for a run.
    pub coverage: CoverageStats,
    pub chunks: Vec<SemanticChunk>,
}

/// Coverage accounting attached to each semantic manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageStats {
    /// Candidate paths considered by the semantic planner.
    pub candidate_files: usize,
    /// Paths accepted for review chunks.
    pub included_files: usize,
    /// Paths excluded by hard planner rules.
    pub ignored_files: usize,
}

/// Diff-based semantic PR planner (rules from YAML + optional `cargo metadata`).
pub struct SemanticPlanner {
    max_files_per_pr: usize,
    /// Paths (forward slashes) starting with one of these keep `*.md` / `*.txt` despite
    /// [`IGNORED_EXTENSIONS`] — see `[review.coderabbit] allow_markdown_prefixes` in `Vox.toml`.
    allow_markdown_prefixes: Vec<String>,
    rule_set: SemanticRuleSet,
    legacy_chunk_split: bool,
}

impl SemanticPlanner {
    pub fn new(
        max_files_per_pr: usize,
        rule_set: SemanticRuleSet,
        legacy_chunk_split: bool,
    ) -> Self {
        Self {
            max_files_per_pr,
            allow_markdown_prefixes: Vec::new(),
            rule_set,
            legacy_chunk_split,
        }
    }

    /// Override markdown/txt extension filtering (typically from `Vox.toml`).
    pub fn with_allow_markdown_prefixes(mut self, prefixes: Vec<String>) -> Self {
        self.allow_markdown_prefixes = prefixes;
        self
    }

    /// Returns `true` if the file should be excluded from review PRs (no markdown allow-list).
    pub fn is_ignored(path: &str) -> bool {
        Self::is_ignored_with(path, &[])
    }

    /// Returns `true` if the file should be excluded, consulting `allow_markdown_prefixes`.
    pub fn is_ignored_with(path: &str, allow_markdown_prefixes: &[String]) -> bool {
        Self::ignored_reason_with(path, allow_markdown_prefixes).is_some()
    }

    /// Returns a stable reason when a path is excluded from semantic review chunks.
    pub fn ignored_reason(path: &str) -> Option<&'static str> {
        Self::ignored_reason_with(path, &[])
    }

    /// [`ignored_reason`] with optional markdown/txt prefix rescue.
    pub fn ignored_reason_with(
        path: &str,
        allow_markdown_prefixes: &[String],
    ) -> Option<&'static str> {
        let p = path.replace('\\', "/");

        if path_policy::is_coderabbit_local_tool_path(&p) {
            return Some("coderabbit_tooling_path");
        }

        if IGNORED_DIRS.iter().any(|d| p.starts_with(d)) {
            return Some("ignored_dir");
        }
        if IGNORED_EXTENSIONS.iter().any(|e| p.ends_with(e)) {
            if markdown_or_txt_allowed(&p, allow_markdown_prefixes) {
                // Fall through — not excluded by extension rule.
            } else {
                return Some("ignored_extension");
            }
        }
        // Root-level scratch patterns and exact names (no directory component)
        if !p.contains('/') {
            if IGNORED_ROOT_EXACT.contains(&p.as_str()) {
                return Some("ignored_root_exact");
            }
            if IGNORED_ROOT_PATTERNS.iter().any(|pat| p.ends_with(pat)) {
                return Some("ignored_root_pattern");
            }
        }
        None
    }

    /// Returns `true` if the file should be excluded from review PRs (instance allow-list).
    pub fn is_path_ignored(&self, path: &str) -> bool {
        Self::ignored_reason_with(path, &self.allow_markdown_prefixes).is_some()
    }

    /// Returns a stable reason when a path is excluded (instance allow-list).
    pub fn path_ignored_reason(&self, path: &str) -> Option<&'static str> {
        Self::ignored_reason_with(path, &self.allow_markdown_prefixes)
    }

    /// Map a file path to its `(order, group_name)` using the loaded rule set.
    pub fn group_for(&self, path: &str) -> (u32, String) {
        self.rule_set.group_for(path)
    }

    /// Plan semantic chunks from a list of files.
    ///
    /// Large groups are sub-divided with path-prefix packing (or legacy alphabetical chunks).
    pub fn plan(&self, files: Vec<String>, baseline_branch: &str) -> SemanticManifest {
        use std::collections::BTreeMap;

        let mut groups: BTreeMap<(u32, String), Vec<String>> = BTreeMap::new();
        let candidate_files = files.len();
        let mut included_files = 0usize;

        for f in files {
            if self.is_path_ignored(&f) {
                continue;
            }
            included_files += 1;
            let (order, name) = self.group_for(&f);
            groups.entry((order, name)).or_default().push(f);
        }

        let mut chunks: Vec<SemanticChunk> = Vec::new();
        for ((order, name), mut group_files) in groups {
            group_files.sort();
            if group_files.len() <= self.max_files_per_pr {
                chunks.push(SemanticChunk {
                    order,
                    name,
                    files: group_files,
                });
            } else {
                let batches = pack_oversized_files(
                    group_files,
                    self.max_files_per_pr,
                    self.legacy_chunk_split,
                );
                for (i, batch) in batches.into_iter().enumerate() {
                    chunks.push(SemanticChunk {
                        order,
                        name: format!("{}_part{}", name, i + 1),
                        files: batch,
                    });
                }
            }
        }

        chunks.sort_by_key(|c| c.order);

        SemanticManifest {
            generated_at: chrono::Utc::now().to_rfc3339(),
            baseline_branch: baseline_branch.to_string(),
            total_files: included_files,
            coverage: CoverageStats {
                candidate_files,
                included_files,
                ignored_files: candidate_files.saturating_sub(included_files),
            },
            chunks,
        }
    }

    pub fn rule_set(&self) -> &SemanticRuleSet {
        &self.rule_set
    }

    pub fn unassigned_name(&self) -> &str {
        &self.rule_set.unassigned_name
    }
}

fn markdown_or_txt_allowed(p: &str, allow_markdown_prefixes: &[String]) -> bool {
    if !p.ends_with(".md") && !p.ends_with(".txt") {
        return false;
    }
    allow_markdown_prefixes
        .iter()
        .any(|pref| p.starts_with(&pref.replace('\\', "/")))
}

/// Configuration for a semantic-submit run.
#[derive(Debug, Clone)]
pub struct SemanticSubmitConfig {
    /// CodeRabbit tier (affects rate-limit delay).
    pub tier: limits::CodeRabbitTier,
    /// Max files per review PR chunk (clamped to tier file cap).
    pub max_files_per_pr: usize,
    /// Seconds between PR triggers (0 = use tier default).
    pub delay_secs: u64,
    /// If false, only plan + write manifest (no git remotes, no PRs).
    pub execute: bool,
    /// Legacy: commit all planned files to `default_branch` and push before opening chunk PRs.
    pub commit_main: bool,
    /// Reuse baseline branch name (enables `--resume` across invocations).
    pub baseline_branch: Option<String>,
    /// Skip chunks already marked `completed` in `.coderabbit/run-state.json`.
    pub resume: bool,
    /// Re-run every chunk even if previously completed.
    pub force_chunks: bool,
    /// Optional pattern filter — only process groups whose name contains this.
    pub group_filter: Option<String>,
    /// Review the **entire tracked repository** from scratch (`git ls-files`) instead of
    /// only files that differ from HEAD. When true, the drift check is skipped (N/A).
    pub full_repo: bool,
    /// Extra exclude prefixes merged after `Vox.toml` (CLI).
    pub extra_exclude_prefixes: Vec<String>,
    /// When set, write JSON array of `{ path, reason }` for candidate paths dropped by planner rules.
    pub write_ignored_paths: Option<std::path::PathBuf>,
    /// From `Vox.toml` `[review.coderabbit] allow_markdown_prefixes`.
    pub allow_markdown_prefixes: Vec<String>,
    /// Alphabetical `chunks(max)` instead of path-prefix packing for oversized groups.
    pub legacy_chunk_split: bool,
    /// Optional path to semantic groups YAML (replaces bundled default).
    pub groups_config: Option<std::path::PathBuf>,
    /// `cargo metadata` workspace crate injection (`false` disables even if YAML enables).
    pub semantic_workspace_crates: bool,
    /// When set, fail planning if unassigned / included exceeds this ratio.
    pub max_unassigned_ratio: Option<f64>,
}

impl Default for SemanticSubmitConfig {
    fn default() -> Self {
        let tier = limits::CodeRabbitTier::Pro;
        let delay = tier.min_delay_between_prs_secs();
        Self {
            tier,
            max_files_per_pr: DEFAULT_MAX_FILES_PER_PR,
            delay_secs: delay,
            execute: false,
            commit_main: false,
            baseline_branch: None,
            resume: false,
            force_chunks: false,
            group_filter: None,
            full_repo: false,
            extra_exclude_prefixes: Vec::new(),
            write_ignored_paths: None,
            allow_markdown_prefixes: Vec::new(),
            legacy_chunk_split: false,
            groups_config: None,
            semantic_workspace_crates: true,
            max_unassigned_ratio: None,
        }
    }
}
