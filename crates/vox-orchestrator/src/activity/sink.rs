//! Drains the EventBus and persists loggable events to activity_log.
use crate::activity::{is_loggable, project::project};
use crate::events::AgentEvent;

/// Run the sink. `insert` persists one row; `max_events` bounds the loop for tests.
///
/// TASK-4.0
pub async fn run_sink<F, Fut>(
    mut rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    insert: F,
    max_events: Option<usize>,
) where
    F: Fn(crate::activity::project::ActivityRow, u64) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let mut seen = 0usize;
    while let Ok(ev) = rx.recv().await {
        if is_loggable(&ev.kind) {
            insert(project(&ev.kind), ev.timestamp_ms).await;
            seen += 1;
            if Some(seen) == max_events {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AgentActivity, AgentEventKind, EventBus};
    use crate::types::AgentId;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn sink_persists_only_loggable() {
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let rows = Arc::new(Mutex::new(Vec::new()));
        let sink = rows.clone();

        let h = tokio::spawn(run_sink(
            rx,
            move |r, _ts| {
                let sink = sink.clone();
                async move {
                    sink.lock().unwrap().push(r.kind);
                }
            },
            Some(1),
        ));

        // Dropped (not loggable)
        bus.emit(AgentEventKind::AgentHeartbeat {
            agent_id: AgentId(1),
            activity: AgentActivity::Thinking,
            active_skill: None,
        });

        // Logged
        bus.emit(AgentEventKind::AgentSpawned {
            agent_id: AgentId(1),
            name: "n".into(),
        });

        h.await.unwrap();
        assert_eq!(
            rows.lock().unwrap().as_slice(),
            &["AgentSpawned".to_string()]
        );
    }
}
