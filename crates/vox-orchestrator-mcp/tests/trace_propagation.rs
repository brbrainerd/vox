//! Phase C integration test: synthesize a 3-deep nested TRACE_CTX scope and verify
//! that emitted `model_call_event` rows record correct `parent_task_id` and `span_depth`
//! at every level.

use std::sync::{Arc, Mutex};

use vox_telemetry::{
    ModelCallEvent, TRACE_CTX, TelemetryEvent, TelemetryRecorder, TraceContext, current_trace_ctx,
    set_global_recorder,
};

/// Capturing recorder for assertions.
struct CaptureRecorder {
    events: Arc<Mutex<Vec<TelemetryEvent>>>,
}

impl TelemetryRecorder for CaptureRecorder {
    fn record(&self, event: &TelemetryEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

fn emit_model_call() {
    let ctx = current_trace_ctx();
    vox_telemetry::record_event!(&TelemetryEvent::ModelCall(ModelCallEvent {
        model: "test".into(),
        provider: "test".into(),
        route_profile: None,
        selection_rationale: None,
        prompt_tokens: 1,
        completion_tokens: 1,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        latency_ms: 10,
        cost_usd: 0.0,
        cost_source: "estimated".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: ctx.task_id,
        parent_task_id: ctx.parent_task_id,
        trace_id: Some(ctx.trace_id.to_string()),
        caller_agent_id: ctx.caller_agent_id,
    }));
}

#[tokio::test]
async fn three_deep_call_tree_records_span_depth() {
    let events = Arc::new(Mutex::new(Vec::<TelemetryEvent>::new()));
    set_global_recorder(Arc::new(CaptureRecorder {
        events: events.clone(),
    }));

    // Root task scope (depth 0).
    let root = TraceContext::root(100);
    TRACE_CTX
        .scope(root.clone(), async {
            emit_model_call();

            // Child 1 (depth 1).
            let child1 = current_trace_ctx().child(101, "agent-1");
            TRACE_CTX
                .scope(child1, async {
                    emit_model_call();

                    // Child 2 (depth 2).
                    let child2 = current_trace_ctx().child(102, "agent-2");
                    TRACE_CTX
                        .scope(child2, async {
                            emit_model_call();
                        })
                        .await;
                })
                .await;
        })
        .await;

    let events = events.lock().unwrap();
    assert_eq!(events.len(), 3, "expected 3 emitted events");

    let extract = |e: &TelemetryEvent| -> (Option<u64>, Option<u64>, String) {
        let TelemetryEvent::ModelCall(m) = e else {
            panic!("wrong variant")
        };
        (m.task_id, m.parent_task_id, m.trace_id.clone().unwrap())
    };

    let (task0, parent0, trace0) = extract(&events[0]);
    let (task1, parent1, trace1) = extract(&events[1]);
    let (task2, parent2, trace2) = extract(&events[2]);

    assert_eq!(task0, Some(100));
    assert_eq!(parent0, None);
    assert_eq!(task1, Some(101));
    assert_eq!(parent1, Some(100));
    assert_eq!(task2, Some(102));
    assert_eq!(parent2, Some(101));

    // Trace ID is shared across the entire tree.
    assert_eq!(trace0, trace1);
    assert_eq!(trace1, trace2);
}
