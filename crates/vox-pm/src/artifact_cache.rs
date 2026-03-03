//! # Artifact Cache
//!
//! A content-addressed cache for Vox build outputs, inspired by Zig's `.zig-cache/`
//! directory with SHA-256 input manifests and per-hash artifact directories.
//!
//! ## How It Works
//!
//! 1. Hash all build inputs (source files + `Vox.toml` + deps) with SHA3-512.
//! 2. Check `.vox-cache/manifests/<hash>.json` for a previously written entry.
//! 3. **Cache hit** → return path to `.vox-cache/artifacts/<hash>/`, skip rebuild.
//! 4. **Cache miss** → compile, write outputs to `.vox-cache/artifacts/<hash>/`,
//!    record the manifest entry.
//!
//! This eliminates redundant compilations in CI pipelines, multi-step deploys, and
//! rapid development cycles.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hash::content_hash;

/// A content-addressed cache for Vox build artifacts.
///
/// Uses SHA3-512 over all build inputs as the cache key.
/// Artifacts are stored under `.vox-cache/artifacts/<hash>/`.
#[derive(Debug, Clone)]
pub struct ArtifactCache {
    /// Root of the cache, e.g. `<project>/.vox-cache` or `~/.vox/artifact-cache`.
    pub root: PathBuf,
}

/// Metadata recorded alongside a cached artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    /// Hex-encoded SHA3-512 hash of all build inputs.
    pub input_hash: String,
    /// Unix timestamp (seconds) when this entry was written.
    pub created_at: u64,
    /// Human-readable description of what was built.
    pub description: String,
    /// Files stored in the artifact directory, relative to it.
    pub files: Vec<String>,
}

/// Outcome of a cache lookup.
pub enum CacheLookup {
    /// Cached artifacts exist; the path points to the artifact directory.
    Hit { artifact_dir: PathBuf, manifest: CacheManifest },
    /// No cached artifacts. The caller should build and then call `record_build`.
    Miss { input_hash: String },
}

impl ArtifactCache {
    /// Create (or open) a cache rooted at `root`.
    ///
    /// Typically `root` is `<project-root>/.vox-cache`.
    pub fn new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(root.join("manifests"))?;
        fs::create_dir_all(root.join("artifacts"))?;
        Ok(Self { root })
    }

    /// Open the default project-local cache at `<project_root>/.vox-cache`.
    pub fn default_for(project_root: &Path) -> io::Result<Self> {
        Self::new(project_root.join(".vox-cache"))
    }

    /// Compute a cache key from a set of input paths.
    ///
    /// Reads every file listed in `input_paths`, appends their contents together
    /// with their relative path as a separator, then returns the SHA3-512 hash.
    ///
    /// Includes `extra_inputs` (e.g. serialized config strings) to capture
    /// non-file inputs such as CLI flags or dependency versions.
    pub fn compute_input_hash(
        input_paths: &[PathBuf],
        extra_inputs: &[&str],
    ) -> io::Result<String> {
        let mut hasher_input: Vec<u8> = Vec::new();

        // Sort for determinism
        let mut sorted = input_paths.to_vec();
        sorted.sort();

        for path in &sorted {
            // Include the path itself as part of the hash material
            hasher_input.extend_from_slice(path.to_string_lossy().as_bytes());
            hasher_input.push(b'\0');
            if path.exists() {
                hasher_input.extend_from_slice(&fs::read(path)?);
            }
            hasher_input.push(b'\0');
        }

        for extra in extra_inputs {
            hasher_input.extend_from_slice(extra.as_bytes());
            hasher_input.push(b'\0');
        }

        Ok(content_hash(&hasher_input))
    }

    /// Look up whether artifacts exist for the given input hash.
    pub fn lookup(&self, input_hash: &str) -> CacheLookup {
        let manifest_path = self.manifest_path(input_hash);
        let artifact_dir = self.artifact_dir(input_hash);

        if manifest_path.exists() && artifact_dir.exists() {
            match fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|s| serde_json::from_str::<CacheManifest>(&s).ok())
            {
                Some(manifest) => CacheLookup::Hit { artifact_dir, manifest },
                None => CacheLookup::Miss { input_hash: input_hash.to_string() },
            }
        } else {
            CacheLookup::Miss { input_hash: input_hash.to_string() }
        }
    }

    /// Record a completed build, copying `output_files` into the artifact directory.
    ///
    /// `output_files` is a list of `(source_path, filename_in_cache)` pairs.
    pub fn record_build(
        &self,
        input_hash: &str,
        description: &str,
        output_files: &[(PathBuf, String)],
    ) -> io::Result<PathBuf> {
        let artifact_dir = self.artifact_dir(input_hash);
        fs::create_dir_all(&artifact_dir)?;

        let mut file_names = Vec::new();
        for (src, name) in output_files {
            let dst = artifact_dir.join(name);
            fs::copy(src, &dst)?;
            file_names.push(name.clone());
        }

        let manifest = CacheManifest {
            input_hash: input_hash.to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            description: description.to_string(),
            files: file_names,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(self.manifest_path(input_hash), manifest_json)?;

        Ok(artifact_dir)
    }

    /// Delete all cached artifacts older than `max_age_secs` seconds.
    pub fn prune(&self, max_age_secs: u64) -> io::Result<usize> {
        let manifests_dir = self.root.join("manifests");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut pruned = 0;
        if !manifests_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&manifests_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let manifest: CacheManifest = match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if now.saturating_sub(manifest.created_at) > max_age_secs {
                // Remove manifest and artifact directory
                let _ = fs::remove_file(&path);
                let artifact_dir = self.artifact_dir(&manifest.input_hash);
                if artifact_dir.exists() {
                    let _ = fs::remove_dir_all(&artifact_dir);
                }
                pruned += 1;
            }
        }

        Ok(pruned)
    }

    /// Path to the manifest JSON file for a given input hash.
    pub fn manifest_path(&self, input_hash: &str) -> PathBuf {
        // Use first 64 chars to keep filenames manageable
        let short = &input_hash[..input_hash.len().min(64)];
        self.root.join("manifests").join(format!("{short}.json"))
    }

    /// Path to the artifact directory for a given input hash.
    pub fn artifact_dir(&self, input_hash: &str) -> PathBuf {
        let short = &input_hash[..input_hash.len().min(64)];
        self.root.join("artifacts").join(short)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_miss_then_hit() {
        let tmp = tempdir().unwrap();
        let cache = ArtifactCache::new(tmp.path().join("cache")).unwrap();

        let hash = "abc123def456";

        // Initial lookup is a miss
        assert!(matches!(cache.lookup(hash), CacheLookup::Miss { .. }));

        // Write a dummy output file
        let out_file = tmp.path().join("output.js");
        fs::write(&out_file, b"console.log('hi')").unwrap();

        cache
            .record_build(hash, "test build", &[(out_file, "output.js".into())])
            .unwrap();

        // Now it should be a hit
        match cache.lookup(hash) {
            CacheLookup::Hit { manifest, artifact_dir } => {
                assert_eq!(manifest.input_hash, hash);
                assert_eq!(manifest.description, "test build");
                assert!(artifact_dir.join("output.js").exists());
            }
            CacheLookup::Miss { .. } => panic!("Expected a cache hit"),
        }
    }

    #[test]
    fn test_input_hash_deterministic() {
        let tmp = tempdir().unwrap();
        let file_a = tmp.path().join("a.vox");
        fs::write(&file_a, b"component Foo {}").unwrap();

        let h1 = ArtifactCache::compute_input_hash(&[file_a.clone()], &["extra"]).unwrap();
        let h2 = ArtifactCache::compute_input_hash(&[file_a], &["extra"]).unwrap();
        assert_eq!(h1, h2, "Same inputs must produce same hash");
    }

    #[test]
    fn test_input_hash_changes_on_content_change() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("main.vox");
        fs::write(&file, b"component A {}").unwrap();
        let h1 = ArtifactCache::compute_input_hash(&[file.clone()], &[]).unwrap();

        fs::write(&file, b"component B {}").unwrap();
        let h2 = ArtifactCache::compute_input_hash(&[file], &[]).unwrap();
        assert_ne!(h1, h2, "Different content must produce different hash");
    }

    #[test]
    fn test_prune_removes_old_entries() {
        let tmp = tempdir().unwrap();
        let cache = ArtifactCache::new(tmp.path().join("cache")).unwrap();

        let hash = "old_entry_hash";
        let out_file = tmp.path().join("old.js");
        fs::write(&out_file, b"old").unwrap();
        cache
            .record_build(hash, "old build", &[(out_file, "old.js".into())])
            .unwrap();

        // Manually backdate the manifest
        let manifest_path = cache.manifest_path(hash);
        let mut manifest: CacheManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.created_at = 0; // epoch = very old
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        // Prune entries older than 1 second
        let pruned = cache.prune(1).unwrap();
        assert_eq!(pruned, 1);
        assert!(matches!(cache.lookup(hash), CacheLookup::Miss { .. }));
    }
}
