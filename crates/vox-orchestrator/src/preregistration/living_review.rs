//! Living-review versioned-DOI management — SCIENTIA Phase 3.
//!
//! Living-review semantics: each publication creates a new immutable DOI version;
//! the canonical URL always points to "latest". [`LivingReviewManifest`] tracks the
//! full `version_history` Vec (oldest first) and keeps `latest_doi` up to date.

use serde::{Deserialize, Serialize};

/// An immutable snapshot of one published version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoiVersion {
    pub doi: String,
    /// 1-based version number (first published = 1).
    pub version: u32,
    /// Unix timestamp (seconds) when this version was published.
    pub published_at: i64,
    /// URL resolving to this specific version (version-pinned).
    pub canonical_url: String,
    /// Human-readable description of what changed in this version.
    pub description: Option<String>,
}

/// A living-review manifest: mutable, append-only version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivingReviewManifest {
    pub title: String,
    /// Always points to the latest published version (updated by [`Self::add_version`]).
    pub canonical_url: String,
    /// DOI of the most recently added version.
    pub latest_doi: String,
    /// Full version history, oldest first.
    pub version_history: Vec<DoiVersion>,
}

impl LivingReviewManifest {
    /// Create an empty manifest with no versions.
    ///
    /// `canonical_url` should be the stable "latest" URL that will be updated
    /// each time [`Self::add_version`] is called.
    pub fn new(title: String, canonical_url: String) -> Self {
        Self {
            title,
            canonical_url,
            latest_doi: String::new(),
            version_history: Vec::new(),
        }
    }

    /// Append a new published version.
    ///
    /// - `version` is auto-incremented (1-based).
    /// - `self.latest_doi` is updated to the new DOI.
    /// - `self.canonical_url` is updated to the new version's `canonical_url`.
    pub fn add_version(
        &mut self,
        doi: String,
        canonical_url: String,
        published_at: i64,
        description: Option<String>,
    ) {
        let version = self.version_history.len() as u32 + 1;
        self.latest_doi = doi.clone();
        self.canonical_url = canonical_url.clone();
        self.version_history.push(DoiVersion {
            doi,
            version,
            published_at,
            canonical_url,
            description,
        });
    }

    /// Returns the most recently added version, or `None` if no versions exist.
    pub fn latest_version(&self) -> Option<&DoiVersion> {
        self.version_history.last()
    }

    /// Total number of published versions.
    pub fn version_count(&self) -> usize {
        self.version_history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest() -> LivingReviewManifest {
        LivingReviewManifest::new(
            "Provider Atlas: Reliability Edition".to_string(),
            "https://vox.research/atlas/latest".to_string(),
        )
    }

    #[test]
    fn new_manifest_has_no_versions() {
        let manifest = base_manifest();
        assert_eq!(
            manifest.version_count(),
            0,
            "new manifest must have 0 versions"
        );
        assert!(
            manifest.latest_version().is_none(),
            "latest_version must be None when empty"
        );
        assert!(
            manifest.latest_doi.is_empty(),
            "latest_doi must be empty string initially"
        );
    }

    #[test]
    fn add_version_updates_latest_doi() {
        let mut manifest = base_manifest();
        manifest.add_version(
            "10.5281/zenodo.100001".to_string(),
            "https://vox.research/atlas/v1".to_string(),
            1_767_225_600,
            Some("Initial release".to_string()),
        );
        assert_eq!(manifest.latest_doi, "10.5281/zenodo.100001");
        assert_eq!(manifest.canonical_url, "https://vox.research/atlas/v1");
    }

    #[test]
    fn add_version_increments_version_number() {
        let mut manifest = base_manifest();
        manifest.add_version(
            "10.5281/zenodo.100001".to_string(),
            "https://vox.research/atlas/v1".to_string(),
            1_767_225_600,
            None,
        );
        manifest.add_version(
            "10.5281/zenodo.100002".to_string(),
            "https://vox.research/atlas/v2".to_string(),
            1_767_312_000,
            None,
        );
        let v1 = &manifest.version_history[0];
        let v2 = &manifest.version_history[1];
        assert_eq!(v1.version, 1, "first version must be 1");
        assert_eq!(v2.version, 2, "second version must be 2");
    }

    #[test]
    fn version_history_is_ordered_oldest_first() {
        let mut manifest = base_manifest();
        for i in 1u32..=3 {
            manifest.add_version(
                format!("10.5281/zenodo.10000{i}"),
                format!("https://vox.research/atlas/v{i}"),
                1_767_225_600 + (i as i64 - 1) * 86_400,
                None,
            );
        }
        assert_eq!(manifest.version_count(), 3);
        let versions: Vec<u32> = manifest.version_history.iter().map(|v| v.version).collect();
        assert_eq!(
            versions,
            vec![1, 2, 3],
            "version_history must be ordered oldest-first (1, 2, 3)"
        );
        assert_eq!(
            manifest.latest_version().unwrap().doi,
            "10.5281/zenodo.100003"
        );
    }
}
