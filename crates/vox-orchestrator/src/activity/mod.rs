//! Activity-log allowlist: which AgentEventKind variants persist to activity_log.

use crate::events::AgentEventKind;

pub mod project;

/// SSOT: true for high-signal lifecycle/resource/build events; false for
/// high-frequency telemetry (heartbeats, throughput/cost ticks, file-diag churn).
pub fn is_loggable(kind: &AgentEventKind) -> bool {
    use AgentEventKind::*;
    matches!(
        kind,
        AgentSpawned { .. }
            | AgentRetired { .. }
            | TaskSubmitted { .. }
            | TaskStarted { .. }
            | TaskPhaseChanged { .. }
            | TaskCompleted { .. }
            | TaskFailed { .. }
            | TaskReprioritized { .. }
            | TaskDelegated { .. }
            | PlanHandoff { .. }
            | CostIncurred { .. }
            | BudgetAlert { .. }
            | AttentionBudgetAlert { .. }
            | LockAcquired { .. }
            | LockReleased { .. }
            | ConflictDetected { .. }
            | BuildStage { .. }
            | MeshTopologyChanged { .. }
            | WorkflowStarted { .. }
            | WorkflowCompleted { .. }
            | WorkflowFailed { .. }
    )
    // High-frequency telemetry deliberately excluded:
    // AgentHeartbeat, ThroughputTick, CostTick, FileDiagChanged → false (fall-through).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AgentActivity, AgentEventKind};
    use crate::types::AgentId;

    #[test]
    fn lifecycle_is_logged_telemetry_is_not() {
        assert!(is_loggable(&AgentEventKind::AgentSpawned {
            agent_id: AgentId(1),
            name: "x".into()
        }));
        assert!(!is_loggable(&AgentEventKind::AgentHeartbeat {
            agent_id: AgentId(1),
            activity: AgentActivity::Thinking,
            active_skill: None
        }));
    }
}
