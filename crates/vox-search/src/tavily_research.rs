//! Tavily `/research` deep-research tier (optional; gated by `VOX_TAVILY_RESEARCH`).

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use vox_secrets::{SecretId, resolve_secret};

const DEFAULT_RESEARCH_BASE: &str = "https://api.tavily.com";

#[derive(Debug, Clone, Serialize)]
struct ResearchRequest {
    api_key: String,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ResearchSource {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ResearchResponse {
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    sources: Vec<ResearchSource>,
    #[serde(default)]
    response_time: Option<f64>,
}

/// Returns true when `VOX_TAVILY_RESEARCH` is set to a truthy value.
#[must_use]
pub fn tavily_research_enabled() -> bool {
    match resolve_secret(SecretId::VoxTavilyResearch).expose() {
        Some(v) => {
            let v = v.trim();
            matches!(v, "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
        }
        None => false,
    }
}

pub struct TavilyResearchClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl TavilyResearchClient {
    pub fn from_env() -> Option<Self> {
        if !tavily_research_enabled() {
            return None;
        }
        let api_key = resolve_secret(SecretId::TavilyApiKey).expose()?.to_string();
        Some(Self {
            http: reqwest::Client::builder()
                .timeout(vox_config::timeouts::D_30S)
                .build()
                .ok()?,
            api_key,
            base_url: DEFAULT_RESEARCH_BASE.to_string(),
        })
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(vox_config::timeouts::D_30S)
                .build()
                .expect("reqwest client"),
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }

    pub async fn research(
        &self,
        query: &str,
        instructions: Option<&str>,
    ) -> Result<Vec<crate::searxng::SearxngResult>, String> {
        let body = ResearchRequest {
            api_key: self.api_key.clone(),
            query: query.to_string(),
            instructions: instructions.map(str::to_string),
        };
        let url = format!("{}/research", self.base_url.trim_end_matches('/'));
        tracing::debug!(query, "tavily_research_request");
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("tavily_research_http:{e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("tavily_research_body:{e}"))?;
        if !status.is_success() {
            return Err(format!("tavily_research_status:{status}:{text}"));
        }
        let parsed: ResearchResponse =
            serde_json::from_str(&text).map_err(|e| format!("tavily_research_parse:{e}"))?;
        info!(
            source_count = parsed.sources.len(),
            response_time = ?parsed.response_time,
            "tavily research succeeded"
        );
        let mut out: Vec<crate::searxng::SearxngResult> = parsed
            .sources
            .into_iter()
            .map(|s| crate::searxng::SearxngResult {
                url: s.url,
                title: s.title,
                content: if s.content.is_empty() {
                    parsed.answer.clone().unwrap_or_default()
                } else {
                    s.content
                },
                engine: Some("tavily_research".to_string()),
                score: s.score.map(f64::from),
            })
            .collect();
        if out.is_empty()
            && let Some(answer) = parsed.answer.filter(|a| !a.trim().is_empty())
        {
            out.push(crate::searxng::SearxngResult {
                url: format!("tavily-research://{query}"),
                title: format!("Tavily research: {query}"),
                content: answer,
                engine: Some("tavily_research".to_string()),
                score: Some(0.85),
            });
        }
        Ok(out)
    }
}

/// Optional research-tier fetch; returns empty vec on disable or error (fail-open).
pub async fn try_tavily_research_hits(query: &str) -> Vec<crate::searxng::SearxngResult> {
    let Some(client) = TavilyResearchClient::from_env() else {
        return Vec::new();
    };
    match client.research(query, None).await {
        Ok(hits) => hits,
        Err(e) => {
            warn!(error = %e, "tavily research tier failed (fail-open)");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_gate_is_boolean() {
        let _ = tavily_research_enabled();
    }
}
