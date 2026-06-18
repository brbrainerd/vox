//! Venue catalog loader for critic-gate policy (Phase E).
//!
//! Reads `contracts/scientia/venue-catalog.v1.yaml` — the same SSOT consumed
//! by `vox-publisher::venue_catalog`, but trimmed to gate-relevant fields.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

use super::venue::VenueCriticPolicy;

#[derive(Debug, Error)]
pub enum VenueCatalogError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueCatalogEntry {
    pub id: String,
    #[serde(default = "default_allows_llm_critic")]
    pub allows_llm_critic: bool,
}

fn default_allows_llm_critic() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueCatalog {
    pub schema_version: u32,
    pub venues: Vec<VenueCatalogEntry>,
}

impl VenueCatalog {
    pub fn from_yaml(yaml: &str) -> Result<Self, VenueCatalogError> {
        serde_yaml::from_str(yaml).map_err(|e| VenueCatalogError::Yaml(e.to_string()))
    }

    pub fn load_from_repo(repo_root: &Path) -> Result<Self, VenueCatalogError> {
        let path = repo_root.join("contracts/scientia/venue-catalog.v1.yaml");
        let raw = std::fs::read_to_string(&path)?;
        Self::from_yaml(&raw)
    }

    pub fn find_by_id(&self, id: &str) -> Option<&VenueCatalogEntry> {
        self.venues.iter().find(|v| v.id == id)
    }

    pub fn critic_policy_for_venue_id(&self, venue_id: &str) -> VenueCriticPolicy {
        match self.find_by_id(venue_id) {
            Some(v) if v.allows_llm_critic => VenueCriticPolicy::Allowed,
            Some(_) => VenueCriticPolicy::Forbidden,
            None => VenueCriticPolicy::Forbidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema_version: 1
venues:
  - id: imc
    allows_llm_critic: false
  - id: tmlr
    allows_llm_critic: true
"#;

    #[test]
    fn catalog_parses_allows_llm_critic() {
        let cat = VenueCatalog::from_yaml(SAMPLE).unwrap();
        assert!(!cat.find_by_id("imc").unwrap().allows_llm_critic);
        assert!(cat.find_by_id("tmlr").unwrap().allows_llm_critic);
    }

    #[test]
    fn critic_policy_for_venue_respects_catalog_flag() {
        let cat = VenueCatalog::from_yaml(SAMPLE).unwrap();
        assert_eq!(
            cat.critic_policy_for_venue_id("imc"),
            VenueCriticPolicy::Forbidden
        );
        assert_eq!(
            cat.critic_policy_for_venue_id("tmlr"),
            VenueCriticPolicy::Allowed
        );
    }
}
