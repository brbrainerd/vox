//! Axum HTTP router for the inbound webhook gateway.
//!
//! Exposes:
//! - POST `/webhooks/:source` — receive an inbound webhook
//! - GET  `/webhooks/health` — health check
//! - GET  `/webhooks/channels` — list registered channels

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use crate::{
    channel::ChannelManager,
    handler::{InboundPayload, WebhookHandler},
    WebhookError,
};

/// Shared state for the webhook router.
#[derive(Clone)]
pub struct WebhookState {
    pub handler: Arc<WebhookHandler>,
    pub channels: Arc<ChannelManager>,
    /// Sink for processed events (e.g. tokio broadcast channel)
    pub event_sink: Arc<tokio::sync::broadcast::Sender<crate::handler::WebhookEvent>>,
}

impl WebhookState {
    pub fn new(handler: WebhookHandler) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            handler: Arc::new(handler),
            channels: Arc::new(ChannelManager::new()),
            event_sink: Arc::new(tx),
        }
    }
}

/// Build the Axum `Router` for the webhook gateway.
pub fn build_router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhooks/health", get(health_check))
        .route("/webhooks/channels", get(list_channels))
        .route("/webhooks/:source", post(receive_webhook))
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
    channels: Vec<crate::channel::Channel>,
}

async fn list_channels(State(state): State<WebhookState>) -> Json<ChannelListResponse> {
    Json(ChannelListResponse {
        channels: state.channels.list(),
    })
}

#[derive(Serialize)]
struct WebhookResponse {
    event_id: String,
    accepted: bool,
}

#[derive(Deserialize)]
struct WebhookQuery {
    // optional event_type override from query param
}

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
        .or_else(|| headers.get("x-gitlab-event"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let signature = headers
        .get("x-hub-signature-256")
        .or_else(|| headers.get("x-vox-signature"))
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
        source: Arc::from(source.as_str()),
        event_type: Arc::from(event_type.as_str()),
        body: parsed_body,
        signature,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    match state.handler.handle(&payload) {
        Ok(event) => {
            info!(source, event_type, id = %event.id, "Webhook event accepted");
            let id = event.id.clone();
            let _ = state.event_sink.send(event);
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({ "event_id": id.as_ref(), "accepted": true })),
            )
        }
        Err(WebhookError::InvalidSignature) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid signature" })),
        ),
        Err(e) => {
            warn!(source, "Webhook rejected: {e}");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}
