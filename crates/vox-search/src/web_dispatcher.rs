use std::collections::HashSet;

use tracing::{info, warn};

use crate::policy::SearchPolicy;

pub struct WebSearchDispatcher;

impl WebSearchDispatcher {
    pub async fn search(
        query: &str,
        policy: &SearchPolicy,
    ) -> anyhow::Result<Vec<crate::memory_hybrid::HybridSearchHit>> {
        let mut results = Vec::new();

        // Tier 2: SearXNG
        if let Some(base_url) = &policy.searxng_url {
            let client = crate::searxng::SearxngSearchClient::new(base_url.clone());
            match client
                .search(
                    query,
                    policy.searxng_max_results,
                    policy.searxng_engines_csv(),
                    policy.searxng_language_tag(),
                )
                .await
            {
                Ok(hits) => {
                    info!(count = hits.len(), "SearXNG search succeeded");
                    results = hits;
                }
                Err(e) => {
                    warn!(error = %e, "SearXNG search failed, falling back");
                }
            }
        }

        // Tier 3: DuckDuckGo Fallback
        if results.is_empty() && policy.duckduckgo_fallback_enabled {
            match crate::duckduckgo::DuckDuckGoClient::search(query, policy.searxng_max_results)
                .await
            {
                Ok(hits) => {
                    info!(count = hits.len(), "DuckDuckGo fallback succeeded");
                    results = hits;
                }
                Err(e) => {
                    warn!(error = %e, "DuckDuckGo fallback failed");
                }
            }
        }

        // Tier 4: Tavily (when SearXNG + DDG produced nothing and policy allows it)
        #[cfg(feature = "tavily")]
        if results.is_empty()
            && policy.tavily_enabled
            && let Some(client) = crate::tavily::TavilySearchClient::from_env()
        {
            match client
                .search(
                    query,
                    policy.tavily_max_results,
                    policy.tavily_search_depth.as_str(),
                )
                .await
            {
                Ok(hits) => {
                    info!(count = hits.len(), "Tavily web fallback succeeded");
                    results = hits
                        .into_iter()
                        .map(|h| crate::searxng::SearxngResult {
                            url: h.url,
                            title: h.title.clone(),
                            content: h.content,
                            engine: Some("tavily".to_string()),
                            score: Some(f64::from(h.score)),
                        })
                        .collect();
                }
                Err(e) => {
                    warn!(error = %e, "Tavily web fallback failed");
                }
            }
        }

        if results.is_empty() {
            return Ok(Vec::new());
        }
        rank_and_dedupe_results(&mut results);

        #[cfg(feature = "web-scrape")]
        {
            // Integrated scraping for clean content (optional — pulls scraper/html2text).
            let mut final_hits = Vec::new();
            let urls_to_scrape = results
                .iter()
                .take(policy.searxng_max_urls_to_scrape)
                .cloned()
                .collect::<Vec<_>>();

            for res in urls_to_scrape {
                match crate::scraper::fetch_and_extract(&res.url, policy.scraper_timeout_ms).await {
                    Ok(doc) => {
                        if doc.text_density >= policy.scraper_min_text_density
                            || !policy.scraper_robots_txt_respect
                        {
                            let mut provenance = vec!["WebResearch".to_string()];
                            if let Some(ref eng) = res.engine {
                                provenance.push(format!("engine:{eng}"));
                            }
                            provenance.push("scraped:true".to_string());
                            final_hits.push(crate::memory_hybrid::HybridSearchHit {
                                path: doc.url,
                                title: doc.title.clone(),
                                content_snippet: doc.markdown.clone(),
                                score: res.score.unwrap_or(1.0),
                                provenance,
                                potential_contradiction: false,
                            });
                        }
                    }
                    Err(e) => {
                        warn!(url = %res.url, error = %e, "Scraping failed for search result");
                        let mut provenance = vec!["WebResearch".to_string()];
                        if let Some(ref eng) = res.engine {
                            provenance.push(format!("engine:{eng}"));
                        }
                        provenance.push("scraped:false".to_string());
                        final_hits.push(crate::memory_hybrid::HybridSearchHit {
                            path: res.url.clone(),
                            title: res.title.clone(),
                            content_snippet: res.content.clone(),
                            score: res.score.unwrap_or(0.5),
                            provenance,
                            potential_contradiction: false,
                        });
                    }
                }
            }

            Ok(final_hits)
        }

        #[cfg(not(feature = "web-scrape"))]
        {
            // Without `web-scrape`, return engine snippets only (no HTML fetch stack).
            let mut final_hits = Vec::new();
            for res in results.iter().take(policy.searxng_max_urls_to_scrape) {
                let mut provenance = vec!["WebResearch".to_string()];
                if let Some(ref eng) = res.engine {
                    provenance.push(format!("engine:{eng}"));
                }
                provenance.push("scraped:disabled".to_string());
                final_hits.push(crate::memory_hybrid::HybridSearchHit {
                    path: res.url.clone(),
                    title: res.title.clone(),
                    content_snippet: res.content.clone(),
                    score: res.score.unwrap_or(0.5),
                    provenance,
                    potential_contradiction: false,
                });
            }
            Ok(final_hits)
        }
    }
}

fn rank_and_dedupe_results(results: &mut Vec<crate::searxng::SearxngResult>) {
    let mut seen = HashSet::new();
    results.retain(|result| seen.insert(canonical_url_key(&result.url)));
    results.sort_by(|a, b| {
        let a_score = a.score.unwrap_or(0.5) * source_authority_score(&a.url);
        let b_score = b.score.unwrap_or(0.5) * source_authority_score(&b.url);
        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn canonical_url_key(url: &str) -> String {
    let mut key = url.trim().to_ascii_lowercase();
    if let Some(stripped) = key.strip_prefix("https://") {
        key = stripped.to_string();
    } else if let Some(stripped) = key.strip_prefix("http://") {
        key = stripped.to_string();
    }
    if let Some((base, _)) = key.split_once('#') {
        key = base.to_string();
    }
    if let Some((base, _)) = key.split_once('?') {
        key = base.to_string();
    }
    key.trim_end_matches('/').to_string()
}

fn source_authority_score(url: &str) -> f64 {
    let key = canonical_url_key(url);
    if key.contains(".gov/")
        || key.ends_with(".gov")
        || key.contains(".edu/")
        || key.ends_with(".edu")
    {
        1.25
    } else if key.contains("arxiv.org/")
        || key.contains("doi.org/")
        || key.contains("pubmed.ncbi.nlm.nih.gov/")
        || key.contains("docs.rs/")
        || key.contains("github.com/")
    {
        1.15
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(url: &str, score: f64) -> crate::searxng::SearxngResult {
        crate::searxng::SearxngResult {
            url: url.to_string(),
            title: url.to_string(),
            content: String::new(),
            engine: Some("test".to_string()),
            score: Some(score),
        }
    }

    #[test]
    fn rank_and_dedupe_prefers_authoritative_free_sources() {
        let mut results = vec![
            result("https://blog.example/post?utm=1", 0.9),
            result("https://blog.example/post", 0.8),
            result("https://docs.rs/vox-search/latest/vox_search/", 0.82),
        ];

        rank_and_dedupe_results(&mut results);

        assert_eq!(results.len(), 2);
        assert!(results[0].url.contains("docs.rs"));
    }
}
