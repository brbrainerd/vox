use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
pub struct ToolTraceRecord {
    pub tool: String,
    pub args_json: serde_json::Value,
    pub result_preview: Option<String>,
    pub session_id: Option<String>,
    pub timestamp_ms: u64,
    pub success: bool,
    pub latency_ms: u64,
}

/// Append one tool trace record as a JSONL line. Non-blocking; errors are swallowed.
pub async fn append_tool_trace(path: &Path, record: &ToolTraceRecord) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let line = serde_json::to_string(record)? + "\n";
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    f.write_all(line.as_bytes()).await?;
    Ok(())
}

/// Helper function to strip sensitive fields. Matches `privacy_scrub_args`.
pub fn strip_telemetry_fields(args: &serde_json::Value) -> serde_json::Value {
    if let Some(obj) = args.as_object() {
        let mut new_obj = obj.clone();
        // Remove known sensitive/noisy fields from args
        new_obj.remove("api_key");
        new_obj.remove("token");
        new_obj.remove("password");
        new_obj.remove("secret");
        serde_json::Value::Object(new_obj)
    } else {
        args.clone()
    }
}

/// Convert a tool trace record into a ChatML training pair for the agents lane.
/// Returns None for failed or system-internal calls.
pub fn tool_trace_to_chatml(rec: &ToolTraceRecord) -> Option<String> {
    if !rec.success {
        return None;
    }

    let cleaned_args = strip_telemetry_fields(&rec.args_json);
    let user_turn = format!("Call tool `{}` with: {}", rec.tool, cleaned_args);
    let assistant_turn = rec.result_preview.as_deref().unwrap_or("[ok]");
    Some(
        serde_json::json!({
            "messages": [
                {"role": "user", "content": user_turn},
                {"role": "assistant", "content": assistant_turn}
            ],
            "category": "tool_trace",
            "quality": 3,
            "tool": rec.tool,
            "latency_ms": rec.latency_ms,
        })
        .to_string(),
    )
}

/// Append a generic JSON object to a file.
pub async fn append_json(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let line = serde_json::to_string(value)? + "\n";
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    f.write_all(line.as_bytes()).await?;
    Ok(())
}
