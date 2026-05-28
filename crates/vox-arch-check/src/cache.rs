//! Persistent on-disk cache for arch-check git-derived data.
//!
//! Cache key: SHA-256 of Cargo.lock + layers.toml + where-things-live.md (concatenated).
//! Cache location: `<workspace_root>/target/vox-arch-check-cache/`.
//! Cache file: `<key>.json` — contains serialized `CachedData`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// The data we cache to avoid re-running expensive git operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedData {
    /// SHA-256 of inputs (Cargo.lock + layers.toml + where-things-live.md).
    pub key: String,
    /// Paths touched since last release date (Rule 8). `None` if git failed.
    pub git_touched_paths: Option<Vec<String>>,
}

/// Compute the cache key from the three input files.
pub fn compute_key(workspace_root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    for rel in &[
        "Cargo.lock",
        "docs/src/architecture/layers.toml",
        "docs/src/architecture/where-things-live.md",
    ] {
        let path = workspace_root.join(rel);
        let contents = std::fs::read(&path)
            .with_context(|| format!("read {} for cache key", path.display()))?;
        hasher.update(&contents);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn cache_path(workspace_root: &Path, key: &str) -> std::path::PathBuf {
    workspace_root
        .join("target")
        .join("vox-arch-check-cache")
        .join(format!("{key}.json"))
}

/// Try to load a valid cache hit. Returns `None` if absent, stale, or corrupt.
pub fn load(workspace_root: &Path, key: &str) -> Option<CachedData> {
    let path = cache_path(workspace_root, key);
    let bytes = std::fs::read(&path).ok()?;
    let data: CachedData = serde_json::from_slice(&bytes).ok()?;
    if data.key != key {
        return None;
    }
    Some(data)
}

/// Persist cache data for a given key.
pub fn store(workspace_root: &Path, data: &CachedData) -> Result<()> {
    let path = cache_path(workspace_root, &data.key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(data)?;
    std::fs::write(&path, bytes).with_context(|| path.display().to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_workspace(root: &Path) {
        fs::write(root.join("Cargo.lock"), b"# test lock").unwrap();
        fs::create_dir_all(root.join("docs/src/architecture")).unwrap();
        fs::write(
            root.join("docs/src/architecture/layers.toml"),
            b"# test layers",
        )
        .unwrap();
        fs::write(
            root.join("docs/src/architecture/where-things-live.md"),
            b"# test wtl",
        )
        .unwrap();
    }

    #[test]
    fn compute_key_is_deterministic() {
        let tmp = tempdir().unwrap();
        make_workspace(tmp.path());
        let k1 = compute_key(tmp.path()).unwrap();
        let k2 = compute_key(tmp.path()).unwrap();
        assert_eq!(k1, k2, "key must be deterministic");
        assert_eq!(k1.len(), 64, "SHA-256 hex is 64 chars");
    }

    #[test]
    fn compute_key_changes_when_input_changes() {
        let tmp = tempdir().unwrap();
        make_workspace(tmp.path());
        let k1 = compute_key(tmp.path()).unwrap();
        fs::write(tmp.path().join("Cargo.lock"), b"# changed lock").unwrap();
        let k2 = compute_key(tmp.path()).unwrap();
        assert_ne!(k1, k2, "key must change when Cargo.lock changes");
    }

    #[test]
    fn round_trip_store_load() {
        let tmp = tempdir().unwrap();
        make_workspace(tmp.path());
        let key = compute_key(tmp.path()).unwrap();
        let data = CachedData {
            key: key.clone(),
            git_touched_paths: Some(vec!["crates/foo/src/lib.rs".to_string()]),
        };
        store(tmp.path(), &data).unwrap();
        let loaded = load(tmp.path(), &key).unwrap();
        assert_eq!(loaded.key, key);
        assert_eq!(
            loaded.git_touched_paths,
            Some(vec!["crates/foo/src/lib.rs".to_string()])
        );
    }

    #[test]
    fn load_returns_none_for_missing_cache() {
        let tmp = tempdir().unwrap();
        make_workspace(tmp.path());
        let key = compute_key(tmp.path()).unwrap();
        assert!(
            load(tmp.path(), &key).is_none(),
            "should return None when no cache file"
        );
    }

    #[test]
    fn load_returns_none_for_wrong_key() {
        let tmp = tempdir().unwrap();
        make_workspace(tmp.path());
        let key = compute_key(tmp.path()).unwrap();
        let wrong_key = "a".repeat(64);
        let data = CachedData {
            key: wrong_key.clone(),
            git_touched_paths: None,
        };
        // Store with wrong key path, then try to load with correct key — should miss
        let wrong_path = tmp
            .path()
            .join("target/vox-arch-check-cache")
            .join(format!("{wrong_key}.json"));
        fs::create_dir_all(wrong_path.parent().unwrap()).unwrap();
        fs::write(&wrong_path, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(load(tmp.path(), &key).is_none());
    }
}
