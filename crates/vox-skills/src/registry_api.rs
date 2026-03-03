//! Vox Skills bridge — HTTP client for the Vox Skills registry.
//!
//! Enabled with feature `skills-registry`. Without it, all methods return an error.

use serde::{Deserialize, Serialize};

use crate::bundle::VoxSkillBundle;
use crate::manifest::SkillManifest;
use crate::SkillError;

/// A search result from the Vox Skills registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsRegistryResult {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub stars: u32,
}

/// HTTP client for the Vox Skills marketplace.
pub struct SkillsRegistryClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl SkillsRegistryClient {
    /// Create a client pointing at the official Vox Skills registry.
    pub fn new() -> Self {
        Self::with_base(crate::SKILLS_REGISTRY_BASE)
    }

    /// Create a client pointing at a custom registry URL.
    pub fn with_base(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Search the registry for skills matching a keyword.
    pub async fn search(&self, query: &str) -> Result<Vec<SkillsRegistryResult>, SkillError> {
        let url = format!("{}/search?q={}", self.base_url, urlencoding(query));
        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(SkillError::Http(format!("HTTP {}", resp.status())));
        }
        resp.json::<Vec<SkillsRegistryResult>>()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))
    }

    /// Download a skill bundle by ID and optional version.
    pub async fn download(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<VoxSkillBundle, SkillError> {
        let ver = version.unwrap_or("latest");
        let url = format!("{}/skills/{id}/{ver}/bundle.json", self.base_url);
        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(SkillError::Http(format!(
                "HTTP {} for skill {id}@{ver}",
                resp.status()
            )));
        }
        let json = resp
            .text()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))?;
        VoxSkillBundle::from_json(&json)
    }

    /// Publish a skill bundle to the registry.
    pub async fn publish(&self, bundle: &VoxSkillBundle) -> Result<(), SkillError> {
        if self.api_key.is_none() {
            return Err(SkillError::Http("API key required to publish".into()));
        }
        let url = format!("{}/skills/publish", self.base_url);
        let json = bundle.to_json()?;
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(json);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(SkillError::Http(format!(
                "Publish failed: HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    /// Get metadata for a specific skill.
    pub async fn get_manifest(&self, id: &str) -> Result<SkillManifest, SkillError> {
        let url = format!("{}/skills/{id}/manifest.json", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(SkillError::Http(format!("HTTP {}", resp.status())));
        }
        resp.json::<SkillManifest>()
            .await
            .map_err(|e| SkillError::Http(e.to_string()))
    }
}

impl Default for SkillsRegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => format!("%{:02X}", other as u32),
        })
        .collect()
}
