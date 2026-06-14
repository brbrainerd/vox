//! Locate a Git work tree by walking parents for `.git`.

use std::path::{Path, PathBuf};

/// Walk upward from `start` and return the directory that contains `.git`, if any.
pub fn find_git_work_tree(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn find_git_work_tree_returns_none_when_no_git_dir() {
        let dir = tempdir().unwrap();
        let result = find_git_work_tree(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn find_git_work_tree_finds_dot_git_at_root() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let result = find_git_work_tree(dir.path());
        assert!(result.is_some());
        let found = result.unwrap();
        assert_eq!(found, dir.path());
    }

    #[test]
    fn find_git_work_tree_walks_up_to_parent() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let sub = dir.path().join("sub").join("deep");
        fs::create_dir_all(&sub).unwrap();
        let result = find_git_work_tree(&sub);
        assert!(result.is_some());
        let found = result.unwrap();
        assert_eq!(found, dir.path());
    }
}
