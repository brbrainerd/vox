//! Axum HTTP router for the inbound webhook gateway.
//!
//! Exposes:
//! - POST `/webhooks/:source` — receive an inbound webhook
//! - GET  `/webhooks/health` — health check
//! - GET  `/webhooks/channels` — list registered channels

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
};
use serde::Serialize;
use serde_json::Value;
use tracing::{info, warn};

use super::{
    WebhookError,
    channel::ChannelManager,
    handler::{InboundPayload, WebhookHandler},
};

/// Shared state for the webhook router.
#[derive(Clone)]
pub struct WebhookState {
    pub handler: Arc<WebhookHandler>,
    pub channels: Arc<ChannelManager>,
    /// Sink for processed events (e.g. tokio broadcast channel)
    pub event_sink: Arc<tokio::sync::broadcast::Sender<super::handler::WebhookEvent>>,
    /// Optional bearer token for inbound request authentication.
    /// Resolved from Clavis `WebhookIngressToken` at startup.
    /// When `None`, auth is skipped with a warning (degraded mode).
    pub ingress_token: Option<Arc<str>>,
}

impl WebhookState {
    pub fn new(handler: WebhookHandler) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            handler: Arc::new(handler),
            channels: Arc::new(ChannelManager::new()),
            event_sink: Arc::new(tx),
            ingress_token: None,
        }
    }

    /// Set the bearer token for ingress authentication.
    pub fn with_ingress_token(mut self, token: impl Into<String>) -> Self {
        self.ingress_token = Some(Arc::from(token.into().as_str()));
        self
    }
}

/// Build the Axum `Router` for the webhook gateway.
///
/// When `WebhookState.ingress_token` is set, all routes except `/webhooks/health`
/// require an `Authorization: Bearer <token>` header.
pub fn build_router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhooks/health", get(health_check))
        .route("/webhooks/channels", get(list_channels))
        .route("/webhooks/{source}", post(receive_webhook))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            bearer_auth_middleware,
        ))
        .with_state(state)
}

/// Start the webhook server on the given bind address.
pub async fn serve(state: WebhookState, addr: &str) -> Result<(), WebhookError> {
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(WebhookError::Io)?;
    info!(addr, "Webhook gateway listening");
    axum::serve(listener, router)
        .await
        .map_err(|e| WebhookError::Http(e.to_string()))
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Bearer token authentication middleware.
///
/// - `/webhooks/health` is always passed through (no auth required).
/// - If `WebhookState.ingress_token` is `None`, the request is passed through
///   with a one-time `WARN` log (degraded mode — set `VOX_WEBHOOK_INGRESS_TOKEN`).
/// - Otherwise, the `Authorization: Bearer <token>` header must match exactly.
async fn bearer_auth_middleware(
    State(state): State<WebhookState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    use std::sync::OnceLock;
    // Skip auth on the liveness probe.
    if request.uri().path() == "/webhooks/health" {
        return next.run(request).await;
    }

    match &state.ingress_token {
        None => {
            // Degraded mode — warn once per process lifetime.
            static DID_WARN: OnceLock<()> = OnceLock::new();
            DID_WARN.get_or_init(|| {
                warn!(
                    "Webhook server is operating WITHOUT an ingress token. \
                     Set VOX_WEBHOOK_INGRESS_TOKEN to require bearer auth."
                );
            });
            next.run(request).await
        }
        Some(expected_token) => {
            let provided = request
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix(vox_http_client::BEARER_PREFIX));

            if provided == Some(expected_token.as_ref()) {
                next.run(request).await
            } else {
                warn!("Webhook ingress: rejected request with missing/invalid bearer token");
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "invalid or missing bearer token" })),
                )
                    .into_response()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Serialize)]
struct ChannelListResponse {
    channels: Vec<super::channel::Channel>,
}

async fn list_channels(State(state): State<WebhookState>) -> Json<ChannelListResponse> {
    Json(ChannelListResponse {
        channels: state.channels.list(),
    })
}

// `WebhookResponse` / `WebhookQuery` placeholders were removed (unused).
// `receive_webhook` returns a `serde_json::Value` directly until the response
// shape is needed; reintroduce typed structs at the same time as their fields.

async fn receive_webhook(
    State(state): State<WebhookState>,
    Path(source): Path<String>,
    headers: HeaderMap,
    body: String,
) -> (StatusCode, Json<serde_json::Value>) {
    // Extract event type from headers (X-Vox-Event, or X-GitHub-Event, etc.)
    let event_type = headers
        .get("x-vox-event")
        .or_else(|| headers.get("x-github-event"))
        // DEPRECATED (2026-06-03): GitLab is no longer supported; the
        // `x-gitlab-event` header is still read for backward compatibility only.
        .or_else(|| headers.get("x-gitlab-event"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Discord, Slack, and generic fallbacks.
    let signature = headers
        .get("x-signature-ed25519")
        .or_else(|| headers.get("x-slack-signature"))
        .or_else(|| headers.get("x-hub-signature-256"))
        .or_else(|| headers.get("x-vox-signature"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let req_timestamp = headers
        .get("x-signature-timestamp")
        .or_else(|| headers.get("x-slack-request-timestamp"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let parsed_body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            warn!(source, "Failed to parse webhook body as JSON: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid JSON body" })),
            );
        }
    };

    let payload = InboundPayload {
        source: source.clone(),
        event_type: event_type.clone(),
        body: parsed_body,
        signature,
        timestamp: req_timestamp,
    };

    match state.handler.handle(&payload) {
        Ok(event) => {
            info!(source, event_type, id = %event.id, "Webhook event accepted");
            let id = event.id.clone();
            let _ = state.event_sink.send(event);
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({ "event_id": id.as_str(), "accepted": true })),
            )
        }
        Err(WebhookError::InvalidSignature) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid signature" })),
        ),
        Err(WebhookError::MissingTimestamp) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "missing timestamp" })),
        ),
        Err(WebhookError::TimestampOutOfWindow(ts)) => {
            warn!(
                source,
                ts, "Webhook rejected: timestamp outside replay window"
            );
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "timestamp outside replay window" })),
            )
        }
        Err(e) => {
            warn!(source, "Webhook rejected: {e}");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}
