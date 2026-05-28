use crate::PublisherConfig;
use crate::adapters::twitter;
use crate::types::UnifiedNewsItem;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_twitter_post_success() {
    let mock_server = MockServer::start().await;
    let token = "test-token";
    let item = UnifiedNewsItem {
        id: "test-item".to_string(),
        title: "Test Title".to_string(),
        content_markdown: "Test Content".to_string(),
        ..Default::default()
    };

    let publisher_cfg = PublisherConfig {
        twitter_api_base: Some(mock_server.uri()),
        ..Default::default()
    };

    Mock::given(method("POST"))
        .and(path("/2/tweets"))
        .and(header(
            "Authorization",
            vox_http_client::bearer_auth_header_string("test-token").as_str(),
        ))
        .and(body_json(json!({"text": "Test Content"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "data": { "id": "tweet-123" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result: anyhow::Result<String> =
        twitter::post(&publisher_cfg, token, &item, None, false).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "tweet-123");
}

#[tokio::test]
async fn test_twitter_post_dry_run() {
    let mock_server = MockServer::start().await;
    let token = "test-token";
    let item = UnifiedNewsItem {
        id: "test-item".to_string(),
        ..Default::default()
    };
    let publisher_cfg = PublisherConfig {
        twitter_api_base: Some(mock_server.uri()),
        ..Default::default()
    };

    // No mocks mounted -> any request would cause panic if it happened

    let result: anyhow::Result<String> =
        twitter::post(&publisher_cfg, token, &item, None, true).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("dry-run-twitter"));
}
