//! Qdrant REST search response parsing (`qdrant-vector` feature).

#[cfg(feature = "qdrant-vector")]
use vox_search::vector_qdrant::QdrantSemanticClient;
#[cfg(feature = "qdrant-vector")]
use wiremock::matchers::{method, path};
#[cfg(feature = "qdrant-vector")]
use wiremock::{Mock, MockServer, ResponseTemplate};

#[cfg(feature = "qdrant-vector")]
#[tokio::test]
async fn qdrant_search_vectors_parses_payload_snippet() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/collections/demo/points/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": [{
                "id": "p1",
                "score": 0.91,
                "payload": { "text": "hello world example" }
            }]
        })))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let client = QdrantSemanticClient::new(base, "demo");
    let vector = vec![0.1_f32, 0.2, 0.3];
    let out = client
        .search_vectors(&vector, 5, None, None)
        .await
        .expect("search_vectors");

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, "p1");
    assert!((out[0].1 - 0.91_f32).abs() < 1e-4);
    assert!(out[0].2.as_deref().unwrap_or("").contains("hello"));
}

#[cfg(not(feature = "qdrant-vector"))]
#[test]
fn qdrant_wiremock_placeholder() {
    // Keeps the integration test target compiling when default features omit Qdrant.
}
