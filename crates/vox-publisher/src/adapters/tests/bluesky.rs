use crate::PublisherConfig;
use crate::adapters::bluesky;
use crate::types::UnifiedNewsItem;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_bluesky_post_success() {
    let mock_server = MockServer::start().await;
    let handle = "test.bsky.social";
    let password = "test-password";
    let pds_base = mock_server.uri();
    let item = UnifiedNewsItem {
        id: "test-item".to_string(),
        title: "Test Title".to_string(),
        content_markdown: "Test Content".to_string(),
        ..Default::default()
    };
    let publisher_cfg = PublisherConfig {
        bluesky_pds_url: Some(pds_base),
        ..PublisherConfig::default()
    };

    // 1. Mock createSession
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.server.createSession"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accessJwt": "test-access-jwt",
            "refreshJwt": "test-refresh-jwt",
            "handle": handle,
            "did": "did:plc:test"
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock createRecord
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": "at://did:plc:test/app.bsky.feed.post/123",
            "cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3hlgtv7u7xlzyba"
        })))
        .mount(&mock_server)
        .await;

    let result = bluesky::post(&publisher_cfg, handle, password, &item, false).await;
    assert!(result.is_ok(), "Bluesky post failed: {:?}", result.err());
}

#[cfg(feature = "scientia-bluesky-sdk")]
#[tokio::test]
async fn test_bluesky_sdk_post_dry_run() {
    let publisher_cfg = PublisherConfig::default();
    let item = UnifiedNewsItem {
        id: "test".to_string(),
        title: "Test Title".to_string(),
        content_markdown: "Test content".to_string(),
        ..Default::default()
    };
    let result = bluesky::post(&publisher_cfg, "handle", "password", &item, true).await;
    assert!(result.is_ok());
    assert!(result.unwrap().starts_with("dry-run-bluesky-"));
}
