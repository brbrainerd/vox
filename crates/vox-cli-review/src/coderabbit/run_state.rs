//! Persistent state for `semantic-submit` resume across chunk failures.

use std::path::Path;

use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};
use vox_bounded_fs::read_utf8_path_capped;

/// One semantic chunk execution record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkRunRecord {
    /// Stable chunk name (e.g. `02_github_agents`).
    pub name: String,
    /// Git branch pushed for this chunk.
    pub branch: String,
    /// GitHub PR number when completed.
    #[serde(default)]
    pub pr_number: Option<u64>,
    /// `pending`, `completed`, or `failed`.
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Full run state under `.coderabbit/run-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoderabbitRunState {
    pub baseline_branch: String,
    pub default_branch: String,
    pub started_at: String,
    pub chunks: Vec<ChunkRunRecord>,
}

impl CoderabbitRunState {
    /// Path to state file inside the repo.
    pub fn path(repo: &Path) -> std::path::PathBuf {
        repo.join(".coderabbit").join("run-state.json")
    }

    pub fn load(repo: &Path) -> Result<Option<Self>> {
        let p = Self::path(repo);
        if !p.is_file() {
            return Ok(None);
        }
        let raw = read_utf8_path_capped(&p).with_context(|| format!("read {}", p.display()))?;
        let s: Self = serde_json::from_str(&raw).context("parse run-state.json")?;
        Ok(Some(s))
    }

    pub fn save(&self, repo: &Path) -> Result<()> {
        let dir = repo.join(".coderabbit");
        std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
        let p = Self::path(repo);
        let json = serde_json::to_string_pretty(self).context("serialize run state")?;
        std::fs::write(&p, json).with_context(|| format!("write {}", p.display()))?;
        Ok(())
    }

    /// Index of first chunk not in `completed` status, or `chunks.len()` if all done.
    pub fn resume_index(&self) -> usize {
        self.chunks
            .iter()
            .position(|c| c.status != "completed")
            .unwrap_or(self.chunks.len())
    }

    pub fn is_completed(&self, chunk_name: &str) -> bool {
        self.chunks
            .iter()
            .any(|c| c.name == chunk_name && c.status == "completed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> CoderabbitRunState {
        CoderabbitRunState {
            baseline_branch: "cr-baseline-test".to_string(),
            default_branch: "main".to_string(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            chunks: vec![
                ChunkRunRecord {
                    name: "a".to_string(),
                    branch: "cr/review-a".to_string(),
                    pr_number: Some(1),
                    status: "completed".to_string(),
                    error: None,
                },
                ChunkRunRecord {
                    name: "b".to_string(),
                    branch: "cr/review-b".to_string(),
                    pr_number: None,
                    status: "pending".to_string(),
                    error: None,
                },
            ],
        }
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let s = sample_state();
        s.save(dir.path()).expect("save");
        let loaded = CoderabbitRunState::load(dir.path())
            .expect("load")
            .expect("some");
        assert_eq!(loaded, s);
    }

    #[test]
    fn resume_index_first_non_completed() {
        let s = sample_state();
        assert_eq!(s.resume_index(), 1);
        let mut all_done = s.clone();
        all_done.chunks[1].status = "completed".to_string();
        assert_eq!(all_done.resume_index(), 2);
    }

    #[test]
    fn is_completed_by_name() {
        let s = sample_state();
        assert!(s.is_completed("a"));
        assert!(!s.is_completed("b"));
    }
}
