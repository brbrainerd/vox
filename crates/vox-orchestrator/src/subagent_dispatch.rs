//! Sub-agent dispatch router: spawn vs. inline decision (D4).
//!
//! Decides whether to spawn a child sub-agent, run inline, or reject a dispatch
//! request based on task complexity, agent chain depth, and budget state.
//! All logic is pure: no async, no I/O.

use serde::{Deserialize, Serialize};

/// Decision made by the dispatch router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchDecision {
    /// Spawn a new child agent for this subtask.
    Spawn,
    /// Execute the subtask inline in the current agent context.
    Inline,
    /// Reject: chain depth exceeded the safety limit.
    Reject,
}

impl std::fmt::Display for DispatchDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn => write!(f, "spawn"),
            Self::Inline => write!(f, "inline"),
            Self::Reject => write!(f, "reject"),
        }
    }
}

/// Signals consumed by the dispatch router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchSignal {
    /// Task complexity 0–10.
    pub complexity: u8,
    /// Current spawn-chain depth (0 = root agent).
    pub chain_depth: u32,
    /// True when session budget is exhausted — avoids spawning expensive child.
    pub budget_exhausted: bool,
    /// True when the parent agent holds a file lock that would be inherited.
    pub parent_lock_held: bool,
}

impl Default for DispatchSignal {
    fn default() -> Self {
        Self {
            complexity: 5,
            chain_depth: 0,
            budget_exhausted: false,
            parent_lock_held: false,
        }
    }
}

/// Thresholds loaded from contract YAML. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchConfig {
    /// Complexity ≥ this → prefer Spawn (if not overridden).
    pub spawn_complexity_threshold: u8,
    /// Chain depth ≥ this → Reject (hard safety limit).
    pub max_chain_depth: u32,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            spawn_complexity_threshold: 6,
            max_chain_depth: 5,
        }
    }
}

/// Pure dispatch router.
pub struct DispatchRouter {
    config: DispatchConfig,
}

impl DispatchRouter {
    pub fn new(config: DispatchConfig) -> Self {
        Self { config }
    }

    /// Route a dispatch request to Spawn, Inline, or Reject.
    ///
    /// Resolution order:
    /// 1. `chain_depth >= max_chain_depth` → Reject.
    /// 2. `budget_exhausted || parent_lock_held` → Inline.
    /// 3. `complexity >= spawn_complexity_threshold` → Spawn.
    /// 4. Default → Inline.
    #[must_use]
    #[inline]
    pub fn route(&self, signal: &DispatchSignal) -> DispatchDecision {
        if signal.chain_depth >= self.config.max_chain_depth {
            return DispatchDecision::Reject;
        }
        if signal.budget_exhausted || signal.parent_lock_held {
            return DispatchDecision::Inline;
        }
        if signal.complexity >= self.config.spawn_complexity_threshold {
            DispatchDecision::Spawn
        } else {
            DispatchDecision::Inline
        }
    }

    /// Like [`Self::route`], plus emits [`vox_telemetry::TelemetryEvent::AiFixture`] subagent telemetry.
    #[must_use]
    pub fn route_with_telemetry(
        &self,
        signal: &DispatchSignal,
        session_id: Option<String>,
    ) -> DispatchDecision {
        let decision = self.route(signal);
        let trace_ctx = vox_telemetry::current_trace_ctx();
        let payload = vox_telemetry::SubagentDispatchTelemetryPayload {
            metric_type: vox_telemetry::METRIC_TYPE_SUBAGENT_DISPATCH.to_string(),
            decision: decision.to_string(),
            complexity: Some(u32::from(signal.complexity)),
            chain_depth: Some(signal.chain_depth),
            session_id,
            parent_task_id: trace_ctx.parent_task_id,
            span_depth: Some(u32::from(trace_ctx.span_depth)),
            dispatch_latency_ms: None,
        };
        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::AiFixture(
            vox_telemetry::AiFixtureEvent::SubagentDispatch(payload)
        ));
        decision
    }
}

/// Metric payload emitted when a sub-agent is dispatched.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct SubAgentDispatchEvent {
    pub metric_type: &'static str,
    pub decision: String,
    pub complexity: u8,
    pub chain_depth: u32,
    pub session_id: Option<String>,
    /// Phase C: parent task id from the ambient TraceContext.
    pub parent_task_id: Option<u64>,
    /// Phase C: span depth from the ambient TraceContext.
    pub span_depth: u16,
    /// Phase C: wall time from dispatch start to decision emit (ms).
    pub dispatch_latency_ms: Option<u64>,
}

impl SubAgentDispatchEvent {
    pub fn new(
        decision: DispatchDecision,
        signal: &DispatchSignal,
        session_id: Option<String>,
    ) -> Self {
        let trace_ctx = vox_telemetry::current_trace_ctx();
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_SUBAGENT_DISPATCH,
            decision: decision.to_string(),
            complexity: signal.complexity,
            chain_depth: signal.chain_depth,
            session_id,
            parent_task_id: trace_ctx.parent_task_id,
            span_depth: trace_ctx.span_depth,
            dispatch_latency_ms: None, // populated by callers that measure dispatch latency
        }
    }
}

/// Metric payload emitted when chain depth hits the safety limit.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct ChainDepthAlertEvent {
    pub metric_type: &'static str,
    pub current_depth: u32,
    pub max_depth: u32,
    pub session_id: Option<String>,
}

impl ChainDepthAlertEvent {
    pub fn new(current_depth: u32, max_depth: u32, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_CHAIN_DEPTH_ALERT,
            current_depth,
            max_depth,
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router() -> DispatchRouter {
        DispatchRouter::new(DispatchConfig::default())
    }

    #[test]
    fn low_complexity_uses_inline() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 3,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Inline);
    }

    #[test]
    fn high_complexity_spawns() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 8,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Spawn);
    }

    #[test]
    fn at_spawn_complexity_threshold_spawns() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 6,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Spawn);
    }

    #[test]
    fn chain_depth_at_limit_rejects() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 8,
            chain_depth: 5,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Reject);
    }

    #[test]
    fn chain_depth_above_limit_rejects() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 8,
            chain_depth: 10,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Reject);
    }

    #[test]
    fn budget_exhausted_forces_inline() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 9,
            budget_exhausted: true,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Inline);
    }

    #[test]
    fn parent_lock_forces_inline() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 9,
            parent_lock_held: true,
            ..Default::default()
        };
        assert_eq!(r.route(&sig), DispatchDecision::Inline);
    }

    #[test]
    fn reject_takes_priority_over_inline_constraints() {
        let r = router();
        let sig = DispatchSignal {
            complexity: 9,
            chain_depth: 5,
            budget_exhausted: true,
            parent_lock_held: true,
        };
        assert_eq!(r.route(&sig), DispatchDecision::Reject);
    }

    #[test]
    fn dispatch_event_has_correct_metric_type() {
        let sig = DispatchSignal::default();
        let ev = SubAgentDispatchEvent::new(DispatchDecision::Inline, &sig, None);
        assert_eq!(ev.metric_type, "orch.subagent.dispatch");
    }

    #[test]
    fn chain_depth_alert_event_has_correct_metric_type() {
        let ev = ChainDepthAlertEvent::new(5, 5, None);
        assert_eq!(ev.metric_type, "orch.subagent.chain_depth_alert");
    }
}
