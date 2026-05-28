//! Integration-style tests: real HTTP to a local mock server (no production endpoints).

use std::time::Duration;

use axum::{Json, Router, routing::post};
use chrono::Utc;
use serde_json::json;
use tokio::net::TcpListener;
use vox_publisher::types::{OpenCollectiveConfig, SyndicationConfig, UnifiedNewsItem};
use vox_publisher::{Publisher, PublisherConfig};

#[tokio::test]
async fn twitter_and_opencollective_use_configured_bases_only() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app = Router::new()
        .route(
            "/2/tweets",
            post(|Json(body): Json<serde_json::Value>| async move {
                assert!(body.get("text").is_some());
                Json(json!({ "data": { "id": "mocktweet1" } }))
            }),
        )
        .route(
            "/graphql/v2",
            post(|Json(body): Json<serde_json::Value>| async move {
                assert!(body.get("query").is_some());
                Json(json!({ "data": { "createUpdate": { "id": "mockoc1" } } }))
            }),
        );

    let _guard = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(vox_config::timeouts::D_80MS).await;

    let base = format!("http://{}", addr);
    let item = UnifiedNewsItem {
        id: "mock-item".to_string(),
        title: "T".to_string(),
        author: "A".to_string(),
        published_at: Utc::now(),
        tags: vec![],
        content_markdown: "body".to_string(),
        syndication: SyndicationConfig {
            social: vec![vox_publisher::types::SocialChannel::Twitter],
            forge: None,
            open_collective: Some(OpenCollectiveConfig {
                is_private: false,
                scheduled_publish_at: None,
            }),
            crates_io: None,
            rss: false,
            dry_run: false,
            ..Default::default()
        },
        topic_pack: None,
    };

    let publisher = Publisher::new(PublisherConfig {
        twitter_bearer_token: Some("bearer".into()),
        open_collective_token: Some("oc-key".into()),
        dry_run: false,
        twitter_api_base: Some(base.clone()),
        opencollective_graphql_url: Some(format!("{}/graphql/v2", base)),
        ..Default::default()
    });

    let out = publisher.publish_all(&item).await.expect("publish");
    assert_eq!(out.twitter_id(), Some("mocktweet1"));
    assert_eq!(out.oc_id(), Some("mockoc1"));
}

#[tokio::test]
async fn partial_channel_failure_is_reported_without_short_circuiting() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app = Router::new()
        .route(
            "/2/tweets",
            post(|Json(_body): Json<serde_json::Value>| async move {
                Json(json!({ "data": { "id": "mocktweet1" } }))
            }),
        )
        .route(
            "/graphql/v2",
            post(|Json(_body): Json<serde_json::Value>| async move {
                (
                    axum::http::StatusCode::BAD_GATEWAY,
                    Json(json!({ "errors": [{ "message": "upstream unavailable" }] })),
                )
            }),
        );
    let _guard = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(vox_config::timeouts::D_80MS).await;

    let base = format!("http://{}", addr);
    let item = UnifiedNewsItem {
        id: "mock-item-failure".to_string(),
        title: "T".to_string(),
        author: "A".to_string(),
        published_at: Utc::now(),
        tags: vec![],
        content_markdown: "body".to_string(),
        syndication: SyndicationConfig {
            social: vec![vox_publisher::types::SocialChannel::Twitter],
            forge: None,
            open_collective: Some(OpenCollectiveConfig {
                is_private: false,
                scheduled_publish_at: None,
            }),
            crates_io: None,
            rss: false,
            dry_run: false,
            ..Default::default()
        },
        topic_pack: None,
    };

    let publisher = Publisher::new(PublisherConfig {
        twitter_bearer_token: Some("bearer".into()),
        open_collective_token: Some("oc-key".into()),
        dry_run: false,
        twitter_api_base: Some(base.clone()),
        opencollective_graphql_url: Some(format!("{}/graphql/v2", base)),
        ..Default::default()
    });
    let out = publisher.publish_all(&item).await.expect("publish");
    assert_eq!(out.twitter_id(), Some("mocktweet1"));
    assert!(out.has_failures());
}
