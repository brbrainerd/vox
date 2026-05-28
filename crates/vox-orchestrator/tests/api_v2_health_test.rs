use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
use vox_orchestrator::services::routes;

#[tokio::test]
#[ignore = "pending routes migration from vox-orchestrator-mcp (Phase 4 reorg) — owner: orchestrator sunset: 2026-12-31"]
async fn api_v2_health_returns_envelope() {
    let app = routes::router();
    let req = Request::builder()
        .uri("/api/v2/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["v"], 1);
    assert_eq!(body["data"]["status"], "ok");
}

#[test]
#[ignore = "pending routes migration from vox-orchestrator-mcp (Phase 4 reorg) — owner: orchestrator sunset: 2026-12-31"]
fn ok_page_envelope_includes_cursor() {
    use vox_orchestrator::services::routes::ok_page;
    let resp = ok_page(
        serde_json::json!([{"id": "a"}, {"id": "b"}]),
        Some("cur-xyz"),
    );
    let body = resp.0;
    assert_eq!(body["v"], 1);
    assert_eq!(body["data"][0]["id"], "a");
    assert_eq!(body["cursor"], "cur-xyz");

    let resp2 = ok_page::<Vec<serde_json::Value>>(vec![], None);
    let body2 = resp2.0;
    assert!(body2["cursor"].is_null());
}
