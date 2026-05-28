use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearxngResult {
    pub url: String,
    pub title: String,
    pub content: String,
    pub engine: Option<String>,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearxngResponse {
    pub results: Vec<SearxngResult>,
}

pub struct SearxngSearchClient {
    pub base_url: String,
}

impl SearxngSearchClient {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        engines_csv: &str,
        language: &str,
    ) -> anyhow::Result<Vec<SearxngResult>> {
        let client = vox_http_client::client();
        let url = format!(
            "{}/search?q={}&format=json&engines={}&language={}",
            self.base_url.trim_end_matches('/'),
            urlencoding::encode(query),
            urlencoding::encode(engines_csv),
            urlencoding::encode(language),
        );

        debug!(
            url = %url,
            query = query,
            engines = engines_csv,
            language = language,
            "Firing SearXNG search"
        );

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "SearXNG search failed with status: {}",
                resp.status()
            ));
        }

        let body: SearxngResponse = resp.json().await?;
        let mut results = body.results;
        results.truncate(limit);

        Ok(results)
    }
}
