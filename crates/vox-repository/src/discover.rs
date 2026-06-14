//! Resolve [`crate::RepositoryContext`] from a starting path.

use std::path::{Path, PathBuf};

use crate::RepositoryContext;
use crate::capabilities::probe_capabilities;
use crate::error::RepositoryError;
use crate::git_root::find_git_work_tree;
use crate::id::compute_repository_id;

/// Resolve starting directory to an absolute path, preferring [`std::fs::canonicalize`].
fn absolutize(path: &Path) -> Result<PathBuf, RepositoryError> {
    if path.as_os_str().is_empty() {
        return Ok(std::env::current_dir()?);
    }
    if path.is_absolute() {
        return Ok(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
    }
    let cwd = std::env::current_dir()?;
    let joined = cwd.join(path);
    Ok(std::fs::canonicalize(&joined).unwrap_or(joined))
}

fn read_origin_url(git_work_tree: &Path) -> Option<String> {
    let config_path = git_work_tree.join(".git").join("config");
    let content = std::fs::read_to_string(&config_path).ok()?;
    parse_origin_from_git_config(&content)
}

fn parse_origin_from_git_config(content: &str) -> Option<String> {
    let mut in_origin = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.starts_with("[remote") && t.contains("origin");
            continue;
        }
        if in_origin {
            let t2 = t.strip_prefix("url").unwrap_or("");
            if t2.trim_start().starts_with('=') {
                let rest = t2.split_once('=').map(|x| x.1).unwrap_or("").trim();
                let url = rest.trim_matches(|c| c == '"' || c == '\'').trim();
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

/// Discover repository context from a directory (typically process CWD or client-provided root).
pub fn discover_repository(start: &Path) -> Result<RepositoryContext, RepositoryError> {
    let start_abs = absolutize(start)?;
    let git_root = find_git_work_tree(&start_abs);
    let root = git_root.clone().unwrap_or_else(|| start_abs.clone());
    let root_canon = std::fs::canonicalize(&root).unwrap_or(root);
    let origin = git_root.as_ref().and_then(|g| read_origin_url(g));
    let in_git = git_root.is_some();
    let caps = probe_capabilities(&root_canon, in_git);
    let repository_id = compute_repository_id(&root_canon, origin.as_deref());
    let vox_toml = {
        let p = root_canon.join("Vox.toml");
        if p.is_file() { Some(p) } else { None }
    };
    let has_vox_agents_dir = root_canon.join(".vox").join("agents").is_dir();
    Ok(RepositoryContext {
        root: root_canon,
        git_root,
        repository_id,
        origin_url: origin,
        capabilities: caps,
        has_vox_agents_dir,
        vox_toml,
    })
}

fn bare_context_from_root(root: &Path) -> RepositoryContext {
    let root_canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let git_root = find_git_work_tree(&root_canon);
    let effective_root = git_root.clone().unwrap_or_else(|| root_canon.clone());
    let effective_canon = std::fs::canonicalize(&effective_root).unwrap_or(effective_root);
    let origin = git_root.as_ref().and_then(|g| read_origin_url(g));
    let caps = probe_capabilities(&effective_canon, git_root.is_some());
    let repository_id = compute_repository_id(&effective_canon, origin.as_deref());
    RepositoryContext {
        root: effective_canon.clone(),
        git_root,
        repository_id,
        origin_url: origin,
        capabilities: caps,
        has_vox_agents_dir: effective_canon.join(".vox").join("agents").is_dir(),
        vox_toml: {
            let p = effective_canon.join("Vox.toml");
            if p.is_file() { Some(p) } else { None }
        },
    }
}

/// Same as [`discover_repository`], but never fails: falls back to CWD with tracing.
pub fn discover_repository_or_fallback(start: &Path) -> RepositoryContext {
    discover_repository(start).unwrap_or_else(|e| {
        tracing::warn!(target: "vox_repository", "discover_repository failed: {e}; falling back to CWD");
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        bare_context_from_root(&cwd)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discover_non_git_temp() {
        let dir = tempdir().unwrap();
        let ctx = discover_repository(dir.path()).expect("discover");
        assert_eq!(
            ctx.root,
            dir.path()
                .canonicalize()
                .unwrap_or_else(|_| dir.path().to_path_buf())
        );
        assert!(ctx.git_root.is_none());
        assert_eq!(ctx.repository_id.len(), 16);
        assert!(!ctx.has_vox_agents_dir);
        assert!(ctx.vox_toml.is_none());
    }

    #[test]
    fn discover_with_vox_toml() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Vox.toml"), "[vox]\nmodel = \"x\"\n").unwrap();
        let ctx = discover_repository(dir.path()).expect("discover");
        assert!(ctx.vox_toml.is_some());
        assert!(ctx.capabilities.vox_project);
    }
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_origin_from_git_config_extracts_url() {
        let config = "[core]\n    repositoryformatversion = 0\n[remote \"origin\"]\n    url = https://github.com/example/repo.git\n    fetch = +refs/heads/*:refs/remotes/origin/*\n";
        let url = parse_origin_from_git_config(config);
        assert_eq!(url.as_deref(), Some("https://github.com/example/repo.git"));
    }

    #[test]
    fn parse_origin_from_git_config_returns_none_when_no_origin() {
        let config =
            "[core]\n    repositoryformatversion = 0\n[branch \"main\"]\n    remote = origin\n";
        let url = parse_origin_from_git_config(config);
        assert!(url.is_none());
    }

    #[test]
    fn parse_origin_from_git_config_ignores_non_origin_remotes() {
        let config = "[remote \"upstream\"]\n    url = https://github.com/upstream/repo.git\n[remote \"origin\"]\n    url = https://github.com/fork/repo.git\n";
        let url = parse_origin_from_git_config(config);
        assert_eq!(url.as_deref(), Some("https://github.com/fork/repo.git"));
    }

    #[test]
    fn discover_repository_or_fallback_succeeds_for_temp_dir() {
        let dir = tempdir().unwrap();
        // Should not panic — fallback path returns a valid context from CWD
        let ctx = discover_repository_or_fallback(dir.path());
        assert!(!ctx.repository_id.is_empty());
    }
}
