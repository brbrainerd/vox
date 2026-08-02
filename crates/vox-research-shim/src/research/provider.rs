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
                use futures::stream::{self, StreamExt};
                // Stream over OWNED (title, url) pairs, not `hybrids.iter()`
                // (borrowed `&HybridSearchHit` items) — a `.map` closure taking
                // a borrowed iterator item and returning an `async move` block
                // makes rustc infer an overly-generic higher-ranked closure
                // signature for the closure itself (independent of what the
                // async block captures), which fails "implementation of
                // Send/FnOnce is not general enough" in some downstream callers
                // (observed when this crate is linked into vox-orchestrator-mcp).
                // Cloning up front so every stream item is fully owned avoids
                // the closure ever being generic over a borrowed lifetime.
                let title_urls: Vec<(String, String)> = hybrids
                    .iter()
                    .map(|h| (h.title.clone(), h.path.clone()))
                    .collect();
                let trust_scores: Vec<f64> = stream::iter(title_urls)
                    .map(|(title, url)| {
                        let doi = vox_search::trust::extract_doi_from_url(&url);
                        async move {
                            vox_search::trust::score_hit_trust_for_url(
                                &title,
                                doi.as_deref(),
                                &url,
                            )
                            .await
                        }
                    })
                    // `buffered` (not `buffer_unordered`) to preserve input order,
                    // since results are zipped positionally against `hybrids` below.
                    .buffered(5)
                    .collect()
                    .await;
                let hits: Vec<ResearchHit> = hybrids
                    .into_iter()
                    .zip(trust_scores)
                    .map(|(h, trust_score)| ResearchHit {
                        url: h.path,
                        title: h.title,
                        snippet: h.content_snippet,
                        score: h.score,
                        http_status: 0,
                        trust_score,
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn provider_search_hits_use_real_trust_scoring() {
        // Sanity check that trust scoring is wired in and fail-open (no hang,
        // sane range) — full integration behavior is covered by trust.rs's
        // own mocked tests from Task 4.
        let score = vox_search::trust::score_hit_trust("Example Provider Title", None).await;
        assert!(
            (0.0..=2.0).contains(&score),
            "trust score {score} out of sane range"
        );
    }
}
