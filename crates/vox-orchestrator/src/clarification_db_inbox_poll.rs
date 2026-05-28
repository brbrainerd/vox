//! Poll SQL `a2a_messages` for clarification routing (`orchestrator_clarifier` / `mcp_chat`).

use std::sync::{Arc, Mutex as SyncMutex};

use tokio::task::JoinHandle;
use vox_db::VoxDb;

/// Background drain: route `clarification_request` → `clarification_response`; ingest `clarification_response` for `mcp_chat`.
pub fn spawn_clarification_db_inbox_poller(
    db: Arc<VoxDb>,
    repository_id: String,
    join_slot: Arc<SyncMutex<Option<JoinHandle<()>>>>,
) {
    let mut guard = join_slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = guard.take() {
        h.abort();
    }
    let db_clone = db.clone();
    let rid = repository_id;
    let handle = tokio::spawn(async move {
        let mut tick = tokio::time::interval(vox_config::timeouts::D_5S);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;

            if let Ok(messages) = db_clone
                .poll_a2a_inbox("orchestrator_clarifier", &rid)
                .await
            {
                for msg in messages {
                    if msg.msg_type != "clarification_request" {
                        continue;
                    }
                    let payload: serde_json::Value = serde_json::from_str(&msg.payload)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let hyp = payload
                        .get("hypothesis_set_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let resp_uuid = format!("clarify-resp-{}-{now_ms}", msg.message_uuid);
                    let body = serde_json::json!({
                        "hypothesis_set_id": hyp,
                        "routing_status": "accepted",
                        "source_message_uuid": msg.message_uuid,
                    });
                    let thread = msg.thread_id.clone();
                    if db_clone
                        .send_a2a_message(
                            &resp_uuid,
                            "orchestrator_clarifier",
                            "mcp_chat",
                            "clarification_response",
                            &body.to_string(),
                            msg.priority,
                            thread.as_deref(),
                            &rid,
                        )
                        .await
                        .is_ok()
                    {
                        let _ = db_clone.acknowledge_a2a_message_by_id(msg.id).await;
                    }
                }
            }

            let Ok(responses) = db_clone.poll_a2a_inbox("mcp_chat", &rid).await else {
                continue;
            };
            for msg in responses {
                if msg.msg_type != "clarification_response" {
                    continue;
                }
                let payload: serde_json::Value =
                    serde_json::from_str(&msg.payload).unwrap_or_else(|_| serde_json::json!({}));
                let session_key = msg.thread_id.as_deref().unwrap_or("unknown_session");
                let meta = serde_json::json!({
                    "kind": "a2a_clarification_response_delivered",
                    "hypothesis_set_id": payload.get("hypothesis_set_id"),
                    "routing_status": payload.get("routing_status"),
                    "source_message_uuid": payload.get("source_message_uuid"),
                });
                let _ = db_clone
                    .record_questioning_metric(session_key, None, &meta.to_string())
                    .await;
                let _ = db_clone.acknowledge_a2a_message_by_id(msg.id).await;
            }
        }
    });
    *guard = Some(handle);
}
