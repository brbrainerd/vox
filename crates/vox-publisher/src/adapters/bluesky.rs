use crate::PublisherConfig;
use crate::types::UnifiedNewsItem;
use anyhow::{Result, anyhow};
#[cfg(not(feature = "scientia-bluesky-sdk"))]
use reqwest::Client;
#[cfg(not(feature = "scientia-bluesky-sdk"))]
use serde::{Deserialize, Serialize};
#[cfg(not(feature = "scientia-bluesky-sdk"))]
use std::sync::OnceLock;
#[cfg(not(feature = "scientia-bluesky-sdk"))]
use tokio::sync::Mutex;

pub const DEFAULT_PDS: &str = "https://bsky.social";
pub const SESSION_PATH: &str = "/xrpc/com.atproto.server.createSession";
pub const RECORD_PATH: &str = "/xrpc/com.atproto.repo.createRecord";
pub const DESCRIBE_PATH: &str = "/xrpc/com.atproto.server.describeServer";
pub const FEED_COLLECTION: &str = "app.bsky.feed.post";
pub const SESSION_TTL_SECS: u64 = 110 * 60;

#[cfg(not(feature = "scientia-bluesky-sdk"))]
#[derive(Serialize)]
struct CreateSessionRequest<'a> {
    identifier: &'a str,
    password: &'a str,
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
#[derive(Deserialize, Clone)]
struct CreateSessionResponse {
    #[serde(rename = "accessJwt")]
    access_jwt: String,
    #[serde(rename = "refreshJwt")]
    #[allow(dead_code)]
    refresh_jwt: String,
    did: String,
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
#[derive(Serialize)]
struct PostRecord {
    #[serde(rename = "$type")]
    type_: &'static str,
    text: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
#[derive(Serialize)]
struct CreateRecordRequest<'a> {
    repo: &'a str,
    collection: &'static str,
    record: PostRecord,
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
struct SessionCacheEntry {
    session: CreateSessionResponse,
    expires_at: std::time::Instant,
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
use std::collections::HashMap;

#[cfg(not(feature = "scientia-bluesky-sdk"))]
static BLUESKY_SESSION_CACHE: OnceLock<Mutex<HashMap<String, SessionCacheEntry>>> = OnceLock::new();

#[cfg(not(feature = "scientia-bluesky-sdk"))]
fn session_cache() -> &'static Mutex<HashMap<String, SessionCacheEntry>> {
    BLUESKY_SESSION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn post(
    publisher_cfg: &PublisherConfig,
    handle: &str,
    password: &str,
    item: &UnifiedNewsItem,
    dry_run: bool,
) -> Result<String> {
    if dry_run {
        return Ok(format!("dry-run-bluesky-{}", item.id));
    }

    #[cfg(feature = "scientia-bluesky-sdk")]
    return sdk_post(publisher_cfg, handle, password, item).await;

    #[cfg(not(feature = "scientia-bluesky-sdk"))]
    return legacy_post(publisher_cfg, handle, password, item).await;
}

#[cfg(not(feature = "scientia-bluesky-sdk"))]
async fn legacy_post(
    publisher_cfg: &PublisherConfig,
    handle: &str,
    password: &str,
    item: &UnifiedNewsItem,
) -> Result<String> {
    let client = Client::new();
    let base = publisher_cfg
        .bluesky_pds_url
        .as_deref()
        .unwrap_or(DEFAULT_PDS)
        .trim_end_matches('/');
    let cache_key = format!("{}::{}", base, handle);
    let mut cache_guard = session_cache().lock().await;

    let mut maybe_session = None;
    if let Some(entry) = cache_guard.get(&cache_key)
        && std::time::Instant::now() < entry.expires_at
    {
        maybe_session = Some(entry.session.clone());
    }

    let session = if let Some(s) = maybe_session {
        s
    } else {
        let session_resp = client
            .post(format!("{}/xrpc/com.atproto.server.createSession", base))
            .json(&CreateSessionRequest {
                identifier: handle,
                password,
            })
            .send()
            .await?;

        if !session_resp.status().is_success() {
            let err_body = session_resp.text().await.unwrap_or_default();
            return Err(anyhow!("Bluesky session creation failed: {}", err_body));
        }

        let parsed: CreateSessionResponse = session_resp.json().await?;

        let entry = SessionCacheEntry {
            session: parsed.clone(),
            // AT Protocol accessJwt usually lasts 2 hours, be conservative with 1 hr 50 mins
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(110 * 60),
        };
        cache_guard.insert(cache_key, entry);
        parsed
    };

    // We can confidently drop the lock now
    drop(cache_guard);

    use unicode_segmentation::UnicodeSegmentation;
    pub const POST_MAX_GRAPHEMES: usize = 300;
    pub const POST_TRUNCATION_SUFFIX: &str = "…";

    // Prefer short_summary; fall back to title (never full article markdown).
    let raw_text = item
        .syndication
        .short_summary
        .as_deref()
        .unwrap_or(item.title.as_str());

    // Enforce the 300-grapheme AT Protocol limit.
    let grapheme_count = raw_text.graphemes(true).count();
    let text = if grapheme_count > POST_MAX_GRAPHEMES {
        let truncated: String = raw_text
            .graphemes(true)
            .take(POST_MAX_GRAPHEMES - 1)
            .collect();
        format!("{}{}", truncated, POST_TRUNCATION_SUFFIX)
    } else {
        raw_text.to_string()
    };

    if text.is_empty() {
        return Err(anyhow!("Bluesky post text cannot be empty"));
    }

    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let post_resp = client
        .post(format!("{}/xrpc/com.atproto.repo.createRecord", base))
        .bearer_auth(&session.access_jwt)
        .json(&CreateRecordRequest {
            repo: &session.did,
            collection: "app.bsky.feed.post",
            record: PostRecord {
                type_: "app.bsky.feed.post",
                text,
                created_at,
            },
        })
        .send()
        .await?;

    if !post_resp.status().is_success() {
        let err_body = post_resp.text().await.unwrap_or_default();
        return Err(anyhow!("Bluesky post creation failed: {}", err_body));
    }

    Ok("bluesky_post_success".to_string())
}

#[cfg(feature = "scientia-bluesky-sdk")]
async fn sdk_post(
    publisher_cfg: &PublisherConfig,
    handle: &str,
    password: &str,
    item: &UnifiedNewsItem,
) -> Result<String> {
    use atrium_api::app::bsky::feed::post::RecordData;
    use atrium_api::types::string::Datetime;
    use bsky_sdk::{BskyAgent, agent::config::Config};

    let pds_url = publisher_cfg
        .bluesky_pds_url
        .as_deref()
        .unwrap_or(DEFAULT_PDS)
        .to_string();

    let agent: BskyAgent<_> = BskyAgent::builder()
        .config(Config {
            endpoint: pds_url,
            ..Default::default()
        })
        .build()
        .await
        .map_err(|e| anyhow!("bsky-sdk agent build failed: {}", e))?;

    agent
        .login(handle, password)
        .await
        .map_err(|e| anyhow!("Bluesky login failed: {}", e))?;

    // Select text source (short_summary → title → never raw markdown)
    let raw_text = item
        .syndication
        .short_summary
        .as_deref()
        .unwrap_or(item.title.as_str());

    use bsky_sdk::rich_text::RichText;

    // RichText handles grapheme counting and facet (URL link) auto-detection.
    let rt = RichText::new_with_detect_facets(raw_text.to_string())
        .await
        .map_err(|e| anyhow!("RichText facet detection failed: {}", e))?;

    let output = agent
        .create_record(RecordData {
            created_at: Datetime::now(),
            text: rt.text,
            facets: rt.facets,
            embed: None,
            entities: None,
            labels: None,
            langs: None,
            reply: None,
            tags: None,
        })
        .await
        .map_err(|e| anyhow!("Bluesky create_record failed: {}", e))?;

    Ok(output.uri.to_string())
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;

    #[cfg(not(feature = "scientia-bluesky-sdk"))]
    #[test]
    fn session_cache_returns_stable_singleton() {
        let a = session_cache();
        let b = session_cache();
        // OnceLock::get_or_init must yield the same &'static instance every call.
        assert!(std::ptr::eq(a, b));
    }
}
