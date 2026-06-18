//! Web provider registry — thin delegate over `vox-search::WebSearchDispatcher`.
//!
//! The research pipeline attributes telemetry to [`ProviderRegistry::primary_name`]
//! while retrieval executes through the shared vox-search web stack (SearXNG → DDG → Tavily).

use serde::{Deserialize, Serialize};
use vox_search::policy::SearchPolicy;
use vox_search::web_dispatcher::WebSearchDispatcher;

use super::types::ResearchHit;

/// Configuration for the provider registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub primary: Option<String>,
    pub fallback: Vec<String>,
}

/// Registry of web search providers used by the research pipeline.
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    primary: String,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            primary: "vox-search/web".to_string(),
        }
    }
}

impl ProviderRegistry {
    /// Construct from environment + supplied config.
    #[must_use]
    pub fn from_env_with_config(config: ProviderConfig) -> Self {
        if let Some(name) = config
            .primary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Self {
                primary: name.to_string(),
            }
        } else {
            Self::default()
        }
    }

    /// Name of the primary provider for telemetry attribution.
    #[must_use]
    pub fn primary_name(&self) -> &str {
        &self.primary
    }

    /// Search for hits matching `query` via [`WebSearchDispatcher`].
    ///
    /// Returns `(hits, provider_name_used)`.
    pub async fn search(&self, query: &str, policy: &SearchPolicy) -> (Vec<ResearchHit>, String) {
        match WebSearchDispatcher::search(query, policy).await {
            Ok(hybrids) => {
                let hits = hybrids
                    .into_iter()
                    .map(|h| ResearchHit {
                        url: h.path,
                        title: h.title,
                        snippet: h.content_snippet,
                        score: h.score,
                        http_status: 0,
                        trust_score: 1.0,
                        raw_content: String::new(),
                    })
                    .collect();
                (hits, self.primary.clone())
            }
            Err(e) => {
                tracing::warn!(error = %e, "provider registry web search failed");
                (Vec::new(), self.primary.clone())
            }
        }
    }

    /// Discover child pages for a site root URL.
    ///
    /// Site-scoped crawling is handled at gather time via `ResearchQuery::site_scope`
    /// filtering; no dedicated site-map API exists in vox-search yet.
    pub async fn map_site(&self, _root_url: &str) -> Option<Vec<String>> {
        None
    }
}
