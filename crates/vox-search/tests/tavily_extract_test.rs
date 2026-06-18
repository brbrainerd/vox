//! Wiremock stub for Tavily `/extract` uplift (`tavily_extract`).

use vox_search::policy::SearchPolicy;
use vox_search::tavily_extract::{TavilyExtractClient, snippet_quality_low};
use vox_search::web_dispatcher::WebSearchDispatcher;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn snippet_quality_heuristic() {
    assert!(snippet_quality_low("tiny"));
    assert!(!snippet_quality_low(
        "A sufficiently long alphanumeric snippet that should not trigger Tavily extract uplift because it has enough grounding text."
    ));
}

#[tokio::test]
async fn extract_client_maps_wiremock_response() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/extract"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "url": "https://example.test/page",
                "raw_content": "Expanded markdown body from Tavily extract with enough detail for grounding and citation."
            }],
            "failed_results": [],
            "response_time": 0.42
        })))
        .mount(&mock)
        .await;

    let client = TavilyExtractClient::with_base_url("test-key", &mock.uri()).expect("client");
    let hits = client
        .extract_urls(
            &[String::from("https://example.test/page")],
            Some("query text"),
        )
        .await
        .expect("extract");

    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("Expanded markdown"));
}

#[tokio::test]
async fn extract_uplift_replaces_thin_snippet_content() {
    let tavily = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/extract"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "url": "https://example.test/thin",
                "raw_content": "This uplifted extract body is long enough to pass snippet quality checks and provide useful grounding for the research pipeline."
            }],
            "failed_results": [],
            "response_time": 0.5
        })))
        .mount(&tavily)
        .await;

    let mut rows = vec![vox_search::searxng::SearxngResult {
        url: "https://example.test/thin".to_string(),
        title: "Thin page".to_string(),
        content: "short".to_string(),
        engine: Some("google".to_string()),
        score: Some(0.7),
    }];
    assert!(snippet_quality_low(&rows[0].content));

    let client = TavilyExtractClient::with_base_url("test-key", &tavily.uri()).expect("client");
    let extracted = client
        .extract_urls(&[rows[0].url.clone()], Some("integration query"))
        .await
        .expect("extract");
    for hit in extracted {
        if let Some(row) = rows.iter_mut().find(|r| r.url == hit.url) {
            row.content = hit.content;
        }
    }
    assert!(!snippet_quality_low(&rows[0].content));
}

#[tokio::test]
async fn web_dispatcher_maps_searxng_without_extract_when_snippet_ok() {
    let searx = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "url": "https://example.test/page",
                "title": "Title",
                "content": "A sufficiently long alphanumeric snippet that should not trigger Tavily extract uplift because it has enough grounding text for the pipeline.",
                "engine": "google",
                "score": 0.9
            }]
        })))
        .mount(&searx)
        .await;

    let policy = SearchPolicy {
        searxng_url: Some(searx.uri()),
        duckduckgo_fallback_enabled: false,
        tavily_enabled: false,
        ..SearchPolicy::default()
    };

    let hits = WebSearchDispatcher::search("query text", &policy)
        .await
        .expect("dispatcher");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "https://example.test/page");
}
