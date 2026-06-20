use crate::config::TelemetryUploadConfig;
use crate::redact::RedactedRecord;

/// Async OTLP/HTTP uploader (feature `remote` only).
///
/// Reads pending clean spool records and POSTs them to the configured OTLP
/// endpoint (default `https://telemetry.voxlang.org/v1/logs`) with the bearer
/// ingest token, when `is_remote_allowed()` returns true. Best-effort: errors
/// are logged at DEBUG level and never propagate to the caller.
pub async fn upload_pending_otlp(
    spool_dir: &std::path::Path,
    cfg: &TelemetryUploadConfig,
) -> std::io::Result<usize> {
    if !vox_telemetry::config::is_remote_allowed() {
        return Ok(0);
    }
    // TODO(track-B5): implement list_pending → POST cfg.endpoint with
    // cfg.authorization_header() → ack. The target + auth are now resolved here;
    // B5 wires the reqwest POST loop.
    let _endpoint = cfg.endpoint.as_str();
    let _auth = cfg.authorization_header();
    let _ = spool_dir;
    Ok(0)
}

/// A single pending record read from the spool, ready for upload.
#[allow(dead_code)]
pub struct PendingRecord {
    pub id: String,
    pub record: RedactedRecord,
}
