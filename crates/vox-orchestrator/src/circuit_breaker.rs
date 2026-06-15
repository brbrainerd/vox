//! Five-signal circuit breaker for orchestrator doom-loop detection (D6).
//!
//! Reads thresholds from `CircuitBreakerConfig` which mirrors
//! `contracts/orchestration/circuit-breaker.v1.yaml`.
//! All trip / tier checks are pure: no async, no I/O, no allocations on the hot path.
//! The `bigram_jaccard` helper is the lone exception — it allocates two small
//! `HashSet`s per call and is called at most once per loop iteration (see its docs).

use serde::{Deserialize, Serialize};

/// Shipped circuit-breaker contract, compiled into the binary (path relative to
/// THIS file → repo-root `contracts/`). Embedded = always live, never inert.
const EMBEDDED_CIRCUIT_BREAKER_YAML: &str =
    include_str!("../../../contracts/orchestration/circuit-breaker.v1.yaml");

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

impl CircuitBreakerConfig {
    /// Parse thresholds from contract YAML text, overlaying onto `Default`.
    /// Unspecified keys retain their default. SSOT: `contracts/orchestration/circuit-breaker.v1.yaml`.
    pub fn from_contract_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        // Overlay field names track the contract YAML keys, not CircuitBreakerConfig field names.
        #[derive(serde::Deserialize, Default)]
        struct TripThresholds {
            no_progress_loops: Option<u32>,
            same_error_loops: Option<u32>,
            tool_thrash_count: Option<u32>,
            ngram_overlap: Option<f64>,
            semantic_drift_sigma: Option<f64>,
        }
        #[derive(serde::Deserialize, Default)]
        struct AlarmLevel {
            no_progress_loops: Option<u32>,
            same_error_loops: Option<u32>,
            tool_thrash_count: Option<u32>,
        }
        #[derive(serde::Deserialize, Default)]
        struct GraduatedAlarms {
            caution: Option<AlarmLevel>,
            warning: Option<AlarmLevel>,
        }
        #[derive(serde::Deserialize, Default)]
        struct Overlay {
            trip_thresholds: Option<TripThresholds>,
            graduated_alarms: Option<GraduatedAlarms>,
            replan_limit: Option<u32>,
        }
        let o: Overlay = if yaml.trim().is_empty() {
            Overlay::default()
        } else {
            serde_yaml::from_str(yaml)?
        };
        let mut c = Self::default();
        if let Some(t) = o.trip_thresholds {
            if let Some(v) = t.no_progress_loops {
                c.no_progress_threshold = v;
            }
            if let Some(v) = t.same_error_loops {
                c.same_error_threshold = v;
            }
            if let Some(v) = t.tool_thrash_count {
                c.tool_thrash_threshold = v;
            }
            if let Some(v) = t.ngram_overlap {
                c.ngram_overlap_threshold = v;
            }
            if let Some(v) = t.semantic_drift_sigma {
                c.semantic_drift_sigma = v;
            }
        }
        if let Some(g) = o.graduated_alarms {
            if let Some(caution) = g.caution {
                if let Some(v) = caution.no_progress_loops {
                    c.caution_no_progress = v;
                }
                if let Some(v) = caution.same_error_loops {
                    c.caution_same_error = v;
                }
                if let Some(v) = caution.tool_thrash_count {
                    c.caution_tool_thrash = v;
                }
            }
            if let Some(warning) = g.warning {
                if let Some(v) = warning.no_progress_loops {
                    c.warning_no_progress = v;
                }
                if let Some(v) = warning.same_error_loops {
                    c.warning_same_error = v;
                }
                if let Some(v) = warning.tool_thrash_count {
                    c.warning_tool_thrash = v;
                }
            }
        }
        if let Some(v) = o.replan_limit {
            c.replan_limit = v;
        }
        Ok(c)
    }

    /// Parse the compile-time-embedded contract.
    pub fn embedded() -> Self {
        Self::from_contract_str(EMBEDDED_CIRCUIT_BREAKER_YAML)
            .expect("embedded circuit-breaker.v1.yaml must parse")
    }

    /// Resolve the live config: explicit override file (env) wins when present and
    /// parseable; otherwise the embedded contract. Never silently inert.
    pub fn resolve() -> Self {
        if let Ok(p) = std::env::var("VOX_CIRCUIT_BREAKER_CONTRACT") {
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = Self::from_contract_str(&text) {
                        return cfg;
                    }
                    tracing::warn!(path = %path.display(),
                        "circuit-breaker override failed to parse; using embedded contract");
                }
            }
        }
        Self::embedded()
    }

    /// Load thresholds from the contract file if it exists; otherwise `Default`.
    pub fn from_contract_file(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_contract_str(&text).unwrap_or_else(|e| {
                tracing::warn!(target: "orch.circuit_breaker", error = %e, path = %path.display(), "circuit-breaker contract parse failed; using defaults");
                Self::default()
            }),
            Err(e) => {
                tracing::debug!(target: "orch.circuit_breaker", error = %e, path = %path.display(), "circuit-breaker contract not read; using defaults");
                Self::default()
            }
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
    fn embedded_contract_is_canonical_and_matches_default() {
        let embedded = CircuitBreakerConfig::embedded();
        let def = CircuitBreakerConfig::default();
        assert_eq!(embedded.no_progress_threshold, def.no_progress_threshold);
        assert_eq!(embedded.tool_thrash_threshold, def.tool_thrash_threshold);
        assert_eq!(embedded.replan_limit, def.replan_limit);
        assert_eq!(
            embedded.ngram_overlap_threshold,
            def.ngram_overlap_threshold
        );
    }

    #[test]
    fn from_contract_str_overrides_defaults() {
        let yaml = r#"
trip_thresholds:
  no_progress_loops: 7
  same_error_loops: 9
graduated_alarms:
  warning:
    tool_thrash_count: 20
"#;
        let cfg = CircuitBreakerConfig::from_contract_str(yaml).expect("parse");
        assert_eq!(cfg.no_progress_threshold, 7);
        assert_eq!(cfg.same_error_threshold, 9);
        assert_eq!(cfg.warning_tool_thrash, 20);
        // unspecified keys keep their default:
        assert_eq!(cfg.tool_thrash_threshold, 15);
        assert_eq!(cfg.replan_limit, 3);
    }

    #[test]
    fn real_contract_parses_and_stays_default_compatible() {
        // Drift guard: the shipped contract must (1) parse under the nested schema
        // and (2) stay Default-compatible. This does NOT by itself prove the schema
        // is consumed (from_contract_file falls back to Default on any error, and the
        // contract mirrors Default) — that binding proof is in
        // `from_contract_str_overrides_defaults`. This test catches the contract
        // drifting away from Default or becoming unparseable.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/orchestration/circuit-breaker.v1.yaml");
        let text = std::fs::read_to_string(&path).expect("contract file must exist");
        // The real contract text must parse under the nested overlay schema.
        let parsed = CircuitBreakerConfig::from_contract_str(&text)
            .expect("shipped contract must parse under the nested schema");
        let def = CircuitBreakerConfig::default();
        assert_eq!(parsed.no_progress_threshold, def.no_progress_threshold);
        assert_eq!(parsed.same_error_threshold, def.same_error_threshold);
        assert_eq!(parsed.tool_thrash_threshold, def.tool_thrash_threshold);
        assert_eq!(parsed.ngram_overlap_threshold, def.ngram_overlap_threshold);
        assert_eq!(parsed.semantic_drift_sigma, def.semantic_drift_sigma);
        assert_eq!(parsed.caution_no_progress, def.caution_no_progress);
        assert_eq!(parsed.caution_same_error, def.caution_same_error);
        assert_eq!(parsed.caution_tool_thrash, def.caution_tool_thrash);
        assert_eq!(parsed.warning_no_progress, def.warning_no_progress);
        assert_eq!(parsed.warning_same_error, def.warning_same_error);
        assert_eq!(parsed.warning_tool_thrash, def.warning_tool_thrash);
        assert_eq!(parsed.replan_limit, def.replan_limit);
    }

    #[test]
    fn real_contract_value_change_flows_through() {
        // Strong consumption proof: take the REAL shipped contract, mutate one value
        // in its text, and confirm the change reaches CircuitBreakerConfig. If the
        // overlay silently ignored the contract (the original flat-schema bug), the
        // parsed value would stay at the default 3 and the assert_ne!/assert_eq! below
        // would fail. `no_progress_loops: 3` is unique to `trip_thresholds` in the
        // shipped contract (caution=1, warning=2), so the replace targets exactly it.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/orchestration/circuit-breaker.v1.yaml");
        let text = std::fs::read_to_string(&path).expect("contract file must exist");
        assert!(
            text.contains("no_progress_loops: 3"),
            "contract shape changed; update this test's mutation target"
        );
        let mutated = text.replace("no_progress_loops: 3", "no_progress_loops: 11");
        let parsed = CircuitBreakerConfig::from_contract_str(&mutated)
            .expect("mutated contract must still parse");
        let def = CircuitBreakerConfig::default();
        // The mutated trip threshold flows through...
        assert_eq!(parsed.no_progress_threshold, 11);
        assert_ne!(parsed.no_progress_threshold, def.no_progress_threshold);
        // ...while an untouched sibling key keeps its contract (== default) value.
        assert_eq!(parsed.tool_thrash_threshold, 15);
        // ...and the graduated-alarm siblings (value 1/2, not 3) are unaffected.
        assert_eq!(parsed.caution_no_progress, def.caution_no_progress);
        assert_eq!(parsed.warning_no_progress, def.warning_no_progress);
    }

    #[test]
    fn from_contract_str_empty_is_all_defaults() {
        let cfg = CircuitBreakerConfig::from_contract_str("").expect("parse");
        assert_eq!(cfg.no_progress_threshold, 3);
        assert_eq!(cfg.ngram_overlap_threshold, 0.85);
    }

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
