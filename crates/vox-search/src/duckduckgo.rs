use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdgResult {
    #[serde(rename = "FirstURL")]
    pub url: String,
    #[serde(rename = "Text")]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DdgResponse {
    #[serde(rename = "RelatedTopics")]
    pub related_topics: Vec<DdgResult>,
}

pub struct DuckDuckGoClient;

impl DuckDuckGoClient {
    pub async fn search(
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::searxng::SearxngResult>> {
        let client = vox_http_client::client();
        // DuckDuckGo Instant Answer API is limited but free and no auth.
        // For actual web search, they have a different endpoint but it's often scraper-blocked.
        // We'll use the RelatedTopics as a factual fallback.
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json",
            urlencoding::encode(query)
        );

        debug!(url = %url, query = query, "Firing DuckDuckGo fallback search");

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "DuckDuckGo search failed with status: {}",
                resp.status()
            ));
        }

        let body: DdgResponse = resp.json().await?;
        let results = body
            .related_topics
            .into_iter()
            .take(limit)
            .map(|r| crate::searxng::SearxngResult {
                url: r.url,
                title: r.text.clone(),
                content: r.text,
                engine: Some("duckduckgo".to_string()),
                score: None,
            })
            .collect();

        Ok(results)
    }
}
