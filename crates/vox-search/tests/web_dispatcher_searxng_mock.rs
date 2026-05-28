//! Wiremock stub for SearXNG JSON (`WebSearchDispatcher`).

use vox_search::policy::SearchPolicy;
use vox_search::web_dispatcher::WebSearchDispatcher;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn web_dispatcher_maps_searxng_json() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "url": "https://example.test/page",
                "title": "Title",
                "content": "snippet body",
                "engine": "google",
                "score": 0.9
            }]
        })))
        .mount(&mock)
        .await;

    let policy = SearchPolicy {
        searxng_url: Some(mock.uri()),
        duckduckgo_fallback_enabled: false,
        tavily_enabled: false,
        ..SearchPolicy::default()
    };

    let hits = WebSearchDispatcher::search("query text", &policy)
        .await
        .expect("dispatcher");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "https://example.test/page");
    assert!(hits[0].provenance.iter().any(|p| p == "engine:google"));
}
