//! Fire-and-forget capture of one tool call into `agent_operations`. Redaction and
//! the DB write happen on a spawned task, so the dispatch path is never blocked;
//! every error is swallowed (capture is best-effort and must not affect results).

use std::sync::Arc;
use vox_db::VoxDb;

const MAX_FIELD: usize = 8 * 1024;

fn cap(mut s: String) -> String {
    if s.len() > MAX_FIELD {
        // Back up to a UTF-8 char boundary — `truncate` panics mid-codepoint,
        // and tool args/results routinely contain multibyte text (paths, CJK, emoji).
        let mut end = MAX_FIELD;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push_str("…[truncated]");
    }
    s
}

/// Spawn a best-effort capture. No-op when disabled or when there is no DB.
#[allow(clippy::too_many_arguments)]
pub fn spawn_capture(
    db: Option<Arc<VoxDb>>,
    enabled: bool,
    tool_name: String,
    args: serde_json::Value,
    result: String,
    session_id: Option<String>,
    agent_id: Option<String>,
    duration_ms: i64,
    is_error: bool,
) {
    if !enabled {
        return;
    }
    let Some(db) = db else {
        return;
    };
    tokio::spawn(async move {
        let args_redacted = cap(vox_redact::redact_args(&args).to_string());
        let result_redacted = cap(vox_redact::redact_owned(&result));
        match db
            .record_operation(
                session_id.as_deref(),
                agent_id.as_deref(),
                &tool_name,
                &args_redacted,
                Some(result_redacted.as_str()),
                duration_ms,
                is_error,
            )
            .await
        {
            // ponytail: prune on a 1-in-500 cadence — the row-count trim runs a
            // subquery, so don't pay it on every tool call.
            Ok(rowid) => {
                if rowid % 500 == 0 {
                    let _ = db.prune_operations().await;
                }
            }
            Err(e) => tracing::debug!(error = %e, "operation capture failed (ignored)"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_truncates_oversized() {
        let big = "x".repeat(MAX_FIELD + 100);
        let out = cap(big);
        assert!(out.len() <= MAX_FIELD + "…[truncated]".len());
        assert!(out.ends_with("[truncated]"));
    }

    #[test]
    fn cap_does_not_panic_on_multibyte_boundary() {
        // 3-byte chars: byte 8192 lands mid-codepoint → naive truncate would panic.
        let big = "あ".repeat(MAX_FIELD); // 3 bytes each, well over MAX_FIELD bytes
        let out = cap(big); // must not panic
        assert!(out.ends_with("[truncated]"));
        // The truncated prefix is still valid UTF-8 (String guarantees it).
        assert!(out.len() <= MAX_FIELD + "…[truncated]".len());
    }

    #[tokio::test]
    async fn disabled_is_noop() {
        // enabled=false returns immediately without touching the (None) db.
        spawn_capture(
            None,
            false,
            "t".into(),
            serde_json::json!({}),
            "r".into(),
            None,
            None,
            1,
            false,
        );
    }
}
