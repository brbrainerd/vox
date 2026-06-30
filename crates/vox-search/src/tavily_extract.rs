//! Tavily `/extract` uplift when search snippets are too thin for grounding.

use tracing::{info, warn};

use vox_secrets::{SecretId, resolve_secret};

#[cfg(feature = "tavily")]
use tavily::Tavily;

/// Heuristic: snippet is too short or mostly non-alphanumeric noise for reliable grounding.
#[must_use]
pub fn snippet_quality_low(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.chars().count() < 80 {
        return true;
    }
    let alnum_or_space = trimmed
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .count();
    let total = trimmed.chars().count().max(1);
    alnum_or_space * 2 < total
}

#[derive(Debug, Clone)]
pub struct ExtractHit {
    pub url: String,
    pub content: String,
}

#[cfg(feature = "tavily")]
pub struct TavilyExtractClient {
    inner: Tavily,
}

#[cfg(feature = "tavily")]
impl TavilyExtractClient {
    pub fn from_env() -> Option<Self> {
        Self::with_optional_base_url(None)
    }

    pub fn with_base_url(api_key: &str, base_url: &str) -> Option<Self> {
        let client = Tavily::builder(api_key)
            .base_url(base_url)
            .timeout(vox_config::timeouts::D_30S)
            .build()
            .ok()?;
        Some(Self { inner: client })
    }

    fn with_optional_base_url(base_url: Option<&str>) -> Option<Self> {
        let binding = resolve_secret(SecretId::TavilyApiKey);
        let key_str = binding.expose()?;
        let mut builder = Tavily::builder(key_str).timeout(vox_config::timeouts::D_30S);
        if let Some(url) = base_url {
            builder = builder.base_url(url);
        }
        let client = builder.build().ok()?;
        Some(Self { inner: client })
    }

    pub async fn extract_urls(
        &self,
        urls: &[String],
        query: Option<&str>,
    ) -> Result<Vec<ExtractHit>, String> {
        if urls.is_empty() {
            return Ok(Vec::new());
        }
        tracing::debug!(
            query = query.unwrap_or(""),
            url_count = urls.len(),
            "tavily_extract_request"
        );
        let resp = self
            .inner
            .extract(urls.iter().map(String::as_str))
            .await
            .map_err(|e| format!("tavily_extract_failed:{e}"))?;
        Ok(resp
            .results
            .into_iter()
            .map(|r| ExtractHit {
                url: r.url,
                content: r.raw_content,
            })
            .collect())
    }
}

/// Replace thin `content` fields on search rows via Tavily extract (fail-open).
#[cfg(feature = "tavily")]
pub async fn uplift_low_quality_snippets(
    results: &mut [crate::searxng::SearxngResult],
    query: &str,
    max_urls: usize,
) {
    let Some(client) = TavilyExtractClient::from_env() else {
        return;
    };
    let urls: Vec<String> = results
        .iter()
        .filter(|r| snippet_quality_low(&r.content))
        .take(max_urls.max(1))
        .map(|r| r.url.clone())
        .collect();
    if urls.is_empty() {
        return;
    }
    match client.extract_urls(&urls, Some(query)).await {
        Ok(extracted) => {
            info!(count = extracted.len(), "tavily extract uplift succeeded");
            for hit in extracted {
                if let Some(row) = results.iter_mut().find(|r| r.url == hit.url)
                    && !hit.content.trim().is_empty() {
                        row.content = hit.content;
                        row.engine = Some(
                            row.engine
                                .clone()
                                .map(|e| format!("{e}+tavily_extract"))
                                .unwrap_or_else(|| "tavily_extract".to_string()),
                        );
                    }
            }
        }
        Err(e) => warn!(error = %e, "tavily extract uplift failed (fail-open)"),
    }
}

#[cfg(not(feature = "tavily"))]
pub async fn uplift_low_quality_snippets(
    _results: &mut [crate::searxng::SearxngResult],
    _query: &str,
    _max_urls: usize,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_quality_low_flags_empty_and_short() {
        assert!(snippet_quality_low(""));
        assert!(snippet_quality_low("short"));
        assert!(!snippet_quality_low(
            "This is a sufficiently long snippet with enough alphanumeric content to pass the quality gate for grounding."
        ));
    }

    #[test]
    fn snippet_quality_low_flags_markup_heavy() {
        assert!(snippet_quality_low(&"<>[]{}".repeat(30)));
    }
}
