//! Bearer-token gate for `POST /v1/logs`.
//!
//! SECURITY NOTE: this token is a **write-only anti-abuse key** (Sentry-DSN
//! model), NOT a confidentiality boundary. The client ships it, so it is
//! extractable by design; its only jobs are coarse abuse-blocking and per-IP
//! rate limiting. Privacy is enforced upstream (client-side redaction) and
//! server-side (the taxonomy allowlist re-applied per ingest in `ingest.rs`).
//! `/healthz` is routed OUTSIDE this layer and stays open.

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};

// drift-allow(bearer-header-inline): standalone workspace, cannot import vox-http-client; BEARER_SCHEME is the scheme prefix, not an API token
const BEARER_SCHEME: &str = "Bearer ";

/// The expected ingest token. `None` ⇒ no token configured ⇒ local-dev mode,
/// all requests pass. Cloned into the axum router state.
#[derive(Clone)]
pub struct IngestToken(pub Option<String>);

/// Middleware: require `Authorization: Bearer <token>` matching `IngestToken`.
/// Apply with `axum::middleware::from_fn_with_state(token, require_bearer)` as a
/// `route_layer` over the protected routes only (not `/healthz`).
pub async fn require_bearer(
    State(expected): State<IngestToken>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = expected.0.as_deref() else {
        return Ok(next.run(req).await); // dev: no token configured
    };
    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix(BEARER_SCHEME));
    match provided {
        Some(t) if constant_time_eq(t.as_bytes(), expected.as_bytes()) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Length-checked, then constant-time byte compare. The token is not a
/// confidentiality boundary, but a constant-time compare costs nothing and
/// avoids a trivial timing oracle.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
