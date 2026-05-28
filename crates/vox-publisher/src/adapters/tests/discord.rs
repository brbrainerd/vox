use crate::adapters::discord;
use crate::adapters::tests::{config_fixture, item_fixture};
use crate::types::DiscordOverride;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn discord_contract_success() {
    let mock_server = MockServer::start().await;
    let item = item_fixture();
    let mut publisher_cfg = config_fixture(None);
    publisher_cfg.discord_webhook_url = Some(format!("{}/webhook/123", mock_server.uri()));

    let cfg = DiscordOverride {
        message: Some("hello".to_string()),
    };

    Mock::given(method("POST"))
        .and(path("/webhook/123"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let res = discord::post(&publisher_cfg, &item, Some(&cfg), false)
        .await
        .unwrap();
    assert!(res.starts_with("discord-"));
}

#[tokio::test]
async fn discord_auto_truncation_check() {
    let item = item_fixture();
    let mut publisher_cfg = config_fixture(None);
    publisher_cfg.discord_webhook_url = Some("http://localhost/dummy".to_string());
    let cfg = DiscordOverride {
        message: Some("x".repeat(2001)),
    };

    let res = discord::post(&publisher_cfg, &item, Some(&cfg), true).await;
    assert!(res.is_ok());
}
