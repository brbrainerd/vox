//! Shared switching helpers used by CLI/MCP/orchestrator publication surfaces.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::types::{SyndicationConfig, UnifiedNewsItem};
use crate::{ChannelOutcome, SyndicationResult};

/// Legacy metadata root key accepted for backward compatibility (prefer `syndication`).
pub const LEGACY_METADATA_SYNDICATION_KEY: &str = "scientia_distribution";
const ROOT_CHANNEL_POLICY_KEY: &str = "channel_policy";
const ROOT_CROSSPOST_PLAN_KEY: &str = "crosspost_plan";

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ManifestSyndicationParseNotes {
    pub warnings: Vec<String>,
    pub used_legacy_distribution_key: bool,
}

/// Build a runtime item from manifest row fields and optional `metadata_json`.
pub fn unified_news_item_from_manifest_parts(
    publication_id: &str,
    title: &str,
    author: &str,
    body_markdown: &str,
    metadata_json: Option<&str>,
) -> anyhow::Result<UnifiedNewsItem> {
    let (item, notes) = unified_news_item_from_manifest_parts_notes(
        publication_id,
        title,
        author,
        body_markdown,
        metadata_json,
    )?;
    for w in &notes.warnings {
        tracing::warn!(target: "vox.publisher.switching", "{}", w);
    }
    Ok(item)
}

/// Like [`unified_news_item_from_manifest_parts`], but returns parse notes (warnings, legacy key usage).
pub fn unified_news_item_from_manifest_parts_notes(
    publication_id: &str,
    title: &str,
    author: &str,
    body_markdown: &str,
    metadata_json: Option<&str>,
) -> anyhow::Result<(UnifiedNewsItem, ManifestSyndicationParseNotes)> {
    let mut notes = ManifestSyndicationParseNotes::default();
    let trimmed = metadata_json.map(str::trim).filter(|s| !s.is_empty());

    let root: Value = trimmed
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("parse metadata_json for route simulation/publish: {e}"))?
        .unwrap_or_else(|| json!({}));

    let tags: Vec<String> = root
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| {
                    x.as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    let topic_pack = root
        .get("topic_pack")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);

    let published_at = root
        .get("published_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.trim().parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    let legacy = root.get(LEGACY_METADATA_SYNDICATION_KEY).cloned();
    let canonical = root.get("syndication").cloned();
    if legacy.is_some() {
        notes.used_legacy_distribution_key = true;
        notes.warnings.push(format!(
            "metadata_json uses deprecated key `{LEGACY_METADATA_SYNDICATION_KEY}`; prefer `syndication` as the canonical root."
        ));
    }

    let merged_syndication_val = merge_syndication_json_values(legacy, canonical, &mut notes)?;
    let syndication: SyndicationConfig = if merged_syndication_val.is_null()
        || merged_syndication_val
            .as_object()
            .is_some_and(|m| m.is_empty())
    {
        SyndicationConfig::default()
    } else {
        serde_json::from_value(merged_syndication_val).map_err(|e| {
            anyhow::anyhow!(
                "metadata_json syndication (after legacy merge / shape normalization): {e}"
            )
        })?
    };

    let mut item = UnifiedNewsItem {
        id: publication_id.to_string(),
        title: title.to_string(),
        author: author.to_string(),
        published_at,
        tags,
        content_markdown: body_markdown.to_string(),
        syndication,
        topic_pack,
    };
    item.hydrate_topic_pack_if_set()?;
    if item.syndication.distribution_policy.dry_run == Some(true) {
        item.syndication.dry_run = true;
    }
    Ok((item, notes))
}

fn merge_syndication_json_values(
    legacy: Option<Value>,
    canonical: Option<Value>,
    notes: &mut ManifestSyndicationParseNotes,
) -> anyhow::Result<Value> {
    let base = normalize_distribution_json_value_with_warnings(
        legacy.unwrap_or(Value::Null),
        &mut notes.warnings,
    )?;
    let overlay = normalize_distribution_json_value_with_warnings(
        canonical.unwrap_or(Value::Null),
        &mut notes.warnings,
    )?;
    Ok(deep_merge_json(base, overlay))
}

/// Normalizes contract `channels` / `channel_payloads` shape into a flat [`SyndicationConfig`]-compatible object.
pub fn normalize_distribution_json_value(v: Value) -> anyhow::Result<Value> {
    let mut warnings = Vec::new();
    normalize_distribution_json_value_with_warnings(v, &mut warnings)
}

fn normalize_distribution_json_value_with_warnings(
    v: Value,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Value> {
    if v.is_null() {
        return Ok(json!({}));
    }
    let Some(obj) = v.as_object().cloned() else {
        anyhow::bail!("syndication value must be a JSON object or null");
    };
    let is_contract_shape = obj.contains_key("channel_payloads") || obj.contains_key("channels");
    if !is_contract_shape {
        return Ok(Value::Object(obj));
    }

    let mut out = serde_json::Map::new();
    let channels = obj
        .get("channels")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let channel_ids: Vec<String> = channels
        .iter()
        .filter_map(|c| {
            c.as_str()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
        })
        .collect();

    let payloads = obj
        .get("channel_payloads")
        .and_then(|c| c.as_object())
        .cloned()
        .unwrap_or_default();
    if obj.contains_key(ROOT_CROSSPOST_PLAN_KEY) {
        warnings.push(format!(
            "`{ROOT_CROSSPOST_PLAN_KEY}` is reserved and currently ignored by runtime hydration."
        ));
    }

    let rss_on = channel_ids.iter().any(|c| c == "rss");
    out.insert("rss".to_string(), json!(rss_on));

    for key in [
        "twitter",
        "github",
        "open_collective",
        "reddit",
        "hacker_news",
        "youtube",
        "crates_io",
        "bluesky",
        "mastodon",
        "linkedin",
        "discord",
    ] {
        if let Some(payload) = payloads.get(key) {
            out.insert(key.to_string(), payload.clone());
        } else if channel_ids.iter().any(|c| c == key) && channel_allows_empty_payload(key) {
            out.insert(key.to_string(), json!({}));
        } else if channel_ids.iter().any(|c| c == key) {
            warnings.push(format!(
                "syndication.channels lists `{key}`, but runtime needs `channel_payloads.{key}` before that channel will materialize."
            ));
        }
    }

    let mut distribution_policy = obj.get("distribution_policy").cloned();
    if let Some(root_policy) = obj.get(ROOT_CHANNEL_POLICY_KEY) {
        warnings.push(format!(
            "root `{ROOT_CHANNEL_POLICY_KEY}` is deprecated; prefer `distribution_policy.channel_policy`."
        ));
        let target = distribution_policy.get_or_insert_with(|| json!({}));
        if let Some(map) = target.as_object_mut()
            && !map.contains_key(ROOT_CHANNEL_POLICY_KEY)
        {
            map.insert(ROOT_CHANNEL_POLICY_KEY.to_string(), root_policy.clone());
        }
    }
    if let Some(dp) = distribution_policy {
        out.insert("distribution_policy".to_string(), dp);
    }
    if let Some(d) = obj.get("dry_run") {
        out.insert("dry_run".to_string(), d.clone());
    }
    Ok(Value::Object(out))
}

fn channel_allows_empty_payload(key: &str) -> bool {
    matches!(
        key,
        "twitter" | "hacker_news" | "bluesky" | "mastodon" | "linkedin" | "discord"
    )
}

fn deep_merge_json(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut a), Value::Object(b)) => {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                    continue;
                }
                match a.remove(&k) {
                    Some(existing) => {
                        a.insert(k, deep_merge_json(existing, v));
                    }
                    None => {
                        a.insert(k, v);
                    }
                }
            }
            Value::Object(a)
        }
        (_, overlay) => overlay,
    }
}

/// Parse and normalize channel CSV from CLI flags.
#[must_use]
pub fn parse_channels_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(normalize_channel_name)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
}

/// Normalize channel names from API payloads/tool params.
#[must_use]
pub fn normalize_channels(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|s| normalize_channel_name(s))
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
}

#[must_use]
pub fn normalize_channel_name(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Apply operator allowlist for selective publication.
pub fn apply_channel_allowlist(item: &mut UnifiedNewsItem, allowed: &[String]) {
    let has = |name: &str| allowed.iter().any(|x| x == name);
    if !has("rss") {
        item.syndication.rss = false;
    }
    if !has("twitter") {
        item.syndication
            .social
            .retain(|c| c != &crate::types::SocialChannel::Twitter);
    }
    if !has("github") {
        item.syndication.forge = None;
    }
    if !has("open_collective") {
        item.syndication.open_collective = None;
    }
    if !has("reddit") {
        item.syndication.reddit = None;
    }
    if !has("hacker_news") {
        item.syndication.hacker_news = false;
    }
    if !has("youtube") {
        item.syndication.youtube = None;
    }
    if !has("crates_io") {
        item.syndication.crates_io = None;
    }
    if !has("bluesky") {
        item.syndication
            .social
            .retain(|c| c != &crate::types::SocialChannel::Bluesky);
    }
    if !has("mastodon") {
        item.syndication
            .social
            .retain(|c| c != &crate::types::SocialChannel::Mastodon);
    }
    if !has("linkedin") {
        item.syndication.linkedin = false;
    }
    if !has("discord") {
        item.syndication
            .social
            .retain(|c| c != &crate::types::SocialChannel::Discord);
    }
}

/// Return channel ids that failed in a publication result.
#[must_use]
pub fn failed_channels(result: &SyndicationResult) -> Vec<String> {
    let mut out = Vec::new();
    let mut maybe = |name: &str, channel_outcome: &ChannelOutcome| {
        if matches!(channel_outcome, ChannelOutcome::Failed { .. }) {
            out.push(name.to_string());
        }
    };
    maybe("rss", &result.rss);
    maybe("twitter", &result.twitter);
    maybe("github", &result.github);
    maybe("open_collective", &result.open_collective);
    maybe("reddit", &result.reddit);
    maybe("hacker_news", &result.hacker_news);
    maybe("youtube", &result.youtube);
    maybe("crates_io", &result.crates_io);
    maybe("bluesky", &result.bluesky);
    maybe("mastodon", &result.mastodon);
    maybe("linkedin", &result.linkedin);
    maybe("discord", &result.discord);
    out
}

/// Return channel ids that succeeded (live) in a publication result.
#[must_use]
pub fn successful_channels(result: &SyndicationResult) -> Vec<String> {
    let mut out = Vec::new();
    let mut maybe = |name: &str, channel_outcome: &ChannelOutcome| {
        if matches!(channel_outcome, ChannelOutcome::Success { .. }) {
            out.push(name.to_string());
        }
    };
    maybe("rss", &result.rss);
    maybe("twitter", &result.twitter);
    maybe("github", &result.github);
    maybe("open_collective", &result.open_collective);
    maybe("reddit", &result.reddit);
    maybe("hacker_news", &result.hacker_news);
    maybe("youtube", &result.youtube);
    maybe("crates_io", &result.crates_io);
    maybe("bluesky", &result.bluesky);
    maybe("mastodon", &result.mastodon);
    maybe("linkedin", &result.linkedin);
    maybe("discord", &result.discord);
    out
}

pub struct AttemptOutcome<'a> {
    pub content_sha3_256: &'a str,
    pub outcome_json: &'a str,
}

/// Describes which channels `publication-retry-failed` will target for a digest-bound attempt.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct PublicationRetryPlan {
    pub will_retry_channels: Vec<String>,
    pub skipped_success_channels: Vec<String>,
    pub blocked_channels: Vec<RetryBlockedChannel>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RetryBlockedChannel {
    pub channel: String,
    pub reason: String,
}

#[must_use]
fn channel_outcome_success(out: &ChannelOutcome) -> bool {
    matches!(out, ChannelOutcome::Success { .. })
}

#[must_use]
fn outcome_for_channel<'a>(result: &'a SyndicationResult, ch: &str) -> Option<&'a ChannelOutcome> {
    Some(match ch {
        "rss" => &result.rss,
        "twitter" => &result.twitter,
        "github" => &result.github,
        "open_collective" => &result.open_collective,
        "reddit" => &result.reddit,
        "hacker_news" => &result.hacker_news,
        "youtube" => &result.youtube,
        "crates_io" => &result.crates_io,
        "bluesky" => &result.bluesky,
        "mastodon" => &result.mastodon,
        "linkedin" => &result.linkedin,
        "discord" => &result.discord,
        _ => return None,
    })
}

/// Build a deterministic retry plan from the latest parseable attempt for `digest`.
///
/// When `explicit_channels` is `None`, the plan is exactly the failed channel set from that attempt.
/// When set (for example a single CLI `--channel`), channels that already **succeeded** for this digest
/// are listed in [`PublicationRetryPlan::skipped_success_channels`] and excluded from `will_retry_channels`.
pub fn plan_publication_retry_channels(
    attempts: &[AttemptOutcome<'_>],
    digest: &str,
    explicit_channels: Option<&[String]>,
) -> anyhow::Result<Option<PublicationRetryPlan>> {
    let Some(latest) = attempts.iter().find(|a| a.content_sha3_256 == digest) else {
        return Ok(None);
    };
    let parsed: SyndicationResult = serde_json::from_str(latest.outcome_json).map_err(|e| {
        anyhow::anyhow!("malformed syndication outcome_json for digest {digest}: {e}")
    })?;
    let failed: Vec<String> = failed_channels(&parsed);
    let success: Vec<String> = successful_channels(&parsed);

    let mut will_retry: Vec<String> = Vec::new();
    let mut skipped_success: Vec<String> = Vec::new();
    let mut blocked: Vec<RetryBlockedChannel> = Vec::new();

    match explicit_channels {
        None => {
            for ch in failed {
                if success.contains(&ch) {
                    skipped_success.push(ch);
                } else {
                    will_retry.push(ch);
                }
            }
            skipped_success.sort();
            will_retry.sort();
        }
        Some(requested) => {
            for raw in requested {
                let ch = normalize_channel_name(raw);
                if ch.is_empty() {
                    continue;
                }
                let Some(out) = outcome_for_channel(&parsed, &ch) else {
                    blocked.push(RetryBlockedChannel {
                        channel: ch,
                        reason: "unknown_channel".to_string(),
                    });
                    continue;
                };
                if channel_outcome_success(out) {
                    skipped_success.push(ch);
                } else if failed.contains(&ch) {
                    will_retry.push(ch);
                } else {
                    blocked.push(RetryBlockedChannel {
                        channel: ch,
                        reason: "not_marked_failed_in_latest_digest_attempt".to_string(),
                    });
                }
            }
            skipped_success.sort();
            will_retry.sort();
            blocked.sort_by(|a, b| a.channel.cmp(&b.channel));
        }
    }

    Ok(Some(PublicationRetryPlan {
        will_retry_channels: will_retry,
        skipped_success_channels: skipped_success,
        blocked_channels: blocked,
    }))
}

/// Parse the latest attempt matching `digest` and return its failed channel list.
///
/// `attempts` must be ordered **newest-first** (as returned by [`vox_db`] publication attempt queries).
/// Parsing failures are surfaced as errors (no silent skip).
pub fn failed_channels_from_latest_digest_attempt(
    attempts: &[AttemptOutcome<'_>],
    digest: &str,
) -> anyhow::Result<Option<Vec<String>>> {
    Ok(plan_publication_retry_channels(attempts, digest, None)?.map(|p| p.will_retry_channels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parse_channels_csv_normalizes() {
        let out = parse_channels_csv(" twitter, reddit ,YOUTUBE ");
        assert_eq!(
            out,
            vec![
                "twitter".to_string(),
                "reddit".to_string(),
                "youtube".to_string()
            ]
        );
    }

    #[test]
    fn distribution_policy_dry_run_true_forces_syndication_dry_run() {
        let meta = r#"{
            "syndication": {
                "dry_run": false,
                "rss": false,
                "distribution_policy": { "dry_run": true }
            }
        }"#;
        let item =
            unified_news_item_from_manifest_parts("p", "t", "a", "b", Some(meta)).expect("item");
        assert!(item.syndication.dry_run);
    }

    #[test]
    fn metadata_published_at_parses_rfc3339() {
        let meta = r#"{
            "published_at": "2024-01-01T00:00:00Z",
            "syndication": { "rss": true, "dry_run": true }
        }"#;
        let item =
            unified_news_item_from_manifest_parts("p", "t", "a", "b", Some(meta)).expect("item");
        assert_eq!(
            item.published_at,
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn legacy_key_merges_under_canonical_syndication() {
        let meta = r#"{
            "scientia_distribution": { "rss": true, "twitter": { "thread": false } },
            "syndication": { "twitter": { "short_text": "hello" } }
        }"#;
        let (item, notes) =
            unified_news_item_from_manifest_parts_notes("p", "t", "a", "b", Some(meta))
                .expect("item");
        assert!(notes.used_legacy_distribution_key);
        assert!(item.syndication.rss);

        let tw = item
            .syndication
            .twitter_override()
            .expect("twitter payload");
        assert!(!tw.thread);
    }

    #[test]
    fn contract_shape_expands_channel_payloads() {
        let meta = r#"{
            "syndication": {
                "channels": ["rss", "twitter"],
                "channel_payloads": { "twitter": { "thread": true } },
                "distribution_policy": { "retry_profile": "minimal" }
            }
        }"#;
        let item =
            unified_news_item_from_manifest_parts("p", "t", "a", "b", Some(meta)).expect("item");
        assert!(item.syndication.rss);

        let tw = item
            .syndication
            .twitter_override()
            .expect("twitter payload");
        assert!(tw.thread);
        assert_eq!(
            item.syndication
                .distribution_policy
                .retry_profile
                .as_deref(),
            Some("minimal")
        );
    }

    #[test]
    fn contract_shape_warns_on_inert_fields_and_missing_payloads() {
        let meta = r#"{
            "syndication": {
                "channels": ["reddit"],
                "channel_policy": {
                    "reddit": { "enabled": true }
                },
                "crosspost_plan": [{ "from": "reddit", "to": "twitter" }]
            }
        }"#;
        let (_item, notes) =
            unified_news_item_from_manifest_parts_notes("p", "t", "a", "b", Some(meta))
                .expect("item");
        assert!(
            notes
                .warnings
                .iter()
                .any(|w| w.contains("root `channel_policy` is deprecated")),
            "{:?}",
            notes.warnings
        );
        assert!(
            notes
                .warnings
                .iter()
                .any(|w| w.contains("`crosspost_plan` is reserved")),
            "{:?}",
            notes.warnings
        );
        assert!(
            notes
                .warnings
                .iter()
                .any(|w| w.contains("channel_payloads.reddit")),
            "{:?}",
            notes.warnings
        );
    }

    #[test]
    fn latest_digest_attempt_parse_errors_are_not_swallowed() {
        let ok_json = serde_json::to_string(&SyndicationResult::default()).expect("json");
        let attempts = [
            AttemptOutcome {
                content_sha3_256: "d1",
                outcome_json: "{not-json",
            },
            AttemptOutcome {
                content_sha3_256: "d1",
                outcome_json: ok_json.as_str(),
            },
        ];
        let err = failed_channels_from_latest_digest_attempt(&attempts, "d1").unwrap_err();
        assert!(
            err.to_string()
                .contains("malformed syndication outcome_json"),
            "{err}"
        );
    }

    #[test]
    fn retry_plan_skips_explicit_channel_already_success() {
        let r = SyndicationResult {
            twitter: ChannelOutcome::Success {
                external_id: Some("1".into()),
            },
            github: ChannelOutcome::Failed {
                code: "x".into(),
                message: "m".into(),
                retryable: true,
                failure_class: None,
            },
            ..SyndicationResult::default()
        };
        let json = serde_json::to_string(&r).expect("json");
        let attempts = [AttemptOutcome {
            content_sha3_256: "d1",
            outcome_json: json.as_str(),
        }];
        let plan = plan_publication_retry_channels(
            &attempts,
            "d1",
            Some(&["twitter".into(), "github".into()]),
        )
        .expect("plan")
        .expect("some");
        assert_eq!(plan.will_retry_channels, vec!["github".to_string()]);
        assert_eq!(plan.skipped_success_channels, vec!["twitter".to_string()]);
        assert!(plan.blocked_channels.is_empty());
    }

    #[test]
    fn retry_auto_mode_lists_only_failed() {
        let r = SyndicationResult {
            twitter: ChannelOutcome::Success {
                external_id: Some("1".into()),
            },
            github: ChannelOutcome::Failed {
                code: "x".into(),
                message: "m".into(),
                retryable: true,
                failure_class: None,
            },
            ..SyndicationResult::default()
        };
        let json = serde_json::to_string(&r).expect("json");
        let attempts = [AttemptOutcome {
            content_sha3_256: "d1",
            outcome_json: json.as_str(),
        }];
        let plan = plan_publication_retry_channels(&attempts, "d1", None)
            .expect("plan")
            .expect("some");
        assert_eq!(plan.will_retry_channels, vec!["github".to_string()]);
        assert!(plan.skipped_success_channels.is_empty());
    }
}
