use crate::features::ExtractedFeatures;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub struct FeatureCache {
    dir: PathBuf,
}

impl FeatureCache {
    pub fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        Self { dir }
    }

    pub fn from_workspace(root: &std::path::Path) -> Self {
        Self::new(root.join(vox_config::paths::REPO_DRIFT_CACHE_DIR))
    }

    pub fn hash_file(content: &str) -> String {
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        format!("{:x}", h.finalize())
    }

    pub fn store(&self, key: &str, features: &ExtractedFeatures) -> Result<()> {
        let path = self.dir.join(format!("{}.bin", &key[..16.min(key.len())]));
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(CACHE_MAGIC);
        bytes.push(CACHE_VERSION);
        bincode::serde::encode_into_std_write(features, &mut bytes, bincode::config::standard())?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load(&self, key: &str) -> Option<ExtractedFeatures> {
        let path = self.dir.join(format!("{}.bin", &key[..16.min(key.len())]));
        let bytes = std::fs::read(path).ok()?;
        // Schema-mismatched cache entries previously caused bincode to decode
        // garbage past the end (capacity_overflow panic on Vec allocation).
        // Now: magic-byte + version prefix; older or differently-shaped entries
        // are treated as cache misses rather than re-extracted into nonsense.
        if bytes.len() < CACHE_MAGIC.len() + 1
            || &bytes[..CACHE_MAGIC.len()] != CACHE_MAGIC
            || bytes[CACHE_MAGIC.len()] != CACHE_VERSION
        {
            return None;
        }
        let payload = &bytes[CACHE_MAGIC.len() + 1..];
        bincode::serde::decode_from_slice::<ExtractedFeatures, _>(
            payload,
            bincode::config::standard(),
        )
        .ok()
        .map(|(f, _)| f)
    }
}

/// 8-byte magic so old (pre-2026-05) raw-bincode cache entries are detected as
/// schema-mismatched and re-extracted instead of panicking on garbage decode.
const CACHE_MAGIC: &[u8; 8] = b"VOXDRIFT";

/// Bump on any change to the on-disk shape of `ExtractedFeatures`:
/// - 1: initial format
/// - 2 (2026-05-28): NumericLoc.in_const + ExtractedFeatures.allowed_lines
const CACHE_VERSION: u8 = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{LiteralContext, LiteralLoc, Loc};
    use vox_code_audit::rules::Language;

    #[test]
    fn cache_round_trips_features() {
        let dir = tempfile::TempDir::new().unwrap();
        let cache = FeatureCache::new(dir.path().to_path_buf());

        let mut f = ExtractedFeatures::new(std::path::PathBuf::from("test.rs"), Language::Rust);
        f.string_literals.push(LiteralLoc {
            value: "hi".into(),
            loc: Loc::default(),
            ctx: LiteralContext::Code,
        });

        let key = "abc123deadbeef0000000000";
        cache.store(key, &f).unwrap();
        let loaded = cache.load(key).unwrap();
        assert_eq!(loaded.string_literals[0].value, "hi");
    }

    #[test]
    fn hash_file_is_deterministic() {
        let h1 = FeatureCache::hash_file("hello world");
        let h2 = FeatureCache::hash_file("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn load_returns_none_for_missing_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let cache = FeatureCache::new(dir.path().to_path_buf());
        assert!(cache.load("nonexistentkey00").is_none());
    }
}
