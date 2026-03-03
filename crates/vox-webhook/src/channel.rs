//! Channel abstraction — Discord, Slack, WebSocket, and custom integrations.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::WebhookError;

/// Channel kind discriminant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Discord,
    Slack,
    WebSocket,
    Webhook,
    Custom(String),
}

impl std::fmt::Display for ChannelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(s) => write!(f, "custom:{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// A message event sent through a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEvent {
    pub channel_id: String,
    pub kind: ChannelKind,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

/// Configuration for a registered channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub kind: ChannelKind,
    pub endpoint: String,
    pub token: Option<String>,
    pub enabled: bool,
}

impl Channel {
    pub fn new(id: impl Into<String>, kind: ChannelKind, endpoint: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            endpoint: endpoint.into(),
            token: None,
            enabled: true,
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

/// Manages registered channels and dispatches events.
pub struct ChannelManager {
    channels: Mutex<HashMap<String, Channel>>,
    client: reqwest::Client,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn register(&self, channel: Channel) {
        info!(id = %channel.id, kind = %channel.kind, "Channel registered");
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(channel.id.clone(), channel);
    }

    pub fn unregister(&self, id: &str) -> bool {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)
            .is_some()
    }

    pub fn get(&self, id: &str) -> Option<Channel> {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    pub fn list(&self) -> Vec<Channel> {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Send a text message to a channel.
    pub async fn send(&self, channel_id: &str, content: &str) -> Result<(), WebhookError> {
        let ch = {
            let channels = self.channels.lock().unwrap_or_else(|e| e.into_inner());
            match channels.get(channel_id).cloned() {
                Some(c) if c.enabled => c,
                Some(_) => {
                    return Err(WebhookError::Channel(format!(
                        "Channel '{channel_id}' is disabled"
                    )))
                }
                None => {
                    return Err(WebhookError::Channel(format!(
                        "Channel '{channel_id}' not found"
                    )))
                }
            }
        };

        match ch.kind {
            ChannelKind::Discord => self.send_discord(&ch, content).await,
            ChannelKind::Slack => self.send_slack(&ch, content).await,
            ChannelKind::Webhook | ChannelKind::Custom(_) => {
                self.send_generic_webhook(&ch, content).await
            }
            ChannelKind::WebSocket => {
                // WebSocket send would require an active connection registry;
                // for now log and no-op (future: store active WS senders in Arc<DashMap>)
                warn!(
                    channel_id,
                    "WebSocket send not yet implemented, message dropped"
                );
                Ok(())
            }
        }
    }

    async fn send_discord(&self, ch: &Channel, content: &str) -> Result<(), WebhookError> {
        let body = serde_json::json!({ "content": content });
        let mut req = self.client.post(&ch.endpoint).json(&body);
        if let Some(ref token) = ch.token {
            req = req.header("Authorization", format!("Bot {token}"));
        }
        req.send()
            .await
            .map_err(|e| WebhookError::Http(e.to_string()))?;
        Ok(())
    }

    async fn send_slack(&self, ch: &Channel, content: &str) -> Result<(), WebhookError> {
        let body = serde_json::json!({ "text": content });
        let mut req = self.client.post(&ch.endpoint).json(&body);
        if let Some(ref token) = ch.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.send()
            .await
            .map_err(|e| WebhookError::Http(e.to_string()))?;
        Ok(())
    }

    async fn send_generic_webhook(&self, ch: &Channel, content: &str) -> Result<(), WebhookError> {
        let body = serde_json::json!({
            "channel_id": ch.id,
            "content": content,
        });
        let mut req = self
            .client
            .post(&ch.endpoint)
            .header("Content-Type", "application/json")
            .json(&body);
        if let Some(ref token) = ch.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.send()
            .await
            .map_err(|e| WebhookError::Http(e.to_string()))?;
        Ok(())
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_list_channels() {
        let mgr = ChannelManager::new();
        let ch = Channel::new(
            "discord-alerts",
            ChannelKind::Discord,
            "https://discord.com/api/webhooks/xxx",
        );
        mgr.register(ch);
        let list = mgr.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "discord-alerts");
    }

    #[test]
    fn unregister_channel() {
        let mgr = ChannelManager::new();
        let ch = Channel::new(
            "slack-dev",
            ChannelKind::Slack,
            "https://hooks.slack.com/xxx",
        );
        mgr.register(ch);
        assert!(mgr.unregister("slack-dev"));
        assert!(mgr.get("slack-dev").is_none());
    }

    #[test]
    fn channel_kind_display() {
        assert_eq!(ChannelKind::Discord.to_string(), "Discord");
        assert_eq!(
            ChannelKind::Custom("teams".into()).to_string(),
            "custom:teams"
        );
    }
}
