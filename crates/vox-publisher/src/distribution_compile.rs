//! Compile a [`crate::types::UnifiedNewsItem`] into a **distribution plan**: per-channel caps,
//! resolved projection profile ids, and a **derivation digest** over canonical inputs (see plan B6).
//!
//! SSOT limits: [`contracts/scientia/projection-profiles.v1.yaml`](../../../contracts/scientia/projection-profiles.v1.yaml).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use tracing::debug;

use crate::types::{SyndicationConfig, UnifiedNewsItem};

const EMBEDDED_PROJECTION_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/scientia/projection-profiles.v1.yaml"
));

/// Repo-relative path for docs and CI messages.
pub const PROJECTION_PROFILES_YAML_REPO_PATH: &str =
    "contracts/scientia/projection-profiles.v1.yaml";

#[derive(Debug, Deserialize)]
struct ProjectionProfilesFile {
    version: u32,
    profiles: BTreeMap<String, ProjectionProfileYaml>,
}

#[derive(Debug, Deserialize, Clone)]
struct ProjectionProfileYaml {
    #[allow(dead_code)]
    description: String,
    max_title_chars: usize,
    max_primary_chars: usize,
}

static PROFILES: OnceLock<ProjectionProfilesFile> = OnceLock::new();

fn embedded_profiles() -> &'static ProjectionProfilesFile {
    PROFILES.get_or_init(|| {
        let file: ProjectionProfilesFile = serde_yaml::from_str(EMBEDDED_PROJECTION_YAML)
            .expect("embedded projection-profiles.v1.yaml must parse");
        debug!(
            schema_version = file.version,
            path = PROJECTION_PROFILES_YAML_REPO_PATH,
            profile_count = file.profiles.len(),
            "loaded embedded projection profiles"
        );
        file
    })
}

/// One enabled channel with resolved caps (after projection profile lookup).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChannelPlan {
    pub channel: String,
    pub template_profile: Option<String>,
    pub projection_profile_id: String,
    pub max_title_chars: usize,
    pub max_primary_chars: usize,
}

/// Output of [`compile_for_publish`]: digest, optional warnings, and per-channel plans.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DistributionCompileReport {
    pub derivation_digest_hex: String,
    pub warnings: Vec<String>,
    pub channel_plans: Vec<ChannelPlan>,
}

#[derive(Serialize)]
struct DerivationPayload<'a> {
    id: &'a str,
    title: &'a str,
    published_at: chrono::DateTime<chrono::Utc>,
    tags: &'a [String],
    syndication: &'a SyndicationConfig,
    content_sha3_256: String,
}

/// Build a stable digest and channel plan from the news item as syndicated today.
#[must_use]
pub fn compile_for_publish(item: &UnifiedNewsItem) -> DistributionCompileReport {
    let profiles = embedded_profiles();
    let mut warnings = Vec::new();
    let mut channel_plans = Vec::new();

    let payload = DerivationPayload {
        id: item.id.as_str(),
        title: item.title.as_str(),
        published_at: item.published_at,
        tags: item.tags.as_slice(),
        syndication: &item.syndication,
        content_sha3_256: item.content_sha3_256(),
    };
    let derivation_digest_hex = derivation_hex(&payload);

    if item.syndication.rss {
        push_channel(
            "rss",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item
        .syndication
        .is_active(crate::types::SocialChannel::Twitter)
    {
        push_channel(
            "twitter",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.forge.is_some() {
        push_channel(
            "github",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.open_collective.is_some() {
        push_channel(
            "open_collective",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.reddit.is_some() {
        push_channel(
            "reddit",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.hacker_news {
        push_channel(
            "hacker_news",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.youtube.is_some() {
        push_channel(
            "youtube",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.crates_io.is_some() {
        push_channel(
            "crates_io",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item
        .syndication
        .is_active(crate::types::SocialChannel::Discord)
    {
        push_channel(
            "discord",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item
        .syndication
        .is_active(crate::types::SocialChannel::Mastodon)
    {
        push_channel(
            "mastodon",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item.syndication.linkedin {
        push_channel(
            "linkedin",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }
    if item
        .syndication
        .is_active(crate::types::SocialChannel::Bluesky)
    {
        push_channel(
            "bluesky",
            &item.syndication,
            profiles,
            &mut warnings,
            &mut channel_plans,
        );
    }

    DistributionCompileReport {
        derivation_digest_hex,
        warnings,
        channel_plans,
    }
}

fn derivation_hex(payload: &DerivationPayload<'_>) -> String {
    let json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    let mut h = Sha3_256::new();
    h.update(json.as_bytes());
    format!("{:x}", h.finalize())
}

fn push_channel(
    channel: &str,
    syn: &SyndicationConfig,
    profiles: &ProjectionProfilesFile,
    warnings: &mut Vec<String>,
    out: &mut Vec<ChannelPlan>,
) {
    let key = channel.to_string();
    let template_profile = syn
        .distribution_policy
        .channel_policy
        .get(channel)
        .and_then(|p| p.template_profile.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let profile_id = template_profile
        .clone()
        .unwrap_or_else(|| "fallback".to_string());

    let (resolved_id, max_title, max_primary) = resolve_caps(
        channel,
        profile_id.as_str(),
        template_profile.as_deref(),
        profiles,
        warnings,
    );

    out.push(ChannelPlan {
        channel: key,
        template_profile,
        projection_profile_id: resolved_id,
        max_title_chars: max_title,
        max_primary_chars: max_primary,
    });
}

fn resolve_caps(
    channel: &str,
    profile_id: &str,
    template_profile: Option<&str>,
    profiles: &ProjectionProfilesFile,
    warnings: &mut Vec<String>,
) -> (String, usize, usize) {
    let (fallback_title, fallback_primary) = channel_contract_defaults(channel);
    let Some(yml) = profiles.profiles.get(profile_id) else {
        if profile_id != "fallback" {
            warnings.push(format!(
                "unknown projection profile {profile_id:?} for channel {channel}; using fallback"
            ));
        }
        let fb = profiles
            .profiles
            .get("fallback")
            .expect("projection profiles must define fallback");
        return (
            "fallback".to_string(),
            fb.max_title_chars.min(fallback_title.max(1)),
            fb.max_primary_chars.min(fallback_primary.max(1)),
        );
    };

    let max_title = yml.max_title_chars.min(fallback_title.max(1));
    let max_primary = yml.max_primary_chars.min(fallback_primary.max(1));

    if template_profile.is_some() && profile_id == "fallback" {
        warnings.push(format!(
            "channel {channel} missing template_profile; using fallback caps"
        ));
    }

    (profile_id.to_string(), max_title, max_primary)
}

fn channel_contract_defaults(channel: &str) -> (usize, usize) {
    match channel {
        "twitter" => (
            crate::adapters::twitter::TWEET_MAX_CHARS,
            crate::adapters::twitter::TWEET_MAX_CHARS,
        ),
        #[cfg(feature = "scientia-reddit")]
        "reddit" => (
            crate::adapters::reddit::TITLE_MAX,
            crate::adapters::reddit::SELFPOST_SUMMARY_MAX,
        ),
        "hacker_news" => (crate::adapters::hacker_news::TITLE_MAX, 500),
        #[cfg(feature = "scientia-youtube")]
        "youtube" => (
            crate::adapters::youtube::TITLE_MAX,
            crate::adapters::youtube::DESCRIPTION_MAX,
        ),
        "github" => (500, 50_000),
        "open_collective" => (200, 10_000),
        "rss" => (500, 100_000),
        "crates_io" => (200, 8000),
        "discord" => (256, 2000),
        "mastodon" => (500, 500),
        "linkedin" => (200, 3000),
        "bluesky" => (300, 300),
        _ => (300, 2000),
    }
}

/// Validate every `template_profile` value used in embedded topic packs exists in projection profiles.
pub fn validate_topic_pack_projection_profiles() -> Result<(), String> {
    let packs = crate::topic_packs::load_topic_packs_embedded().map_err(|e| e.to_string())?;
    let profiles = embedded_profiles();
    if profiles.version != 1 {
        return Err(format!(
            "expected projection profiles version 1, got {}",
            profiles.version
        ));
    }
    let mut seen = BTreeMap::new();
    for (pack_id, pack) in &packs.packs {
        for (ch, prof) in &pack.template_profile {
            let key = format!("{pack_id}:{ch}:{prof}");
            if seen.insert(key, ()).is_some() {
                continue;
            }
            let pid = prof.trim();
            if pid.is_empty() {
                continue;
            }
            if !profiles.profiles.contains_key(pid) {
                return Err(format!(
                    "topic_pack {pack_id:?} channel {ch:?} references unknown template_profile/projection id {pid:?}; add it to {PROJECTION_PROFILES_YAML_REPO_PATH}"
                ));
            }
        }
    }
    if !profiles.profiles.contains_key("fallback") {
        return Err("projection profiles must define \"fallback\"".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChannelPolicyConfig, SyndicationConfig, UnifiedNewsItem};
    use chrono::Utc;

    #[test]
    fn embedded_projection_profiles_load() {
        let p = embedded_profiles();
        assert!(
            p.profiles.contains_key("short_insight_thread"),
            "twitter pack profile"
        );
        assert!(p.profiles.contains_key("fallback"));
    }

    #[test]
    fn topic_pack_profiles_all_resolve() {
        validate_topic_pack_projection_profiles()
            .expect("all topic pack template_profile ids valid");
    }

    #[test]
    fn compile_emits_digest_and_twitter_plan() {
        let mut syn = SyndicationConfig::default();
        syn.social.push(crate::types::SocialChannel::Twitter);
        syn.distribution_policy.channel_policy.insert(
            "twitter".to_string(),
            ChannelPolicyConfig {
                template_profile: Some("short_insight_thread".to_string()),
                ..Default::default()
            },
        );
        let item = UnifiedNewsItem {
            id: "t1".to_string(),
            title: "Hello".to_string(),
            author: "a".to_string(),
            published_at: Utc::now(),
            tags: vec![],
            content_markdown: "body".to_string(),
            syndication: syn,
            topic_pack: None,
        };
        let r1 = compile_for_publish(&item);
        let r2 = compile_for_publish(&item);
        assert_eq!(r1.derivation_digest_hex, r2.derivation_digest_hex);
        assert_eq!(r1.channel_plans.len(), 1);
        assert_eq!(r1.channel_plans[0].channel, "twitter");
        assert_eq!(
            r1.channel_plans[0].projection_profile_id,
            "short_insight_thread"
        );
    }

    #[test]
    fn unknown_profile_warns_and_falls_back() {
        let mut syn = SyndicationConfig {
            rss: true,
            ..SyndicationConfig::default()
        };
        syn.distribution_policy.channel_policy.insert(
            "rss".to_string(),
            ChannelPolicyConfig {
                template_profile: Some("does_not_exist".to_string()),
                ..Default::default()
            },
        );
        let item = UnifiedNewsItem {
            id: "t2".to_string(),
            title: "Hi".to_string(),
            author: "b".to_string(),
            published_at: Utc::now(),
            tags: vec![],
            content_markdown: "x".to_string(),
            syndication: syn,
            topic_pack: None,
        };
        let r = compile_for_publish(&item);
        assert!(!r.warnings.is_empty());
        assert_eq!(r.channel_plans[0].projection_profile_id, "fallback");
    }
}
