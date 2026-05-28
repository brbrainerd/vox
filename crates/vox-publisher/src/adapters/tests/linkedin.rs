use crate::PublisherConfig;
use crate::adapters::linkedin;
use crate::types::UnifiedNewsItem;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_linkedin_post_success() {
    let mock_server = MockServer::start().await;
    let item = UnifiedNewsItem {
        id: "test-item".to_string(),
        title: "Test Title".to_string(),
        content_markdown: "Test Content".to_string(),
        ..Default::default()
    };

    let publisher_cfg = PublisherConfig {
        linkedin_access_token: Some("test-token".to_string()),
        linkedin_author_urn: Some("urn:li:person:123".to_string()),
        linkedin_api_base: Some(mock_server.uri()),
        ..Default::default()
    };

    Mock::given(method("POST"))
        .and(path("/rest/posts"))
        .and(header(
            "Authorization",
            vox_http_client::bearer_auth_header_string("test-token").as_str(),
        ))
        .and(header("Linkedin-Version", "202504"))
        .and(header("X-RestLi-Protocol-Version", "2.0.0"))
        .respond_with(ResponseTemplate::new(201).insert_header("x-restli-id", "urn:li:share:456"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = linkedin::post(&publisher_cfg, &item, false).await;
    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(result.unwrap(), "urn:li:share:456");
}
