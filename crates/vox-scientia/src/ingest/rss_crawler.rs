use crate::ingest::FeedSource;
use crate::ingest::InboundItem;
use anyhow::{Context, Result};
use feed_rs::parser;
use reqwest::{Client, header};
use std::time::Duration;

pub struct FeedCrawler {
    client: Client,
}

impl FeedCrawler {
    pub fn new() -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("VoxScientia/0.4.0 (Autonomous Research Crawler)"),
        );
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .default_headers(headers)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { client })
    }

    pub async fn crawl_all(&self, sources: &[FeedSource]) -> Vec<InboundItem> {
        let mut items = Vec::new();
        for source in sources {
            match self.crawl_url(&source.url, &source.source_kind).await {
                Ok(mut source_items) => {
                    items.append(&mut source_items);
                }
                Err(e) => {
                    tracing::warn!("Failed to crawl source {}: {}", source.url, e);
                }
            }
        }
        items
    }

    pub async fn crawl_url(&self, url: &str, kind: &str) -> Result<Vec<InboundItem>> {
        let resp = self.client.get(url).send().await?;
        let bytes = resp.bytes().await?;
        let feed = parser::parse(&bytes[..])?;

        let mut items = Vec::new();
        for entry in feed.entries {
            let title = entry
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "Untitled".to_string());
            let abstract_text = entry.summary.map(|s| s.content);
            let link = entry
                .links
                .into_iter()
                .next()
                .map(|l| l.href)
                .unwrap_or_default();

            if !link.is_empty() {
                items.push(InboundItem {
                    source_url: link,
                    source_kind: kind.to_string(),
                    title,
                    abstract_text,
                });
            }
        }

        Ok(items)
    }
}
