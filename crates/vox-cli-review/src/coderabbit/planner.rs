//! Size-first PR batch planner for CodeRabbit.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::git;
use super::path_policy;

/// Configuration for the batch planner.
#[derive(Debug, Clone)]
pub struct BatchPlanConfig {
    pub max_files_per_pr: u32,
    pub hard_cap: u32,
}

impl Default for BatchPlanConfig {
    fn default() -> Self {
        Self {
            max_files_per_pr: 250,
            hard_cap: 300,
        }
    }
}

/// A single batch (one PR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBatch {
    pub batch_index: usize,
    pub files: Vec<String>,
    pub file_count: usize,
}

/// Manifest artifact for restart/resume and downstream ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchManifest {
    pub generated_at: String,
    pub max_files_per_pr: u32,
    pub hard_cap: u32,
    pub batches: Vec<FileBatch>,
    pub total_files: usize,
}

/// Size-first batch planner.
pub struct BatchPlanner {
    config: BatchPlanConfig,
}

impl BatchPlanner {
    pub fn new(config: BatchPlanConfig) -> Self {
        Self { config }
    }

    pub fn plan(&self, files_with_size: Vec<(PathBuf, u64)>) -> BatchManifest {
        let mut sorted: Vec<_> = files_with_size.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let total_files = sorted.len();
        let max_per = self.config.max_files_per_pr.min(self.config.hard_cap) as usize;
        let mut batches: Vec<FileBatch> = Vec::new();
        let mut current_batch: Vec<String> = Vec::new();

        for (path, _) in sorted {
            let path_str = path.to_string_lossy().into_owned();
            if current_batch.len() >= max_per {
                let idx = batches.len() + 1;
                let files = std::mem::take(&mut current_batch);
                let file_count = files.len();
                batches.push(FileBatch {
                    batch_index: idx,
                    files,
                    file_count,
                });
            }
            current_batch.push(path_str);
        }

        if !current_batch.is_empty() {
            let idx = batches.len() + 1;
            let file_count = current_batch.len();
            batches.push(FileBatch {
                batch_index: idx,
                files: current_batch,
                file_count,
            });
        }

        BatchManifest {
            generated_at: chrono::Utc::now().to_rfc3339(),
            max_files_per_pr: self.config.max_files_per_pr,
            hard_cap: self.config.hard_cap,
            batches,
            total_files,
        }
    }
}

/// Run batch-submit: plan batches and optionally write manifest.
pub async fn run_batch_submit(
    path: &Path,
    max_files_per_pr: u32,
    hard_cap: u32,
    base_ref: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let vox_cfg = super::config::load_from_dir(path);
    let diffs = git::collect_git_diffs(path, base_ref).context("git diff")?;
    let diffs: Vec<_> = diffs
        .into_iter()
        .filter(|d| {
            let p = d.path.to_string_lossy();
            !path_policy::is_coderabbit_local_tool_path(&p)
                && !path_policy::is_excluded_by_prefixes(&p, &vox_cfg.exclude_prefixes)
        })
        .collect();
    let files_with_size: Vec<(PathBuf, u64)> = diffs
        .into_iter()
        .map(|d| (d.path, d.weight.max(1)))
        .collect();

    if files_with_size.is_empty() {
        anyhow::bail!(
            "No changed files to batch. Use a repo with uncommitted/staged changes, or --base-ref <ref>."
        );
    }

    let config = BatchPlanConfig {
        max_files_per_pr,
        hard_cap,
    };
    let planner = BatchPlanner::new(config);
    let manifest = planner.plan(files_with_size);

    eprintln!(
        "[batch-submit] effective caps: max_files_per_pr={}, hard_cap={}",
        max_files_per_pr, hard_cap
    );
    eprintln!(
        "Batch plan: {} files in {} batch(es){}",
        manifest.total_files,
        manifest.batches.len(),
        if dry_run { " [DRY RUN]" } else { "" }
    );
    for batch in &manifest.batches {
        eprintln!("  Batch {}: {} files", batch.batch_index, batch.file_count);
    }

    if dry_run {
        eprintln!(
            "\n[PLAN ONLY] No manifest written. Pass `--execute` to write .coderabbit-batch-manifest.json"
        );
        return Ok(());
    }

    let manifest_path = path.join(".coderabbit-batch-manifest.json");
    let json = serde_json::to_string_pretty(&manifest).context("Serialize batch manifest")?;
    std::fs::write(&manifest_path, json)
        .with_context(|| format!("Write manifest to {}", manifest_path.display()))?;

    eprintln!("Manifest: {}", manifest_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_size_first_ordering() {
        let config = BatchPlanConfig {
            max_files_per_pr: 3,
            hard_cap: 5,
        };
        let planner = BatchPlanner::new(config);
        let files = vec![
            (PathBuf::from("a.rs"), 10),
            (PathBuf::from("b.rs"), 100),
            (PathBuf::from("c.rs"), 50),
            (PathBuf::from("d.rs"), 200),
            (PathBuf::from("e.rs"), 75),
        ];
        let manifest = planner.plan(files);
        assert_eq!(manifest.total_files, 5);
        assert_eq!(manifest.batches.len(), 2);
        assert_eq!(manifest.batches[0].files[0], "d.rs");
        assert_eq!(manifest.batches[0].files[1], "b.rs");
    }
}
