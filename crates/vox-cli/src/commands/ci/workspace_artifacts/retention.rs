//! YAML retention policy + mens run prune planner.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::artifact_policy;

#[derive(Debug, Clone, Deserialize)]
pub struct TransientPolicy {
    #[serde(default = "default_transient_age")]
    pub max_age_days: u32,
}

fn default_transient_age() -> u32 {
    7
}

impl Default for TransientPolicy {
    fn default() -> Self {
        Self {
            max_age_days: default_transient_age(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScratchPolicy {
    #[serde(default = "default_scratch_age")]
    pub max_age_days: u32,
}

fn default_scratch_age() -> u32 {
    7
}

impl Default for ScratchPolicy {
    fn default() -> Self {
        Self {
            max_age_days: default_scratch_age(),
        }
    }
}

/// Policy for cleaning per-worktree `target/` dirs (see `worktree_gc`).
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeTargetsPolicy {
    #[serde(default = "default_worktree_target_age")]
    pub max_age_days: u32,
    /// When true (default), worktrees with uncommitted source are protected
    /// unless `--include-dirty-targets` is passed.
    #[serde(default = "default_true")]
    pub protect_dirty: bool,
}

fn default_worktree_target_age() -> u32 {
    7
}

fn default_true() -> bool {
    true
}

impl Default for WorktreeTargetsPolicy {
    fn default() -> Self {
        Self {
            max_age_days: default_worktree_target_age(),
            protect_dirty: true,
        }
    }
}

/// Policy for removing whole stale worktrees (see `worktree_gc`).
#[derive(Debug, Clone, Deserialize)]
pub struct StaleWorktreesPolicy {
    #[serde(default = "default_stale_worktree_age")]
    pub max_age_days: u32,
}

fn default_stale_worktree_age() -> u32 {
    7
}

impl Default for StaleWorktreesPolicy {
    fn default() -> Self {
        Self {
            max_age_days: default_stale_worktree_age(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MensPolicy {
    #[serde(default = "default_mens_age")]
    pub max_age_days: u32,
    #[serde(default = "default_mens_bytes")]
    pub max_total_bytes: u64,
    #[serde(default = "default_mens_min_keep")]
    pub min_keep: usize,
    #[serde(default)]
    pub protected_names: Vec<String>,
    #[serde(default = "default_latest_pointer")]
    pub latest_pointer: String,
}

fn default_mens_age() -> u32 {
    90
}

fn default_mens_bytes() -> u64 {
    50 * 1024 * 1024 * 1024
}

fn default_mens_min_keep() -> usize {
    2
}

fn default_latest_pointer() -> String {
    "mens/runs/latest".to_string()
}

impl Default for MensPolicy {
    fn default() -> Self {
        Self {
            max_age_days: default_mens_age(),
            max_total_bytes: default_mens_bytes(),
            min_keep: default_mens_min_keep(),
            protected_names: vec![],
            latest_pointer: default_latest_pointer(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceArtifactRetentionFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub transient: TransientPolicy,
    #[serde(default)]
    pub scratch: ScratchPolicy,
    #[serde(default)]
    pub mens: MensPolicy,
    #[serde(default)]
    pub worktree_targets: WorktreeTargetsPolicy,
    #[serde(default)]
    pub stale_worktrees: StaleWorktreesPolicy,
}

impl WorkspaceArtifactRetentionFile {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("read retention policy {}", path.display()))?;
        serde_yaml::from_str(&raw).context("parse workspace-artifact-retention YAML")
    }

    pub fn embedded_defaults() -> Self {
        Self {
            schema_version: 1,
            transient: TransientPolicy::default(),
            scratch: ScratchPolicy::default(),
            mens: MensPolicy::default(),
            worktree_targets: WorktreeTargetsPolicy::default(),
            stale_worktrees: StaleWorktreesPolicy::default(),
        }
    }
}

pub(crate) fn age_days(mtime: SystemTime) -> u32 {
    SystemTime::now()
        .duration_since(mtime)
        .map(|d| (d.as_secs() / 86_400) as u32)
        .unwrap_or(0)
}

fn dir_total_bytes(path: &Path) -> u64 {
    let mut n = 0u64;
    for e in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        if e.path_is_symlink() {
            continue;
        }
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                n = n.saturating_add(m.len());
            }
        }
    }
    n
}

#[derive(Clone)]
struct MensRunRow {
    path: PathBuf,
    mtime: SystemTime,
    bytes: u64,
}

/// Plan deletions for `mens/runs/*` per policy (age, total cap, min_keep, protected, latest).
pub(crate) fn plan_mens_run_deletions(
    root: &Path,
    policy: &MensPolicy,
) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let mut warnings = Vec::new();
    let runs_root = root.join("mens").join("runs");
    if !runs_root.is_dir() {
        return Ok((vec![], warnings));
    }

    let latest_path = root.join(&policy.latest_pointer);
    let mut latest_canon: Option<PathBuf> = None;
    if latest_path.exists() {
        latest_canon = fs::canonicalize(&latest_path).ok();
    }

    let protected_names: HashSet<String> = policy.protected_names.iter().cloned().collect();

    let mut rows: Vec<MensRunRow> = Vec::new();
    for entry in
        fs::read_dir(&runs_root).with_context(|| format!("read {}", runs_root.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "latest" {
            continue;
        }
        if protected_names.contains(&name) {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if !ft.is_dir() {
            continue;
        }
        let p = entry.path();
        let meta = fs::metadata(&p)?;
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let bytes = dir_total_bytes(&p);
        if let Some(ref lc) = latest_canon {
            if let Ok(c) = fs::canonicalize(&p) {
                if c == *lc {
                    continue;
                }
            }
        }
        rows.push(MensRunRow {
            path: p,
            mtime,
            bytes,
        });
    }

    rows.sort_by_key(|r| std::cmp::Reverse(r.mtime));

    let mut never_delete: HashSet<PathBuf> = HashSet::new();
    for r in rows.iter().take(policy.min_keep) {
        never_delete.insert(r.path.clone());
    }

    let mut to_delete: Vec<PathBuf> = Vec::new();
    for r in &rows {
        if never_delete.contains(&r.path) {
            continue;
        }
        if age_days(r.mtime) >= policy.max_age_days {
            to_delete.push(r.path.clone());
        }
    }

    while rows
        .iter()
        .filter(|r| !to_delete.contains(&r.path))
        .map(|r| r.bytes)
        .sum::<u64>()
        > policy.max_total_bytes
    {
        let next = rows
            .iter()
            .filter(|r| !to_delete.contains(&r.path))
            .filter(|r| !never_delete.contains(&r.path))
            .min_by_key(|r| r.mtime);
        let Some(r) = next else {
            break;
        };
        to_delete.push(r.path.clone());
    }

    let surviving_count = rows.len().saturating_sub(to_delete.len());
    if surviving_count == 0 && !rows.is_empty() {
        warnings.push(
            "mens retention would delete all non-protected runs; keeping newest run".to_string(),
        );
        if let Some(newest) = rows.first() {
            to_delete.retain(|p| p != &newest.path);
        }
    }

    Ok((to_delete, warnings))
}

/// Root-level dirs that look like Cargo target sprawl (`target-*`, `target_*`).
pub(crate) fn repo_root_stale_target_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(root) else {
        return out;
    };
    for e in rd.filter_map(Result::ok) {
        let Ok(ft) = e.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("target-") || name.starts_with("target_") {
            out.push(e.path());
        }
    }
    out
}

pub(crate) fn is_scratch_root_file(name: &str) -> bool {
    name.ends_with(".log")
        || name.ends_with(".err")
        || name == "cargo-out.txt"
        || name == "test_all.txt"
        || name.ends_with(".bak")
        || name.ends_with(".orig")
        || name.ends_with(".jsonl")
        || name.starts_with("check_err")
        || name.starts_with("raw_err")
        || name == "clippy_output.json"
        || name == "pm_errors.json"
        || (name.ends_with(".txt") && name != "README.txt" && name != "LICENSE.txt")
}

fn collect_stale_rename_under(dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    for e in WalkDir::new(dir)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(Result::ok)
    {
        if e.path_is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy();
        if name.contains(".stale-") {
            out.push(e.path().to_path_buf());
        }
    }
    out
}

pub(crate) fn collect_stale_rename_paths(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let t = root.join("target");
    out.extend(collect_stale_rename_under(&t, 16));
    for lane in artifact_policy::transient_lane_roots(root) {
        out.extend(collect_stale_rename_under(&lane, 12));
    }
    out
}

pub(crate) fn path_allowed_for_prune(path: &Path, root: &Path) -> bool {
    if artifact_policy::is_allowed_artifact_path(path, root) {
        return true;
    }
    if repo_root_stale_target_dirs(root)
        .iter()
        .any(|p| path.starts_with(p))
    {
        return true;
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name().and_then(|s| s.to_str())) {
        if parent == root && is_scratch_root_file(name) {
            return true;
        }
    }
    false
}
