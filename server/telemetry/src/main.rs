use std::net::SocketAddr;

use axum::{Router, routing::get, routing::post};
use clickhouse::Client;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use vox_server::ingest::{AppState, ingest_logs};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (local dev only).
    let _ = dotenvy::dotenv();

    // Tracing — default to INFO, structured JSON in production.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer())
        .init();

    let ch_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://localhost:8123".to_string());
    let ch_db = std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "vox_telemetry".to_string());
    let ch_user = std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string());
    let ch_pass = std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default();

    let ch = Client::default()
        .with_url(&ch_url)
        .with_database(&ch_db)
        .with_user(&ch_user)
        .with_password(&ch_pass);

    // Apply the schema idempotently on boot (every statement is CREATE … IF NOT
    // EXISTS), so a fresh Coolify volume is migrated without a separate one-shot
    // job. Clone first — AppState::new consumes the client.
    let ch_for_migrate = ch.clone();
    vox_server::schema::ensure_schema(&ch_for_migrate).await?;

    let state = AppState::new(ch)?;

    // Write-only anti-abuse key (see auth.rs). Absent ⇒ local-dev, gate is open.
    let ingest_token = vox_server::auth::IngestToken(
        std::env::var("VOX_TELEMETRY_INGEST_TOKEN")
            .ok()
            .filter(|s| !s.is_empty()),
    );

    let addr: SocketAddr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:4318".to_string())
        .parse()?;

    let app = Router::new()
        .route("/v1/logs", post(ingest_logs))
        // Bearer gate on /v1/logs ONLY; /healthz (added below) stays open.
        .route_layer(axum::middleware::from_fn_with_state(
            ingest_token,
            vox_server::auth::require_bearer,
        ))
        .route("/healthz", get(healthz))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!(addr = %addr, "vox-server starting");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}
