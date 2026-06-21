//! OTLP/HTTP logs ingest handler (`POST /v1/logs`).
//!
//! Accepts the JSON OTLP logs format emitted by `vox-telemetry-otlp`'s uploader.
//! Server-side re-applies the taxonomy allowlist before inserting into ClickHouse.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use clickhouse::Client;
use serde::Deserialize;
use serde_json::Value;
use tracing::{info, warn};

use crate::redact::{build_allowlist, filter_record, FilteredRecord};
use crate::schema::load_taxonomy;

// ── OTLP JSON structures ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpLogsPayload {
    pub resource_logs: Vec<ResourceLogs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLogs {
    #[serde(default)]
    pub resource: Option<OtlpResource>,
    #[serde(default)]
    pub scope_logs: Vec<ScopeLogs>,
}

#[derive(Debug, Deserialize)]
pub struct OtlpResource {
    #[serde(default)]
    pub attributes: Vec<OtlpKV>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeLogs {
    #[serde(default)]
    pub log_records: Vec<LogRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    /// Nanoseconds since Unix epoch (string per OTLP JSON spec).
    #[serde(default)]
    pub time_unix_nano: Option<String>,
    #[serde(default)]
    pub attributes: Vec<OtlpKV>,
}

#[derive(Debug, Deserialize)]
pub struct OtlpKV {
    pub key: String,
    pub value: OtlpValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub int_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bool_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_value: Option<f64>,
}

impl From<OtlpValue> for Value {
    fn from(v: OtlpValue) -> Self {
        if let Some(s) = v.string_value {
            Value::String(s)
        } else if let Some(i) = v.int_value {
            Value::Number(i.into())
        } else if let Some(b) = v.bool_value {
            Value::Bool(b)
        } else if let Some(d) = v.double_value {
            serde_json::Number::from_f64(d)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        }
    }
}

// ── Server state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub ch: Arc<Client>,
    pub allowlist: Arc<HashMap<String, std::collections::HashSet<String>>>,
}

impl AppState {
    pub fn new(ch: Client) -> anyhow::Result<Self> {
        let taxonomy = load_taxonomy().map_err(|e| anyhow::anyhow!("taxonomy parse: {e}"))?;
        let allowlist = build_allowlist(&taxonomy);
        Ok(Self {
            ch: Arc::new(ch),
            allowlist: Arc::new(allowlist),
        })
    }
}

// ── Handler ───────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct IngestResponse {
    accepted: usize,
    discarded: usize,
}

pub async fn ingest_logs(
    State(state): State<AppState>,
    Json(payload): Json<OtlpLogsPayload>,
) -> impl IntoResponse {
    let mut accepted = 0usize;
    let mut discarded = 0usize;
    let mut records: Vec<FilteredRecord> = Vec::new();

    for rl in &payload.resource_logs {
        // Extract install_id from resource attributes.
        let install_id = rl
            .resource
            .as_ref()
            .and_then(|r| {
                r.attributes.iter().find(|kv| kv.key == "install_id").and_then(|kv| {
                    kv.value.string_value.as_deref().map(str::to_string)
                })
            })
            .unwrap_or_default();

        for sl in &rl.scope_logs {
            for lr in &sl.log_records {
                // Parse timestamp (nanos → millis).
                let ts_ms = lr
                    .time_unix_nano
                    .as_deref()
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|ns| ns / 1_000_000)
                    .unwrap_or(0);

                // Build raw attribute map.
                let mut raw: HashMap<String, Value> = HashMap::new();
                let mut event_name = String::new();
                for kv in &lr.attributes {
                    if kv.key == "event_type" {
                        // client sends event_type; map to otlp_event_name via taxonomy lookup
                        // For now we forward the event_name field as-is and let filter_record handle it.
                        event_name = kv
                            .value
                            .string_value
                            .clone()
                            .unwrap_or_default();
                    } else {
                        raw.insert(kv.key.clone(), kv.value.clone().into());
                    }
                }

                // event_name may also be set as "event_name" attribute.
                if event_name.is_empty() {
                    event_name = raw
                        .remove("event_name")
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_default();
                }

                if event_name.is_empty() {
                    warn!("log record has no event_name — discarding");
                    discarded += 1;
                    continue;
                }

                match filter_record(&install_id, &event_name, ts_ms, raw, &state.allowlist) {
                    Some(rec) => {
                        records.push(rec);
                        accepted += 1;
                    }
                    None => {
                        warn!(event_name = %event_name, "unknown category — discarding");
                        discarded += 1;
                    }
                }
            }
        }
    }

    // Batch insert into ClickHouse.
    if !records.is_empty() {
        if let Err(e) = insert_batch(&state.ch, &records).await {
            warn!("ClickHouse insert failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }

    info!(accepted, discarded, "ingest batch complete");
    (StatusCode::OK, Json(serde_json::json!({"accepted": accepted, "discarded": discarded})))
        .into_response()
}

// ── ClickHouse insert ─────────────────────────────────────────────────────────

async fn insert_batch(ch: &Client, records: &[FilteredRecord]) -> anyhow::Result<()> {
    let mut insert = ch.insert("events_raw")?;
    for rec in records {
        let row = EventRow::from_record(rec);
        insert.write(&row).await?;
    }
    insert.end().await?;
    Ok(())
}

/// Flat row for ClickHouse insert — all optional except the primary key columns.
#[derive(Debug, clickhouse::Row, serde::Serialize)]
struct EventRow {
    install_id: String,
    event_name: String,
    ts: i64,
    // Common attribute fields — absent fields are NULL in ClickHouse.
    verb: Option<String>,
    exit_class: Option<String>,
    duration_bucket: Option<String>,
    skill_id_hash: Option<String>,
    trigger_source: Option<String>,
    accepted: Option<u8>,
    surface: Option<String>,
    op_type: Option<String>,
    file_kind: Option<String>,
    size_bucket: Option<String>,
    tool_call_kind: Option<String>,
    mode: Option<String>,
    error_class: Option<String>,
    subsystem: Option<String>,
    recoverable: Option<u8>,
    decision_id: Option<String>,
    chosen: Option<String>,
    outcome: Option<String>,
    magnitude_bucket: Option<i64>,
}

impl EventRow {
    fn from_record(r: &FilteredRecord) -> Self {
        let s = |k: &str| -> Option<String> {
            r.attrs.get(k).and_then(|v| v.as_str().map(String::from))
        };
        let b = |k: &str| -> Option<u8> {
            r.attrs.get(k).and_then(|v| v.as_bool()).map(|b| b as u8)
        };
        let i = |k: &str| -> Option<i64> {
            r.attrs.get(k).and_then(|v| v.as_i64())
        };
        Self {
            install_id: r.install_id.clone(),
            event_name: r.event_name.clone(),
            ts: r.ts_ms,
            verb: s("verb"),
            exit_class: s("exit_class"),
            duration_bucket: s("duration_bucket"),
            skill_id_hash: s("skill_id_hash"),
            trigger_source: s("trigger_source"),
            accepted: b("accepted"),
            surface: s("surface"),
            op_type: s("op_type"),
            file_kind: s("file_kind"),
            size_bucket: s("size_bucket"),
            tool_call_kind: s("tool_call_kind"),
            mode: s("mode"),
            error_class: s("error_class"),
            subsystem: s("subsystem"),
            recoverable: b("recoverable"),
            decision_id: s("decision_id"),
            chosen: s("chosen"),
            outcome: s("outcome"),
            magnitude_bucket: i("magnitude_bucket"),
        }
    }
}
