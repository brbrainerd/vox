use crate::redact::RedactedRecord;

/// Async OTLP/HTTP uploader (feature `remote` only).
///
/// Reads pending clean spool records and POSTs them to the central OTLP endpoint
/// when `is_remote_allowed()` returns true. Best-effort: errors are logged at
/// DEBUG level and never propagate to the caller.
pub async fn upload_pending_otlp(
    spool_dir: &std::path::Path,
    endpoint: &str,
) -> std::io::Result<usize> {
    if !vox_telemetry::config::is_remote_allowed() {
        return Ok(0);
    }
    // TODO(track-B5): implement list_pending → POST OTLP/HTTP → ack.
    // Placeholder returns 0 until B5 wires the full upload path.
    let _ = (spool_dir, endpoint);
    Ok(0)
}

/// A single pending record read from the spool, ready for upload.
#[allow(dead_code)]
pub struct PendingRecord {
    pub id: String,
    pub record: RedactedRecord,
}
