//! Append-only JSONL telemetry for training and serving.
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/telemetry.rs` (SP3 sub-batch C).

use std::io::Write;
use std::path::Path;

pub fn append(out_dir: &Path, event: &str, payload: serde_json::Value) -> anyhow::Result<()> {
    let p = out_dir.join("telemetry.jsonl");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)?;
    let line = serde_json::json!({
        "ts_ms": chrono::Utc::now().timestamp_millis(),
        "event": event,
        "payload": payload,
    });
    writeln!(f, "{}", serde_json::to_string(&line)?)?;
    f.flush()?;
    Ok(())
}
