use crate::PublisherConfig;
use crate::contract::NewsSiteConfig;
use crate::types::{DistributionPolicyConfig, SyndicationConfig, UnifiedNewsItem};

#[cfg(test)]
pub fn item_fixture() -> UnifiedNewsItem {
    UnifiedNewsItem {
        id: "p1".to_string(),
        title: "Test Title".to_string(),
        author: "Test Author".to_string(),
        published_at: chrono::Utc::now(),
        tags: vec![],
        content_markdown: "Test Content".to_string(),
        syndication: SyndicationConfig {
            rss: false,
            dry_run: false,
            distribution_policy: DistributionPolicyConfig::default(),
            ..Default::default()
        },
        topic_pack: None,
    }
}

#[cfg(test)]
pub fn config_fixture(base_url: Option<String>) -> PublisherConfig {
    PublisherConfig {
        dry_run: false,
        site: NewsSiteConfig::default(),
        twitter_api_base: base_url.clone(),
        linkedin_api_base: base_url.clone(),
        reddit_api_base: base_url,
        ..PublisherConfig::default()
    }
}

#[cfg(test)]
mod bluesky;
#[cfg(test)]
mod discord;
#[cfg(test)]
mod linkedin;
#[cfg(test)]
mod mastodon;
#[cfg(test)]
mod opencollective;
#[cfg(all(test, feature = "scientia-reddit"))]
mod reddit;
#[cfg(test)]
mod twitter;
