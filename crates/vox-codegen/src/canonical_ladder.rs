//! Loader for `contracts/pipeline/canonical-ladder.v1.yaml` — SSOT for backwards emission verification.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One curated golden fixture and the language concerns it proves end-to-end.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LadderFixture {
    pub id: String,
    pub path: String,
    pub proves: Vec<String>,
    pub targets: Vec<String>,
}

/// Parsed canonical ladder contract.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CanonicalLadder {
    #[serde(rename = "x-vox-version")]
    pub schema_version: u32,
    pub fixtures: Vec<LadderFixture>,
}

impl CanonicalLadder {
    /// Load ladder YAML from `repo_root/contracts/pipeline/canonical-ladder.v1.yaml`.
    pub fn load_from_repo_root(repo_root: &Path) -> Result<Self, String> {
        let path = repo_root.join("contracts/pipeline/canonical-ladder.v1.yaml");
        let raw =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
    }

    /// Find a fixture by stable `id`.
    #[must_use]
    pub fn fixture(&self, id: &str) -> Option<&LadderFixture> {
        self.fixtures.iter().find(|f| f.id == id)
    }

    /// Stable fixture ids (for k-complexity budget scoping).
    #[must_use]
    pub fn fixture_ids(&self) -> BTreeSet<String> {
        self.fixtures.iter().map(|f| f.id.clone()).collect()
    }

    /// Union of all `proves` tags across fixtures.
    #[must_use]
    pub fn all_proves_tags(&self) -> BTreeSet<String> {
        self.fixtures
            .iter()
            .flat_map(|f| f.proves.iter().cloned())
            .collect()
    }

    /// Absolute path to a golden `.vox` file under `repo_root`.
    #[must_use]
    pub fn fixture_vox_path(&self, repo_root: &Path, id: &str) -> Option<PathBuf> {
        self.fixture(id).map(|f| repo_root.join(&f.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn ladder_loads_twelve_fixtures() {
        let ladder = CanonicalLadder::load_from_repo_root(&repo_root()).expect("load ladder");
        assert_eq!(ladder.schema_version, 1);
        assert_eq!(ladder.fixtures.len(), 12, "{:?}", ladder.fixtures);
        assert!(ladder.fixture("hello").is_some());
        assert!(ladder.fixture("crud_api").is_some());
    }
}
