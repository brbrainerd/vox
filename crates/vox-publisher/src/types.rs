use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::BTreeMap;

use crate::contract::validate_github_repo;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedNewsItem {
    pub id: String,
    pub title: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub content_markdown: String,
    #[serde(default)]
    pub syndication: SyndicationConfig,
    /// Optional id of a row in `contracts/scientia/distribution.topic-packs.yaml` (news frontmatter / DB metadata).
    #[serde(default)]
    pub topic_pack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyndicationConfig {
    // ── Enable/Disable ──────────────────────────────────────────────
    #[serde(default = "default_rss")]
    pub rss: bool,
    #[serde(default)]
    pub hacker_news: bool,
    #[serde(default)]
    pub researchgate: bool,
    #[serde(default)]
    pub linkedin: bool,

    // ── Social status/payload (Converged SSOT) ──────────────────────
    #[serde(default)]
    pub twitter: serde_json::Value,
    #[serde(default)]
    pub bluesky: serde_json::Value,
    #[serde(default)]
    pub mastodon: serde_json::Value,
    #[serde(default)]
    pub discord: serde_json::Value,

    // ── Social broadcast (Legacy list) ──────────────────────────────
    #[serde(default)]
    pub social: Vec<SocialChannel>,

    #[serde(default)]
    pub short_summary: Option<String>,

    // ── Platform targets ────────────────────────────────────────────
    #[serde(default)]
    pub reddit: Option<RedditConfig>,
    #[serde(default)]
    pub youtube: Option<YouTubeConfig>,
    #[serde(default)]
    pub open_collective: Option<OpenCollectiveConfig>,
    #[serde(default)]
    pub forge: Option<ForgeConfig>,
    #[serde(default)]
    pub crates_io: Option<CratesIoConfig>,

    // ── Scholarly ────────────────────────────────────────────────────
    #[serde(default)]
    pub scholarly: Vec<String>,

    // ── Pre-existing Policy / Legacy Compat ──────────────────────────
    #[serde(default)]
    pub distribution_policy: DistributionPolicyConfig,
    #[serde(default)]
    pub dry_run: bool,
}

impl SyndicationConfig {
    pub fn is_active(&self, channel: SocialChannel) -> bool {
        let val = match channel {
            SocialChannel::Twitter => &self.twitter,
            SocialChannel::Bluesky => &self.bluesky,
            SocialChannel::Mastodon => &self.mastodon,
            SocialChannel::Discord => &self.discord,
        };

        if val.is_boolean() {
            return val.as_bool().unwrap_or(false);
        }
        if val.is_object() {
            return true;
        }

        // Fallback to the social vec if the root key is null/missing
        self.social.contains(&channel)
    }

    pub fn twitter_override(&self) -> Option<TwitterOverride> {
        serde_json::from_value(self.twitter.clone()).ok()
    }

    pub fn bluesky_override(&self) -> Option<BlueskyOverride> {
        serde_json::from_value(self.bluesky.clone()).ok()
    }

    pub fn mastodon_override(&self) -> Option<MastodonOverride> {
        serde_json::from_value(self.mastodon.clone()).ok()
    }

    pub fn discord_override(&self) -> Option<DiscordOverride> {
        serde_json::from_value(self.discord.clone()).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocialChannel {
    Twitter,
    Bluesky,
    Mastodon,
    Discord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BlueskyOverride {
    // Reserved for future use (facets, custom lexicon fields, etc.)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiscordOverride {
    pub message: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MastodonOverride {
    pub visibility: Option<String>,
    pub language: Option<String>,
    pub spoiler_text: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TwitterOverride {
    pub thread: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DistributionPolicyConfig {
    /// Per-channel routing policy keyed by channel name (`rss`, `twitter`, ...).
    #[serde(default)]
    pub channel_policy: BTreeMap<String, ChannelPolicyConfig>,
    /// Optional retry profile label (contract/documentation level, runtime-specific handling).
    #[serde(default)]
    pub retry_profile: Option<String>,
    /// Optional rate-limit profile label.
    #[serde(default)]
    pub rate_limit_profile: Option<String>,
    /// Optional explicit policy gate for required approvals.
    #[serde(default)]
    pub approval_required: Option<bool>,
    /// Optional per-item dry-run override (merged with existing dry_run booleans).
    #[serde(default)]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelPolicyConfig {
    /// Explicit enable/disable override for this channel.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Optional topic filters for selective routing.
    #[serde(default)]
    pub topic_filters: Option<TopicFiltersConfig>,
    /// Optional channel-specific publishability floor in `[0,1]`.
    #[serde(default)]
    pub worthiness_floor: Option<f64>,
    /// Optional template profile key consumed by template derivation.
    #[serde(default)]
    pub template_profile: Option<String>,
    /// Optional override for the auto-derived text for this specific platform.
    #[serde(default)]
    pub text_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopicFiltersConfig {
    #[serde(default)]
    pub include_tags: Vec<String>,
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub min_topic_score: Option<f64>,
}

fn default_rss() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum ForgePostType {
    #[default]
    Release,
    Discussion,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForgeConfig {
    pub repo: String,
    pub post_type: ForgePostType,
    #[serde(default)]
    pub release_tag: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub discussion_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenCollectiveConfig {
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub scheduled_publish_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RedditPostKind {
    #[default]
    Link,
    SelfPost,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RedditConfig {
    #[serde(default)]
    pub subreddit: Option<String>,
    #[serde(default)]
    pub subreddits: Vec<RedditTarget>,
    #[serde(default = "default_reddit_kind")]
    pub kind: RedditPostKind,
    #[serde(default)]
    pub title_override: Option<String>,
    #[serde(default)]
    pub text_override: Option<String>,
    #[serde(default)]
    pub url_override: Option<String>,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub send_replies: bool,
}

impl RedditConfig {
    pub fn get_targets(&self) -> Vec<RedditTarget> {
        let mut t = self.subreddits.clone();
        if let Some(ref s) = self.subreddit {
            let legacy = RedditTarget {
                name: s.clone(),
                kind: self.kind,
                title_override: self.title_override.clone(),
                text_override: self.text_override.clone(),
                url_override: self.url_override.clone(),
                nsfw: self.nsfw,
                spoiler: self.spoiler,
                send_replies: self.send_replies,
            };
            if !t.iter().any(|x| x.name == legacy.name) {
                t.push(legacy);
            }
        }
        t
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RedditTarget {
    pub name: String,
    #[serde(default = "default_reddit_kind")]
    pub kind: RedditPostKind,
    #[serde(default)]
    pub title_override: Option<String>,
    #[serde(default)]
    pub text_override: Option<String>,
    #[serde(default)]
    pub url_override: Option<String>,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub send_replies: bool,
}

fn default_reddit_kind() -> RedditPostKind {
    RedditPostKind::Link
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HackerNewsMode {
    #[default]
    ManualAssist,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HackerNewsConfig {
    #[serde(default = "default_hn_mode")]
    pub mode: HackerNewsMode,
    #[serde(default)]
    pub title_override: Option<String>,
    #[serde(default)]
    pub url_override: Option<String>,
    /// First-comment text to display in the manual-assist output.
    #[serde(default)]
    pub comment_draft: Option<String>,
}

fn default_hn_mode() -> HackerNewsMode {
    HackerNewsMode::ManualAssist
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum YouTubePrivacyStatus {
    #[default]
    Private,
    Unlisted,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YouTubeConfig {
    /// Repo-relative or absolute path to a local video payload for upload.
    pub video_asset_ref: String,
    #[serde(default)]
    pub title_override: Option<String>,
    #[serde(default)]
    pub description_override: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default = "default_youtube_privacy")]
    pub privacy_status: YouTubePrivacyStatus,
    #[serde(default)]
    pub notify_subscribers: bool,
}

fn default_youtube_privacy() -> YouTubePrivacyStatus {
    YouTubePrivacyStatus::Private
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResearchGateConfig {
    /// Optional DOI of the publication to signal matching.
    pub doi: Option<String>,
    /// Whether user confirmed manual matching at researchgate.net.
    #[serde(default)]
    pub manual_confirmation: bool,
}

fn normalize_channel_key(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn merge_channel_policy(base: &mut ChannelPolicyConfig, incoming: ChannelPolicyConfig) {
    if incoming.enabled.is_some() {
        base.enabled = incoming.enabled;
    }
    if incoming.topic_filters.is_some() {
        base.topic_filters = incoming.topic_filters;
    }
    if let Some(floor) = incoming.worthiness_floor {
        base.worthiness_floor = Some(match base.worthiness_floor {
            Some(existing) => existing.max(floor),
            None => floor,
        });
    }
    if incoming.template_profile.is_some() {
        base.template_profile = incoming.template_profile;
    }
}

impl SyndicationConfig {
    /// Canonicalize `distribution_policy.channel_policy` keys to lowercase channel ids.
    pub fn normalize_channel_policy_keys(&mut self) {
        if self.distribution_policy.channel_policy.is_empty() {
            return;
        }
        let mut normalized = BTreeMap::new();
        for (raw, policy) in std::mem::take(&mut self.distribution_policy.channel_policy) {
            let key = normalize_channel_key(raw.as_str());
            if key.is_empty() {
                continue;
            }
            match normalized.get_mut(&key) {
                Some(existing) => merge_channel_policy(existing, policy),
                None => {
                    normalized.insert(key, policy);
                }
            }
        }
        self.distribution_policy.channel_policy = normalized;
    }
}

impl UnifiedNewsItem {
    pub fn parse(content: &str, id: &str) -> anyhow::Result<Self> {
        crate::contract::validate_news_id(id)?;

        if !content.starts_with("---") {
            return Err(anyhow::anyhow!("Missing YAML frontmatter"));
        }

        let mut parts = content.splitn(3, "---");
        parts.next();
        let yaml = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("Incomplete frontmatter"))?;
        let markdown = parts.next().unwrap_or_default().trim();

        #[derive(Deserialize)]
        struct Frontmatter {
            title: String,
            author: String,
            #[serde(default)]
            published_at: Option<String>,
            #[serde(default)]
            tags: Vec<String>,
            #[serde(default)]
            syndication: SyndicationConfig,
            #[serde(default)]
            topic_pack: Option<String>,
        }

        let front: Frontmatter = serde_yaml::from_str(yaml)?;

        let published_at = if let Some(ref s) = front.published_at {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|_| {
                    s.parse::<DateTime<Utc>>()
                        .map_err(|e| anyhow::anyhow!("{}", e))
                })?
        } else {
            Utc::now()
        };

        if let Some(ref gh) = front.syndication.forge {
            validate_github_repo(&gh.repo)?;
        }

        let topic_pack = front
            .topic_pack
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);

        let mut item = Self {
            id: id.to_string(),
            title: front.title,
            author: front.author,
            published_at,
            tags: front.tags,
            content_markdown: markdown.to_string(),
            syndication: front.syndication,
            topic_pack,
        };
        item.hydrate_topic_pack_if_set()?;
        item.syndication.normalize_channel_policy_keys();
        Ok(item)
    }

    /// Merge [`crate::topic_packs`] policy into [`Self::syndication`] when [`Self::topic_pack`] is set.
    pub fn hydrate_topic_pack_if_set(&mut self) -> anyhow::Result<()> {
        if let Some(ref p) = self.topic_pack {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                crate::topic_packs::hydrate_syndication_from_pack_id(
                    &mut self.syndication,
                    trimmed,
                )?;
            }
        }
        self.syndication.normalize_channel_policy_keys();
        Ok(())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        crate::contract::validate_news_id(&self.id)?;
        if let Some(ref gh) = self.syndication.forge {
            validate_github_repo(&gh.repo)?;
            match gh.post_type {
                ForgePostType::Release => {
                    let tag = gh
                        .release_tag
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or(self.id.as_str());
                    if tag.contains('/') {
                        anyhow::bail!("release_tag must not contain slashes: {:?}", tag);
                    }
                }
                ForgePostType::Discussion => {
                    let cat = gh
                        .discussion_category
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("");
                    if cat.is_empty() {
                        anyhow::bail!(
                            "github.discussion_category is required when post_type is Discussion"
                        );
                    }
                }
            }
        }

        if let Some(ref yt) = self.syndication.youtube
            && yt.video_asset_ref.trim().is_empty()
        {
            anyhow::bail!("youtube.video_asset_ref must not be empty");
        }
        for (channel, policy) in &self.syndication.distribution_policy.channel_policy {
            if channel.trim().is_empty() {
                anyhow::bail!("distribution_policy.channel_policy keys must not be empty");
            }
            if let Some(floor) = policy.worthiness_floor
                && !(0.0..=1.0).contains(&floor)
            {
                anyhow::bail!(
                    "distribution_policy.channel_policy[{channel}].worthiness_floor must be within [0,1]"
                );
            }
            if let Some(filters) = policy.topic_filters.as_ref()
                && let Some(min) = filters.min_topic_score
                && !(0.0..=1.0).contains(&min)
            {
                anyhow::bail!(
                    "distribution_policy.channel_policy[{channel}].topic_filters.min_topic_score must be within [0,1]"
                );
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn content_sha3_256(&self) -> String {
        let mut canonical = serde_json::json!({
            "id": self.id,
            "title": self.title,
            "author": self.author,
            "published_at": self.published_at.to_rfc3339(),
            "tags": self.tags,
            "content_markdown": self.content_markdown,
            "syndication": self.syndication,
        });
        if let Some(ref p) = self.topic_pack {
            canonical["topic_pack"] = serde_json::Value::String(p.clone());
        }
        let mut hasher = Sha3_256::new();
        hasher.update(canonical.to_string().as_bytes());
        let digest = hasher.finalize();
        format!("{digest:x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn item_with_policy_floor(floor: f64) -> UnifiedNewsItem {
        let mut channel_policy = BTreeMap::new();
        channel_policy.insert(
            "twitter".to_string(),
            ChannelPolicyConfig {
                worthiness_floor: Some(floor),
                ..Default::default()
            },
        );
        UnifiedNewsItem {
            id: "policy-validate".to_string(),
            title: "Title".to_string(),
            author: "Author".to_string(),
            published_at: Utc::now(),
            tags: vec![],
            content_markdown: "Body".to_string(),
            syndication: SyndicationConfig {
                distribution_policy: DistributionPolicyConfig {
                    channel_policy,
                    ..Default::default()
                },
                ..Default::default()
            },
            topic_pack: None,
        }
    }

    #[test]
    fn validate_rejects_invalid_worthiness_floor() {
        let item = item_with_policy_floor(1.2);
        let err = item.validate().expect_err("invalid floor should fail");
        assert!(err.to_string().contains("worthiness_floor"));
    }

    #[test]
    fn parse_applies_topic_pack_channel_allowlist() {
        let md = r#"---
title: T
author: A
topic_pack: research_breakthrough
syndication:
  rss: true
  twitter:
    short_text: null
    thread: false
---
body"#;
        let item = UnifiedNewsItem::parse(md, "nid").expect("parse");
        assert_eq!(item.topic_pack.as_deref(), Some("research_breakthrough"));
        assert!(!item.syndication.social.contains(&SocialChannel::Twitter));
        assert!(item.syndication.rss);
    }

    #[test]
    fn parse_normalizes_channel_policy_keys_to_lowercase() {
        let md = r#"---
title: T
author: A
syndication:
  distribution_policy:
    channel_policy:
      Twitter:
        enabled: false
---
body"#;
        let item = UnifiedNewsItem::parse(md, "nid").expect("parse");
        assert!(
            item.syndication
                .distribution_policy
                .channel_policy
                .contains_key("twitter")
        );
        assert!(
            !item
                .syndication
                .distribution_policy
                .channel_policy
                .contains_key("Twitter")
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CratesIoConfig {
    pub crates_to_update: Vec<String>,
}
