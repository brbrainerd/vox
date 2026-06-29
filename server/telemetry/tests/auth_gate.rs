//! Bearer-gate tests for `POST /v1/logs`. `/healthz` must stay open.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // oneshot
use vox_server::auth::{require_bearer, IngestToken};

fn app(token: Option<&str>) -> axum::Router {
    axum::Router::new()
        .route("/v1/logs", axum::routing::post(|| async { "accepted" }))
        .route_layer(axum::middleware::from_fn_with_state(
            IngestToken(token.map(str::to_string)),
            require_bearer,
        ))
        .route("/healthz", axum::routing::get(|| async { "ok" }))
}

#[tokio::test]
async fn rejects_missing_bearer_when_token_set() {
    let res = app(Some("secret"))
        .oneshot(Request::post("/v1/logs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_wrong_bearer() {
    let res = app(Some("secret"))
        .oneshot(
            Request::post("/v1/logs")
                .header("authorization", "Bearer nope") // drift-allow(bearer-header-inline): test fixture, not a real token
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_correct_bearer() {
    let res = app(Some("secret"))
        .oneshot(
            Request::post("/v1/logs")
                .header("authorization", "Bearer secret") // drift-allow(bearer-header-inline): test fixture, not a real token
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn healthz_is_open_even_with_token_set() {
    let res = app(Some("secret"))
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn no_token_configured_allows_all_for_local_dev() {
    let res = app(None)
        .oneshot(Request::post("/v1/logs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
