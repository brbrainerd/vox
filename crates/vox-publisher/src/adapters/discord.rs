use crate::PublisherConfig;
use crate::types::{DiscordOverride, UnifiedNewsItem};
use anyhow::{Result, anyhow};

pub const CONTENT_MAX: usize = 2000;

pub async fn post(
    publisher_cfg: &PublisherConfig,
    item: &UnifiedNewsItem,
    cfg: Option<&DiscordOverride>,
    dry_run: bool,
) -> Result<String> {
    let webhook_url = publisher_cfg
        .discord_webhook_url
        .clone()
        .ok_or_else(|| anyhow!("Missing Discord webhook URL"))?;

    let message_content = cfg.and_then(|c| c.message.clone()).unwrap_or_else(|| {
        let mut msg = format!("**{}**\n\n", item.title);
        msg.push_str(&item.content_markdown);
        msg
    });

    use unicode_segmentation::UnicodeSegmentation;
    let auto_truncate = message_content
        .graphemes(true)
        .take(CONTENT_MAX)
        .collect::<String>();

    let payload = serde_json::json!({
        "content": auto_truncate,
    });

    if dry_run {
        return Ok(format!("dry-run-discord-{}", item.id));
    }

    let client = vox_http_client::client();
    let res = client.post(&webhook_url).json(&payload).send().await?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow!("Discord API error: {} - {}", status, text));
    }

    Ok(format!("discord-{}", item.id))
}
