//! Populi mesh remote execution gating (single-owner leased task class).

use crate::config::OrchestratorConfig;
use crate::types::AgentTask;

/// Stable machine-readable placement reason codes for task routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementReasonCode {
    /// Normal local queue placement.
    LocalQueueDefault,
    /// Lease-gated remote path is active and local queue holds authority remotely.
    PopuliRemoteLeaseHold,
    /// Remote relay/lease path failed and task is requeued locally.
    LocalQueueFallbackAfterRemoteRelayError,
    /// No registered node meets the task's `min_vram_mb` requirement (W2 admission control).
    LocalQueueFallbackInsufficientVram,
}

impl PlacementReasonCode {
    /// Stable `snake_case` string used in logs and telemetry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalQueueDefault => "local_queue_default",
            Self::PopuliRemoteLeaseHold => "populi_remote_lease_hold",
            Self::LocalQueueFallbackAfterRemoteRelayError => {
                "local_queue_fallback_after_remote_relay_error"
            }
            Self::LocalQueueFallbackInsufficientVram => "local_queue_fallback_insufficient_vram",
        }
    }
}

/// Resolve a stable Populi node id used by orchestrator lease calls.
#[must_use]
pub fn lease_claimer_node_id(cfg: &OrchestratorConfig) -> String {
    let preferred = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
        .expose()
        .map(std::string::ToString::to_string)
        .or_else(|| cfg.populi_scope_id.clone())
        .unwrap_or_else(|| {
            let host = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "local".to_string());
            format!("orchestrator-{host}")
        });
    format!("orch-{preferred}")
}

/// True when this task must use awaited mesh relay + remote hold semantics (no concurrent local dequeue).
#[must_use]
pub fn task_matches_populi_remote_lease_gate(task: &AgentTask, cfg: &OrchestratorConfig) -> bool {
    cfg.populi_remote_lease_gating_enabled
        && cfg
            .populi_remote_lease_gated_roles
            .iter()
            .any(|r| task.execution_role == Some(*r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;
    use crate::reconstruction::AgentExecutionRole;
    use crate::types::{AgentTask, TaskEnqueueHints, TaskId, TaskPriority};

    #[test]
    fn lease_gate_matches_configured_roles_only() {
        let cfg = OrchestratorConfig {
            populi_remote_lease_gating_enabled: true,
            populi_remote_lease_gated_roles: vec![AgentExecutionRole::Builder],
            ..OrchestratorConfig::default()
        };
        let mut t = AgentTask::new(TaskId(1), "x", TaskPriority::Normal, vec![]);
        assert!(!task_matches_populi_remote_lease_gate(&t, &cfg));
        t.execution_role = Some(AgentExecutionRole::Builder);
        assert!(task_matches_populi_remote_lease_gate(&t, &cfg));
        t.execution_role = Some(AgentExecutionRole::Planner);
        assert!(!task_matches_populi_remote_lease_gate(&t, &cfg));
    }

    #[test]
    fn lease_gate_disabled_never_matches() {
        let cfg = OrchestratorConfig {
            populi_remote_lease_gating_enabled: false,
            populi_remote_lease_gated_roles: vec![AgentExecutionRole::Builder],
            ..OrchestratorConfig::default()
        };
        let mut t = AgentTask::new(TaskId(1), "x", TaskPriority::Normal, vec![]);
        t.execution_role = Some(AgentExecutionRole::Builder);
        assert!(!task_matches_populi_remote_lease_gate(&t, &cfg));
    }

    #[test]
    fn hints_merge_execution_role_for_gate() {
        let cfg = OrchestratorConfig {
            populi_remote_lease_gating_enabled: true,
            populi_remote_lease_gated_roles: vec![AgentExecutionRole::Verifier],
            ..OrchestratorConfig::default()
        };
        let mut t = AgentTask::new(TaskId(1), "x", TaskPriority::Normal, vec![]);
        let h = TaskEnqueueHints {
            execution_role: Some(AgentExecutionRole::Verifier),
            ..Default::default()
        };
        if let Some(r) = h.execution_role {
            t.execution_role = Some(r);
        }
        assert!(task_matches_populi_remote_lease_gate(&t, &cfg));
    }

    #[test]
    fn placement_reason_codes_are_stable() {
        assert_eq!(
            PlacementReasonCode::LocalQueueDefault.as_str(),
            "local_queue_default"
        );
        assert_eq!(
            PlacementReasonCode::PopuliRemoteLeaseHold.as_str(),
            "populi_remote_lease_hold"
        );
        assert_eq!(
            PlacementReasonCode::LocalQueueFallbackAfterRemoteRelayError.as_str(),
            "local_queue_fallback_after_remote_relay_error"
        );
    }
}
