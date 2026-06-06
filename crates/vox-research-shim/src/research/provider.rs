//! Web provider registry. Phase 0a STUB — single in-memory "stub" provider.
//!
//! Phase 5 wires this to real providers via `vox-search`'s SearXNG/DDG/Tavily
//! adapters and Phase 6 introduces `ProviderObservation` per Mesh §4.1.

use serde::{Deserialize, Serialize};

use super::types::{ResearchHit, ResearchQuery};

/// Configuration for the provider registry. Phase 0a — fields are placeholders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub primary: Option<String>,
    pub fallback: Vec<String>,
}

/// Registry of web search providers used by the research pipeline.
///
/// **PHASE_0a_STUB**: in-memory only; all search/crawl/extract operations
/// return empty collections. Phase 5 replaces with real provider adapters.
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    primary: String,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            primary: "stub".to_string(),
        }
    }
}

impl ProviderRegistry {
    /// Construct from environment + supplied config. Phase 0a ignores both.
    #[must_use]
    pub fn from_env_with_config(_config: ProviderConfig) -> Self {
        // PHASE_0a_STUB: replaced by real provider resolution in Phase 5.
        Self::default()
    }

    /// Name of the primary provider for telemetry attribution.
    #[must_use]
    pub fn primary_name(&self) -> &str {
        &self.primary
    }

    /// Search for hits matching the query. Phase 0a — returns empty Vec.
    ///
    /// Returns `(hits, provider_name_used)`.
    ///
    /// **PHASE_0a_STUB**
    pub async fn search(&self, _query: &ResearchQuery) -> (Vec<ResearchHit>, String) {
        // PHASE_0a_STUB: replaced by real provider search in Phase 5.
        (Vec::new(), self.primary.clone())
    }

    /// Discover child pages for a site root URL. Phase 0a — returns None.
    ///
    /// **PHASE_0a_STUB**
    pub async fn map_site(&self, _root_url: &str) -> Option<Vec<String>> {
        // PHASE_0a_STUB: replaced by real site-mapper in Phase 5.
        None
    }
}
