//! Reverse-proxy server that forwards incoming requests to an upstream app.
//!
//! Pass-through only in S1 (no auth). S5 adds auth middleware.

use crate::utils::share::auth::{AuthMode, auth_middleware};
use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, Uri};
use axum::middleware;
use axum::response::Response;
use axum::routing::any;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;

/// Configuration for the reverse-proxy server.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// Address of the upstream application to forward requests to.
    pub upstream_addr: SocketAddr,
    /// Local address to bind the proxy listener on.
    pub bind_addr: SocketAddr,
    /// Authentication mode for incoming requests.
    pub auth_mode: AuthMode,
}

#[derive(Clone)]
struct ProxyState {
    upstream_addr: SocketAddr,
    client: Client<HttpConnector, Body>,
}

/// Build the Axum [`Router`] for the reverse-proxy.
///
/// All requests are forwarded to `cfg.upstream_addr` with the same method,
/// headers, and body. The upstream response is returned verbatim.
/// Auth middleware (if configured) is applied before forwarding.
pub fn build_app(cfg: ProxyConfig) -> Router {
    let client = Client::builder(TokioExecutor::new()).build_http::<Body>();
    let state = ProxyState {
        upstream_addr: cfg.upstream_addr,
        client,
    };
    Router::new()
        .fallback(any(forward))
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            cfg.auth_mode,
            auth_middleware,
        ))
}

async fn forward(
    axum::extract::State(state): axum::extract::State<ProxyState>,
    mut req: Request,
) -> Result<Response, (StatusCode, String)> {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or("/");
    let target = format!("http://{}{}", state.upstream_addr, path_and_query);
    *req.uri_mut() = Uri::try_from(target)
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("bad upstream URI: {e}")))?;
    state
        .client
        .request(req)
        .await
        .map(|resp| resp.map(Body::new))
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")))
}
