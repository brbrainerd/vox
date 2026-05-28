//! Five-signal circuit breaker for orchestrator doom-loop detection (D6).
//!
//! Reads thresholds from `CircuitBreakerConfig` which mirrors
//! `contracts/orchestration/circuit-breaker.v1.yaml`.
//! All trip / tier checks are pure: no async, no I/O, no allocations on the hot path.
//! The `bigram_jaccard` helper is the lone exception — it allocates two small
//! `HashSet`s per call and is called at most once per loop iteration (see its docs).

use serde::{Deserialize, Serialize};

/// Reason the circuit was tripped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TripReason {
    NoProgress,
    SameError,
    ToolThrash,
    NgramOverlap,
    SemanticDrift,
}

impl std::fmt::Display for TripReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProgress => write!(f, "no-progress"),
            Self::SameError => write!(f, "same-error"),
            Self::ToolThrash => write!(f, "tool-thrash"),
            Self::NgramOverlap => write!(f, "ngram-overlap"),
            Self::SemanticDrift => write!(f, "semantic-drift"),
        }
    }
}

/// Graduated alarm tier (below trip threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlarmTier {
    None,
    Caution,
    Warning,
}

/// Running counters for the breaker; update after each loop iteration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    /// Consecutive plan loops with no new tool results.
    pub no_progress_loops: u32,
    /// Consecutive loops returning the same error class.
    pub same_error_loops: u32,
    /// Total redundant tool calls in this plan cycle.
    pub tool_thrash_count: u32,
    /// Jaccard similarity of current action bigrams vs prior bigrams (0.0–1.0).
    pub ngram_overlap: f64,
    /// Z-score of current embedding vs session baseline.
    pub semantic_drift_sigma: f64,
    /// How many replan attempts have occurred after a trip.
    pub replan_attempts: u32,
}

/// Thresholds loaded from contract YAML. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub no_progress_threshold: u32,
    pub same_error_threshold: u32,
    pub tool_thrash_threshold: u32,
    pub ngram_overlap_threshold: f64,
    pub semantic_drift_sigma: f64,
    pub caution_no_progress: u32,
    pub caution_same_error: u32,
    pub caution_tool_thrash: u32,
    pub warning_no_progress: u32,
    pub warning_same_error: u32,
    pub warning_tool_thrash: u32,
    pub replan_limit: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            no_progress_threshold: 3,
            same_error_threshold: 5,
            tool_thrash_threshold: 15,
            ngram_overlap_threshold: 0.85,
            semantic_drift_sigma: 2.0,
            caution_no_progress: 1,
            caution_same_error: 2,
            caution_tool_thrash: 8,
            warning_no_progress: 2,
            warning_same_error: 3,
            warning_tool_thrash: 12,
            replan_limit: 3,
        }
    }
}

/// Pure, allocation-free circuit breaker.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self { config }
    }

    /// Returns `Some(reason)` if the breaker should trip, `None` otherwise.
    #[must_use]
    #[inline]
    pub fn should_trip(&self, state: &CircuitBreakerState) -> Option<TripReason> {
        if state.no_progress_loops >= self.config.no_progress_threshold {
            return Some(TripReason::NoProgress);
        }
        if state.same_error_loops >= self.config.same_error_threshold {
            return Some(TripReason::SameError);
        }
        if state.tool_thrash_count >= self.config.tool_thrash_threshold {
            return Some(TripReason::ToolThrash);
        }
        if state.ngram_overlap >= self.config.ngram_overlap_threshold {
            return Some(TripReason::NgramOverlap);
        }
        if state.semantic_drift_sigma >= self.config.semantic_drift_sigma {
            return Some(TripReason::SemanticDrift);
        }
        None
    }

    /// Returns the current alarm tier without tripping.
    #[must_use]
    #[inline]
    pub fn check_tier(&self, state: &CircuitBreakerState) -> AlarmTier {
        if state.no_progress_loops >= self.config.warning_no_progress
            || state.same_error_loops >= self.config.warning_same_error
            || state.tool_thrash_count >= self.config.warning_tool_thrash
        {
            return AlarmTier::Warning;
        }
        if state.no_progress_loops >= self.config.caution_no_progress
            || state.same_error_loops >= self.config.caution_same_error
            || state.tool_thrash_count >= self.config.caution_tool_thrash
        {
            return AlarmTier::Caution;
        }
        AlarmTier::None
    }

    /// Returns true if replanning should escalate to HITL (replan limit exceeded).
    #[must_use]
    #[inline]
    pub fn should_escalate(&self, state: &CircuitBreakerState) -> bool {
        state.replan_attempts >= self.config.replan_limit
    }
}

/// Compute bigram Jaccard similarity between two action sequences.
/// Used for the n-gram overlap signal. O(n) time, O(n) space — allocates two small
/// `HashSet<(&str, &str)>`s per call. Intended to run at most once per loop iteration
/// where the allocation cost is negligible against the surrounding LLM round-trip.
#[must_use]
pub fn bigram_jaccard(a: &[&str], b: &[&str]) -> f64 {
    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }
    use std::collections::HashSet;
    let bigrams_a: HashSet<(&str, &str)> = a.windows(2).map(|w| (w[0], w[1])).collect();
    let bigrams_b: HashSet<(&str, &str)> = b.windows(2).map(|w| (w[0], w[1])).collect();
    let intersection = bigrams_a.intersection(&bigrams_b).count();
    let union = bigrams_a.union(&bigrams_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Metric payload emitted to `llm_interactions` when the breaker trips.
/// Serialize-only: `Deserialize` is incompatible with `&'static str` lifetimes
/// and isn't needed — these events are written, never read back.
#[derive(Debug, Clone, Serialize)]
pub struct TripEvent {
    pub metric_type: &'static str,
    pub trip_reason: String,
    pub replan_attempts: u32,
    pub session_id: Option<String>,
}

impl TripEvent {
    pub fn new(reason: TripReason, state: &CircuitBreakerState) -> Self {
        let trace_ctx = vox_telemetry::current_trace_ctx();
        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
            vox_telemetry::ErrorEvent {
                subsystem: "orch.circuit_breaker".into(),
                error_class: reason.to_string(),
                http_status: None,
                retry_attempt: state.replan_attempts,
                retried: false,
                model: None,
                provider: None,
                task_id: trace_ctx.task_id,
                trace_id: Some(trace_ctx.trace_id.to_string()),
            }
        ));
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CIRCUIT_BREAKER_TRIP,
            trip_reason: reason.to_string(),
            replan_attempts: state.replan_attempts,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trip_when_all_signals_zero() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState::default();
        assert!(cb.should_trip(&state).is_none());
    }

    #[test]
    fn trips_on_no_progress_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 3,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::NoProgress));
    }

    #[test]
    fn caution_tier_at_one_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 1,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Caution);
    }

    #[test]
    fn warning_tier_at_two_no_progress() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            no_progress_loops: 2,
            ..Default::default()
        };
        assert_eq!(cb.check_tier(&state), AlarmTier::Warning);
    }

    #[test]
    fn trips_on_same_error_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            same_error_loops: 5,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::SameError));
    }

    #[test]
    fn trips_on_tool_thrash_at_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            tool_thrash_count: 15,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::ToolThrash));
    }

    #[test]
    fn trips_on_ngram_overlap() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            ngram_overlap: 0.90,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::NgramOverlap));
    }

    #[test]
    fn trips_on_semantic_drift() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            semantic_drift_sigma: 2.5,
            ..Default::default()
        };
        assert_eq!(cb.should_trip(&state), Some(TripReason::SemanticDrift));
    }

    #[test]
    fn no_escalation_below_replan_limit() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            replan_attempts: 2,
            ..Default::default()
        };
        assert!(!cb.should_escalate(&state));
    }

    #[test]
    fn escalates_at_replan_limit() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let state = CircuitBreakerState {
            replan_attempts: 3,
            ..Default::default()
        };
        assert!(cb.should_escalate(&state));
    }

    #[test]
    fn bigram_jaccard_identical_sequences() {
        let a = vec!["read_file", "write_file", "run_test"];
        let b = vec!["read_file", "write_file", "run_test"];
        assert!((bigram_jaccard(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bigram_jaccard_disjoint_sequences() {
        let a = vec!["read_file", "write_file"];
        let b = vec!["run_test", "commit"];
        assert!(bigram_jaccard(&a, &b) < 1e-9);
    }

    #[test]
    fn bigram_jaccard_empty_inputs() {
        assert!((bigram_jaccard(&[], &[])).abs() < 1e-9);
    }

    #[test]
    fn trip_event_has_correct_metric_type() {
        let state = CircuitBreakerState::default();
        let event = TripEvent::new(TripReason::NoProgress, &state);
        assert_eq!(event.metric_type, "orch.circuit_breaker.trip");
    }
}
