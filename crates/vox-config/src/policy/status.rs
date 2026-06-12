//! Per-branch policy run-status overlay (Phase 1c).
//!
//! One report per branch at `.vox/policy-status/<sanitized-branch>.json`, so
//! multiple worktrees/branches coexist. See spec §4.5 / §10 addendum point 3.
//!
//! Honesty contract: a result is recorded ONLY when a gate/rule actually ran.
//! Rules with no result are surfaced as `unknown` ("not run", grey) by the
//! catalog-join in `vox policy status` — never faked green.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One full run report for a single branch. Accumulates across gate runs (the
/// writer in `vox-cli` MERGES results by `id`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRunReport {
    /// The (unsanitized) git branch this report describes.
    pub branch: String,
    /// Short commit the run observed (provenance; may be `"unknown"`).
    pub commit: String,
    /// ISO-8601 timestamp, STAMPED BY THE CALLER (never `now()` in pure code).
    pub ran_at: String,
    /// One entry per policy id that has actually run.
    #[serde(default)]
    pub results: Vec<PolicyResult>,
}

/// The recorded outcome for one policy id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyResult {
    /// Registry id (matches `PolicyEntry::id`), e.g. `code-audit/arch/stub`.
    pub id: String,
    pub status: RunStatus,
    /// Per-finding locations (empty for per-gate pass/fail).
    #[serde(default)]
    pub hits: Vec<Hit>,
    /// Wall-clock of the run that produced this result.
    #[serde(default)]
    pub duration_ms: u64,
}

/// A single finding location within a per-rule result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub file: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pass,
    Fail,
    Warn,
    /// Honest default: the gate/rule has not produced a result on this branch.
    #[default]
    Unknown,
}

/// Directory (relative to repo root) holding per-branch status files.
pub const STATUS_DIR_REL: &str = ".vox/policy-status";

/// Sanitize a branch name into a single filesystem-safe path segment.
/// Any non `[A-Za-z0-9._-]` run collapses to a single `-`.
pub fn sanitize_branch(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    let mut prev_dash = false;
    for ch in branch.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Absolute path to the status file for a (sanitized) branch.
pub fn status_path(repo_root: &Path, branch: &str) -> PathBuf {
    repo_root
        .join(STATUS_DIR_REL)
        .join(format!("{}.json", sanitize_branch(branch)))
}

/// Error returned when a status file cannot be read/parsed.
#[derive(Debug)]
pub enum PolicyStatusError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for PolicyStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyStatusError::Io(e) => write!(f, "reading policy status: {e}"),
            PolicyStatusError::Parse(e) => write!(f, "parsing policy status: {e}"),
        }
    }
}
impl std::error::Error for PolicyStatusError {}

/// Load the status report for one branch. `Ok(None)` if no run has happened
/// (file absent) — the honest "nothing ran yet" state.
pub fn load_status(
    repo_root: &Path,
    branch: &str,
) -> Result<Option<PolicyRunReport>, PolicyStatusError> {
    let path = status_path(repo_root, branch);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(PolicyStatusError::Parse),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PolicyStatusError::Io(e)),
    }
}

/// Load reports for several branches at once (multi-worktree selector).
/// Each entry is `(requested_branch, Option<report>)`, preserving input order.
pub fn load_status_for_branches(
    repo_root: &Path,
    branches: &[String],
) -> Result<Vec<(String, Option<PolicyRunReport>)>, PolicyStatusError> {
    branches
        .iter()
        .map(|b| load_status(repo_root, b).map(|r| (b.clone(), r)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_report() {
        let json = r#"{
          "branch": "main",
          "commit": "abc123",
          "ran_at": "2026-06-06T00:00:00Z",
          "results": [
            { "id": "code-audit/stub/todo", "status": "fail", "duration_ms": 12,
              "hits": [{ "file": "src/a.rs", "line": 7, "note": "todo!()" }] },
            { "id": "ci/manifest", "status": "pass", "duration_ms": 40, "hits": [] }
          ]
        }"#;
        let r: PolicyRunReport = serde_json::from_str(json).unwrap();
        assert_eq!(r.branch, "main");
        assert_eq!(r.results.len(), 2);
        assert_eq!(r.results[0].status, RunStatus::Fail);
        assert_eq!(r.results[0].hits[0].line, 7);
        assert_eq!(r.results[1].status, RunStatus::Pass);
    }

    #[test]
    fn sanitize_branch_is_filesystem_safe() {
        assert_eq!(sanitize_branch("main"), "main");
        assert_eq!(sanitize_branch("feature/foo"), "feature-foo");
        assert_eq!(sanitize_branch("cc/bot/amazing-x"), "cc-bot-amazing-x");
        assert_eq!(sanitize_branch("a b\\c"), "a-b-c");
    }

    #[test]
    fn unknown_is_the_default_variant_for_missing_results() {
        // RunStatus::default() is the honest "not run" grey.
        assert_eq!(RunStatus::default(), RunStatus::Unknown);
    }

    #[test]
    fn multi_branch_reader_isolates_per_branch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(STATUS_DIR_REL)).unwrap();
        std::fs::write(
            status_path(root, "main"),
            r#"{"branch":"main","commit":"a","ran_at":"t","results":[{"id":"ci/manifest","status":"pass","hits":[],"duration_ms":1}]}"#,
        )
        .unwrap();
        // "feature/x" sanitizes to "feature-x"; only one file exists.
        let got =
            load_status_for_branches(root, &["main".to_string(), "feature/x".to_string()]).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "main");
        assert!(got[0].1.is_some(), "main report present");
        assert_eq!(
            got[0].1.as_ref().unwrap().results[0].status,
            RunStatus::Pass
        );
        assert_eq!(got[1].0, "feature/x");
        assert!(got[1].1.is_none(), "absent branch → None (not run)");
    }
}
