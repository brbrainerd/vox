use crate::PublisherConfig;
use crate::types::{MastodonOverride, UnifiedNewsItem};
use anyhow::{Result, anyhow};

pub const STATUS_MAX: usize = 500;
pub const DEFAULT_VISIBILITY: &str = "public";
pub const CANARY_PATH: &str = "/api/v2/instance";

pub async fn post(
    publisher_cfg: &PublisherConfig,
    item: &UnifiedNewsItem,
    cfg: Option<&MastodonOverride>,
    short_summary: &str,
    dry_run: bool,
) -> Result<String> {
    if dry_run {
        return Ok(format!("dry-run-mastodon-{}", item.id));
    }

    let token = publisher_cfg
        .mastodon_access_token
        .as_deref()
        .ok_or_else(|| anyhow!("Mastodon config present but missing access token"))?;

    let mut instance = publisher_cfg.mastodon_domain.clone().unwrap_or_default();

    if instance.trim().is_empty() {
        return Err(anyhow!(
            "Mastodon domain missing; set VoxSocialMastodonDomain or instance_url_override"
        ));
    }

    if !instance.starts_with("http") {
        instance = format!("https://{}", instance);
    }
    let endpoint = format!("{}/api/v1/statuses", instance.trim_end_matches('/'));

    let status = short_summary.to_string();
    use unicode_segmentation::UnicodeSegmentation;
    let status_len = status.graphemes(true).count();
    if status_len > STATUS_MAX {
        return Err(anyhow!(
            "Mastodon status ({} graphemes) exceeds {} limit",
            status_len,
            STATUS_MAX
        ));
    }

    let visibility = cfg
        .and_then(|c| c.visibility.clone())
        .unwrap_or_else(|| DEFAULT_VISIBILITY.to_string());
    let sensitive = cfg
        .and_then(|c| c.spoiler_text.as_ref().map(|_| true))
        .unwrap_or(false);
    let spoiler_text = cfg.and_then(|c| c.spoiler_text.clone());
    let language = cfg.and_then(|c| c.language.clone());

    let mut payload = serde_json::json!({
        "status": status,
        "visibility": visibility,
        "sensitive": sensitive,
    });

    if let Some(st) = spoiler_text {
        payload["spoiler_text"] = serde_json::json!(st);
    }
    if let Some(lang) = language {
        payload["language"] = serde_json::json!(lang);
    }

    let client = vox_http_client::client();
    let res = client
        .post(&endpoint)
        .bearer_auth(token)
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow!("Mastodon API failed: {}", text));
    }

    let body: serde_json::Value = res.json().await?;
    let post_url = body
        .get("url")
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("mastodon-{}", item.id));

    Ok(post_url)
}
