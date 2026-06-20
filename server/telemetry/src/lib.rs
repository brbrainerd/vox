//! `vox-server` — OTLP/HTTP telemetry ingest → ClickHouse.
//!
//! # Architecture
//! ```text
//! Client (vox-cli / vox-gui)
//!   └─ POST /v1/logs  (JSON OTLP logs payload)
//!        │
//!        ▼
//!   ingest::handler
//!        │  1. parse OTLP JSON
//!        │  2. server-side allowlist filter (defense-in-depth, spec §3.3)
//!        │  3. batch insert → ClickHouse `events_raw`
//!        ▼
//!   ClickHouse
//!        └─ materialized views → per-category rollup tables
//! ```

pub mod auth;
pub mod ingest;
pub mod redact;
pub mod schema;

pub use schema::gen_ddl;
