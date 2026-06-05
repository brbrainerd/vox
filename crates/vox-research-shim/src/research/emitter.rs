use tokio::sync::broadcast;
use vox_research_events::{ResearchEvent, ResearchEventEmitter};

#[derive(Clone)]
pub struct BroadcastEmitter {
    sender: broadcast::Sender<ResearchEvent>,
}

impl BroadcastEmitter {
    #[must_use]
    pub fn new(sender: broadcast::Sender<ResearchEvent>) -> Self {
        Self { sender }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ResearchEvent> {
        self.sender.subscribe()
    }
}

impl ResearchEventEmitter for BroadcastEmitter {
    fn emit(&self, event: ResearchEvent) {
        let _ = self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_emitter_sends_events() {
        let (sender, mut rx) = broadcast::channel(4);
        let emitter = BroadcastEmitter::new(sender);
        emitter.emit(ResearchEvent::TelemetryObservation {
            provider: "test".to_string(),
            metric_type: "latency_ms".to_string(),
            value: 1.0,
            session_id: "s".to_string(),
            recorded_at_ms: 1,
        });
        assert!(rx.try_recv().is_ok());
    }
}
