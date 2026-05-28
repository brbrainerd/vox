pub use crate::tavily_budget::TavilySessionBudget;

use vox_secrets::{SecretId, resolve_secret};

#[cfg(feature = "tavily")]
use tavily::{SearchRequest, Tavily};

#[cfg(feature = "tavily")]
pub struct TavilySearchClient {
    inner: Tavily,
}

#[cfg(feature = "tavily")]
impl TavilySearchClient {
    pub fn from_env() -> Option<Self> {
        let binding = resolve_secret(SecretId::TavilyApiKey);
        let key_str = binding.expose()?;
        let client = Tavily::builder(key_str)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .ok()?;
        Some(Self { inner: client })
    }

    pub async fn search(
        &self,
        query: &str,
        max_results: usize,
        depth: &str,
    ) -> Result<Vec<TavilyHit>, String> {
        let req = SearchRequest::new(
            // key is internally stored in client
            "", query,
        )
        .search_depth(depth)
        .max_results(max_results as i32);

        let resp = self
            .inner
            .call(&req)
            .await
            .map_err(|e| format!("tavily_search_failed:{e}"))?;

        Ok(resp
            .results
            .into_iter()
            .map(|r| TavilyHit {
                url: r.url,
                title: r.title,
                content: r.content,
                score: r.score,
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct TavilyHit {
    pub url: String,
    pub title: String,
    pub content: String,
    pub score: f32,
}
