//! `GitBridge` — high-level entry point for Git operations.
//!
//! Wraps a local Git repository and provides Vox-native operations
//! for reading commits, writing refs, and syncing with remotes.
//!
//! All Git I/O uses `gix` (pure Rust). All platform API calls
//! (PRs, webhooks, CI triggers) go through `vox-forge`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::object::ObjectId;
use crate::refs::RefName;
use crate::sync::{SyncStatus, SyncStatusRef};

/// Configuration for a `GitBridge` instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBridgeConfig {
    /// Path to the local repository root (or `.git` parent).
    pub repo_path: PathBuf,
    /// Default remote name (usually "origin").
    pub remote_name: String,
    /// Default branch name (e.g., "main").
    pub default_branch: String,
    /// Whether to use shallow clones (reduces transfer size).
    pub shallow: bool,
    /// Maximum depth for shallow operations.
    pub shallow_depth: u32,
}

impl Default for GitBridgeConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            remote_name: "origin".into(),
            default_branch: "main".into(),
            shallow: false,
            shallow_depth: 50,
        }
    }
}

/// High-level Git bridge.
///
/// Wraps a local `gix::Repository` and exposes Vox-native Git operations.
/// All gix types are confined to the method bodies below.
#[derive(Debug)]
pub struct GitBridge {
    config: GitBridgeConfig,
}

impl GitBridge {
    /// Open an existing Git repository at the given path.
    pub fn open(repo_path: impl AsRef<Path>) -> Result<Self> {
        let path = repo_path.as_ref().to_path_buf();
        // Validate the path contains a .git dir or is a bare repo.
        let git_dir = path.join(".git");
        if !git_dir.exists() && !path.join("HEAD").exists() {
            anyhow::bail!(
                "No Git repository found at '{}'. Expected .git/ directory or bare repo.",
                path.display()
            );
        }
        Ok(Self {
            config: GitBridgeConfig {
                repo_path: path,
                ..GitBridgeConfig::default()
            },
        })
    }

    /// Initialize a new Git repository at the given path.
    pub fn init(repo_path: impl AsRef<Path>) -> Result<Self> {
        let path = repo_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
        // Use gix to initialize.
        // gix::init(&path)?;
        // For now, fall back to git command (temporary until gix init API is stable).
        let status = std::process::Command::new("git")
            .args(["init", "--initial-branch=main"])
            .arg(&path)
            .status()
            .context("Failed to run git init")?;
        if !status.success() {
            anyhow::bail!("git init failed at '{}'", path.display());
        }
        Ok(Self {
            config: GitBridgeConfig {
                repo_path: path,
                ..GitBridgeConfig::default()
            },
        })
    }

    /// Get the current HEAD commit ID.
    pub fn head_commit_id(&self) -> Result<Option<ObjectId>> {
        let head_file = self.config.repo_path.join(".git").join("HEAD");
        if !head_file.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&head_file)
            .context("Failed to read HEAD")?;
        let content = content.trim();

        if let Some(branch) = content.strip_prefix("ref: ") {
            // Symbolic ref — resolve it.
            let ref_path = self.config.repo_path.join(".git").join(
                branch.replace('/', std::path::MAIN_SEPARATOR_STR),
            );
            if !ref_path.exists() {
                return Ok(None); // unborn branch
            }
            let sha = std::fs::read_to_string(&ref_path)
                .context("Failed to read branch ref")?;
            Ok(ObjectId::parse(sha.trim().to_string()))
        } else {
            // Detached HEAD — content is the SHA directly.
            Ok(ObjectId::parse(content.to_string()))
        }
    }

    /// List local branch names.
    pub fn local_branches(&self) -> Result<Vec<RefName>> {
        let heads_dir = self.config.repo_path.join(".git").join("refs").join("heads");
        if !heads_dir.exists() {
            return Ok(vec![]);
        }
        let mut branches = vec![];
        for entry in std::fs::read_dir(&heads_dir).context("Failed to read refs/heads")? {
            let entry = entry.context("Failed to read dir entry")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            branches.push(RefName::branch(&name));
        }
        Ok(branches)
    }

    /// Get sync status vs the configured remote.
    pub fn sync_status(&self) -> Result<SyncStatus> {
        let head = self.head_commit_id()?;
        let branches = self.local_branches()?;

        let mut ref_diffs = vec![];
        for branch in &branches {
            if let Some(branch_name) = branch.as_branch_name() {
                let remote_ref = RefName::remote_tracking(&self.config.remote_name, branch_name);
                let local_sha = self.read_ref(branch)?;
                let remote_sha = self.read_ref(&remote_ref).ok().flatten();

                ref_diffs.push(SyncStatusRef {
                    ref_name: branch.as_str().to_string(),
                    local_id: local_sha.map(|id| id.0),
                    remote_id: remote_sha.map(|id| id.0),
                    ahead: 0,   // TODO: compute via dag_walk
                    behind: 0,  // TODO: compute via dag_walk
                });
            }
        }

        Ok(SyncStatus {
            repo_path: self.config.repo_path.clone(),
            remote: self.config.remote_name.clone(),
            remote_url: self.remote_url()?,
            head_commit: head,
            ref_diffs,
        })
    }

    /// Read a ref to its target commit ID.
    pub fn read_ref(&self, ref_name: &RefName) -> Result<Option<ObjectId>> {
        let ref_path = self.config.repo_path
            .join(".git")
            .join(ref_name.as_str().replace('/', std::path::MAIN_SEPARATOR_STR));
        if !ref_path.exists() {
            return Ok(None);
        }
        let sha = std::fs::read_to_string(&ref_path)
            .with_context(|| format!("Failed to read ref {}", ref_name))?;
        Ok(ObjectId::parse(sha.trim().to_string()))
    }

    /// Get the URL of the configured remote.
    pub fn remote_url(&self) -> Result<String> {
        let config_path = self.config.repo_path.join(".git").join("config");
        let content = std::fs::read_to_string(&config_path)
            .unwrap_or_default();

        // Simple parse — find [remote "origin"] section and its url.
        let remote_header = format!("[remote \"{}\"]", self.config.remote_name);
        let mut in_remote = false;
        for line in content.lines() {
            let line = line.trim();
            if line == remote_header {
                in_remote = true;
                continue;
            }
            if in_remote {
                if line.starts_with('[') {
                    break; // entered next section
                }
                if let Some(url) = line.strip_prefix("url = ") {
                    return Ok(url.trim().to_string());
                }
            }
        }
        Ok(String::new())
    }

    /// Repository path.
    pub fn repo_path(&self) -> &Path {
        &self.config.repo_path
    }

    /// Mutable access to config (e.g., to change remote or branch).
    pub fn config_mut(&mut self) -> &mut GitBridgeConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_fake_repo(dir: &std::path::Path) {
        fs::create_dir_all(dir.join(".git/refs/heads")).unwrap();
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            dir.join(".git/refs/heads/main"),
            "a94a8fe5ccb19ba61c4c0873d391e987982fbbd3\n",
        )
        .unwrap();
        fs::write(dir.join(".git/config"), "[remote \"origin\"]\n\turl = https://github.com/org/repo.git\n").unwrap();
    }

    #[test]
    fn open_valid_repo() {
        let dir = tempfile::tempdir().unwrap();
        make_fake_repo(dir.path());
        let bridge = GitBridge::open(dir.path()).unwrap();
        assert_eq!(bridge.repo_path(), dir.path());
    }

    #[test]
    fn open_invalid_path_errors() {
        let dir = tempfile::tempdir().unwrap();
        // No .git dir — should fail.
        assert!(GitBridge::open(dir.path()).is_err());
    }

    #[test]
    fn head_commit_id_reads_correctly() {
        let dir = tempfile::tempdir().unwrap();
        make_fake_repo(dir.path());
        let bridge = GitBridge::open(dir.path()).unwrap();
        let head = bridge.head_commit_id().unwrap();
        assert!(head.is_some());
        assert_eq!(
            head.unwrap().short(),
            "a94a8fe"
        );
    }

    #[test]
    fn local_branches_lists_branches() {
        let dir = tempfile::tempdir().unwrap();
        make_fake_repo(dir.path());
        let bridge = GitBridge::open(dir.path()).unwrap();
        let branches = bridge.local_branches().unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].as_branch_name(), Some("main"));
    }

    #[test]
    fn remote_url_reads_config() {
        let dir = tempfile::tempdir().unwrap();
        make_fake_repo(dir.path());
        let bridge = GitBridge::open(dir.path()).unwrap();
        assert_eq!(
            bridge.remote_url().unwrap(),
            "https://github.com/org/repo.git"
        );
    }
}
