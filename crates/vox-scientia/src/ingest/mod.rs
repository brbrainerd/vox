//! Scientia ingest: RSS crawling + dedup for the scholarly-external-jobs pipeline.

use serde::{Deserialize, Serialize};

pub mod deduplicator;
pub mod rss_crawler;

pub use deduplicator::IngestDeduplicator;
pub use rss_crawler::FeedCrawler;

/// A registry feed source endpoint (e.g. RSS URL) for inbound research scoping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSource {
    pub id: String,
    pub url: String,
    pub source_kind: String,
    pub crawl_interval_ms: i64,
}

/// A raw scraped/received item before semantic evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundItem {
    pub source_url: String,
    pub source_kind: String,
    pub title: String,
    pub abstract_text: Option<String>,
}
