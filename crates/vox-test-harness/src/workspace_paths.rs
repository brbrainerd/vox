//! Locate the workspace repository root from integration tests.
//!
//! Integration tests should pass [`std::path::Path::new`]`(env!("CARGO_MANIFEST_DIR"))` as the
//! starting directory — never assume the process current directory.

use std::path::{Path, PathBuf};

/// Walk parents from `start` until a `Cargo.toml` containing a `[workspace]` table is found.
pub fn find_workspace_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().to_path_buf();
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file()
            && let Ok(txt) = std::fs::read_to_string(&manifest)
            && txt.lines().any(|l| l.trim() == "[workspace]")
        {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Prefer [`vox_secrets::SecretId::VoxRepositoryRoot`] (`VOX_REPO_ROOT`) when it points at a checkout root; otherwise walk up from `start`.
pub fn repo_root_for_tests(start: impl AsRef<Path>) -> PathBuf {
    if let Some(raw) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRepositoryRoot).expose()
    {
        let p = PathBuf::from(raw.trim());
        if p.join("Cargo.toml").is_file() {
            return p;
        }
    }
    find_workspace_root(start.as_ref()).unwrap_or_else(|| {
        panic!(
            "could not find workspace root above {}",
            start.as_ref().display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_from_manifest_dir() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = find_workspace_root(manifest).expect("workspace root");
        assert!(root.join("AGENTS.md").is_file());
        assert!(root.join("contracts/config/env-vars.v1.yaml").is_file());
    }
}
