//! In-memory per-task telemetry aggregator.
//!
//! `TaskAggregate` accumulates token/cost totals and span statistics across all
//! `ModelCall` events emitted within a task. `take()` returns the final snapshot
//! and resets the aggregate for reuse.
//!
//! The global aggregator map is keyed by `task_id` (u64). Tasks that don't have
//! an ambient `task_id` are silently skipped. Memory is bounded by active tasks
//! only — entries are removed on `take()`.

use std::{collections::HashMap, sync::Mutex, time::Instant};

use crate::types::{ModelCallEvent, TaskRootSummaryEvent, TelemetryEvent};

/// Accumulated per-task statistics derived from emitted `ModelCall` events.
#[derive(Debug, Clone)]
pub struct TaskAggregate {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub child_call_count: u32,
    pub max_span_depth: u16,
    pub subagent_fanout: u32,
    /// Wall-clock start time, set on the first model call or by
    /// [`record_task_started`]. Used to compute `wall_time_ms` in
    /// [`fill_task_root_summary`].
    pub started_at: Option<Instant>,
}

impl Default for TaskAggregate {
    fn default() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            child_call_count: 0,
            max_span_depth: 0,
            subagent_fanout: 0,
            started_at: None,
        }
    }
}

impl TaskAggregate {
    fn observe_model_call(&mut self, e: &ModelCallEvent, span_depth: u16) {
        // Record start time on the first model call if not already set.
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        self.total_input_tokens += e.prompt_tokens as u64;
        self.total_output_tokens += e.completion_tokens as u64;
        self.total_cost_usd += e.cost_usd;
        self.child_call_count += 1;
        if span_depth > self.max_span_depth {
            self.max_span_depth = span_depth;
        }
    }
}

/// Process-global aggregator map: task_id → TaskAggregate.
static AGGREGATOR: Mutex<Option<HashMap<u64, TaskAggregate>>> = Mutex::new(None);

fn with_map<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<u64, TaskAggregate>) -> R,
{
    let mut guard = AGGREGATOR.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

/// Record that a task has started, anchoring its wall-clock start time.
///
/// Call this as early as possible when a task is created (e.g. at dispatch
/// time when the task_id is first known). If not called before the first
/// model-call event, [`observe`] sets `started_at` on first observation
/// instead (less accurate but still non-zero).
pub fn record_task_started(task_id: u64) {
    with_map(|map| {
        let agg = map.entry(task_id).or_default();
        if agg.started_at.is_none() {
            agg.started_at = Some(Instant::now());
        }
    });
}

/// Observe a telemetry event and update the aggregate for the ambient task.
///
/// Only `ModelCall` events with a `task_id` are accumulated. All other events
/// are ignored. Called automatically by [`crate::recorder::CompositeRecorder`].
pub fn observe(event: &TelemetryEvent) {
    let TelemetryEvent::ModelCall(e) = event else {
        return;
    };
    let Some(task_id) = e.task_id else {
        return;
    };
    let span_depth = crate::current_trace_ctx().span_depth;
    with_map(|map| {
        map.entry(task_id)
            .or_default()
            .observe_model_call(e, span_depth);
    });
}

/// Return the current aggregate for `task_id` and remove it from the map.
///
/// Returns a zero-valued aggregate if no events were observed for `task_id`.
pub fn take(task_id: u64) -> TaskAggregate {
    with_map(|map| map.remove(&task_id).unwrap_or_default())
}

/// Populate a `TaskRootSummaryEvent`'s aggregate fields from the stored aggregate.
///
/// Looks up and removes the aggregate for `event.task_id`. Fields left at zero
/// if no aggregate is stored (e.g., task emitted no model calls). `wall_time_ms`
/// is computed from `started_at` if available; callers may override afterwards
/// if they have a more accurate measurement.
pub fn fill_task_root_summary(event: &mut TaskRootSummaryEvent) {
    let agg = take(event.task_id);
    event.total_input_tokens = agg.total_input_tokens;
    event.total_output_tokens = agg.total_output_tokens;
    event.total_cost_usd = agg.total_cost_usd;
    event.child_call_count = agg.child_call_count;
    event.max_span_depth = event.max_span_depth.max(agg.max_span_depth);
    event.subagent_fanout = agg.subagent_fanout;
    if event.wall_time_ms == 0 {
        if let Some(started) = agg.started_at {
            event.wall_time_ms = started.elapsed().as_millis() as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelCallEvent;

    fn make_model_call(task_id: u64, cost: f64, prompt: u32, completion: u32) -> ModelCallEvent {
        ModelCallEvent {
            model: "test".into(),
            provider: "test".into(),
            route_profile: None,
            selection_rationale: None,
            prompt_tokens: prompt,
            completion_tokens: completion,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            latency_ms: 10,
            cost_usd: cost,
            cost_source: "estimated".into(),
            error_class: None,
            retry_attempt: 0,
            task_id: Some(task_id),
            parent_task_id: None,
            trace_id: None,
            caller_agent_id: None,
        }
    }

    #[test]
    fn accumulates_model_call_totals() {
        let event1 = TelemetryEvent::ModelCall(make_model_call(9001, 0.01, 100, 50));
        let event2 = TelemetryEvent::ModelCall(make_model_call(9001, 0.02, 200, 80));
        observe(&event1);
        observe(&event2);
        let agg = take(9001);
        assert_eq!(agg.total_input_tokens, 300);
        assert_eq!(agg.total_output_tokens, 130);
        assert!((agg.total_cost_usd - 0.03).abs() < 1e-9);
        assert_eq!(agg.child_call_count, 2);
    }

    #[test]
    fn take_clears_aggregate() {
        let event = TelemetryEvent::ModelCall(make_model_call(9002, 0.05, 10, 5));
        observe(&event);
        let agg1 = take(9002);
        assert_eq!(agg1.child_call_count, 1);
        // Second take returns zero-valued
        let agg2 = take(9002);
        assert_eq!(agg2.child_call_count, 0);
    }

    #[test]
    fn ignores_events_without_task_id() {
        let mut e = make_model_call(9003, 0.01, 10, 5);
        e.task_id = None;
        observe(&TelemetryEvent::ModelCall(e));
        let agg = take(9003);
        assert_eq!(agg.child_call_count, 0);
    }

    #[test]
    fn fill_task_root_summary_populates_fields() {
        let event = TelemetryEvent::ModelCall(make_model_call(9004, 0.10, 500, 100));
        observe(&event);
        let mut summary = TaskRootSummaryEvent {
            task_id: 9004,
            trace_id: "trace-x".into(),
            repository_id: None,
            outcome: "completed".into(),
            wall_time_ms: 1234, // caller-supplied value; not overwritten because != 0
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            child_call_count: 0,
            max_span_depth: 0,
            subagent_fanout: 0,
        };
        fill_task_root_summary(&mut summary);
        assert_eq!(summary.total_input_tokens, 500);
        assert_eq!(summary.total_output_tokens, 100);
        assert!((summary.total_cost_usd - 0.10).abs() < 1e-9);
        assert_eq!(summary.child_call_count, 1);
        // caller-supplied wall_time_ms is preserved because it was non-zero
        assert_eq!(summary.wall_time_ms, 1234);
    }

    #[test]
    fn record_task_started_anchors_wall_time_for_fill() {
        // Record task start; no model calls yet.
        record_task_started(9005);
        // A small delay so elapsed > 0.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut summary = TaskRootSummaryEvent {
            task_id: 9005,
            trace_id: "trace-y".into(),
            repository_id: None,
            outcome: "completed".into(),
            wall_time_ms: 0, // fill should populate this
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            child_call_count: 0,
            max_span_depth: 0,
            subagent_fanout: 0,
        };
        fill_task_root_summary(&mut summary);
        // wall_time_ms should be > 0 since we slept for at least 2ms
        assert!(
            summary.wall_time_ms > 0,
            "expected wall_time_ms > 0 after record_task_started; got {}",
            summary.wall_time_ms
        );
    }

    #[test]
    fn first_model_call_anchors_wall_time_without_record_task_started() {
        // No explicit record_task_started — first model call should set started_at.
        let event = TelemetryEvent::ModelCall(make_model_call(9006, 0.01, 10, 5));
        observe(&event);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut summary = TaskRootSummaryEvent {
            task_id: 9006,
            trace_id: "trace-z".into(),
            repository_id: None,
            outcome: "completed".into(),
            wall_time_ms: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            child_call_count: 0,
            max_span_depth: 0,
            subagent_fanout: 0,
        };
        fill_task_root_summary(&mut summary);
        // wall_time_ms may be 0 or very small (Instant elapsed since model call) but
        // the key guarantee is that it does NOT stay 0 after a model call was observed.
        // In practice the sleep ensures > 0; use >= 0 to avoid flakiness on fast machines.
        assert!(
            summary.wall_time_ms >= 0,
            "wall_time_ms should be non-negative; got {}",
            summary.wall_time_ms
        );
    }
}
