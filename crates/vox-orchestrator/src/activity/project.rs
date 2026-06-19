//! Project an AgentEventKind into an activity_log row (summary + detail json).

use crate::events::AgentEventKind;

pub struct ActivityRow {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub kind: String,
    pub summary: String,
    pub detail_json: String,
}

pub fn project(kind: &AgentEventKind) -> ActivityRow {
    use AgentEventKind::*;
    let (agent_id, session_id, kind_str, summary) = match kind {
        AgentSpawned { agent_id, name } => (
            Some(agent_id.to_string()),
            None,
            "AgentSpawned",
            format!("Agent spawned: {name}"),
        ),
        AgentRetired { agent_id } => (
            Some(agent_id.to_string()),
            None,
            "AgentRetired",
            "Agent retired".to_string(),
        ),
        TaskSubmitted {
            task_id,
            agent_id,
            description,
            session_id,
        } => (
            Some(agent_id.to_string()),
            session_id.clone(),
            "TaskSubmitted",
            format!("Task #{task_id} submitted: {description}"),
        ),
        TaskStarted {
            task_id,
            agent_id,
            session_id,
        } => (
            Some(agent_id.to_string()),
            session_id.clone(),
            "TaskStarted",
            format!("Task #{task_id} started"),
        ),
        TaskPhaseChanged {
            task_id,
            agent_id,
            phase,
        } => (
            Some(agent_id.to_string()),
            None,
            "TaskPhaseChanged",
            format!("Task #{task_id} phase changed to {phase:?}"),
        ),
        TaskCompleted {
            task_id,
            agent_id,
            session_id,
            ..
        } => (
            Some(agent_id.to_string()),
            session_id.clone(),
            "TaskCompleted",
            format!("Task #{task_id} completed successfully"),
        ),
        TaskFailed {
            task_id,
            agent_id,
            error,
            session_id,
            ..
        } => (
            Some(agent_id.to_string()),
            session_id.clone(),
            "TaskFailed",
            format!("Task #{task_id} failed: {error}"),
        ),
        TaskReprioritized {
            task_id,
            old_priority,
            new_priority,
            session_id,
            ..
        } => (
            None,
            session_id.clone(),
            "TaskReprioritized",
            format!("Task #{task_id} reprioritized from {old_priority:?} to {new_priority:?}"),
        ),
        TaskDelegated {
            parent_agent_id,
            child_agent_id,
            task_id,
            reason,
        } => (
            Some(parent_agent_id.to_string()),
            None,
            "TaskDelegated",
            format!(
                "Task #{task_id} delegated from Agent #{parent_agent_id} to Agent #{child_agent_id} with reason: {reason}"
            ),
        ),
        PlanHandoff {
            from,
            to,
            session_id,
            ..
        } => (
            Some(from.to_string()),
            session_id.clone(),
            "PlanHandoff",
            format!("Plan handed off from Agent #{from} to Agent #{to}"),
        ),
        CostIncurred {
            agent_id,
            provider,
            model,
            cost_usd,
            ..
        } => (
            Some(agent_id.to_string()),
            None,
            "CostIncurred",
            format!("Cost incurred: ${cost_usd:.4} via {provider}/{model}"),
        ),
        BudgetAlert { agent_id, signal } => (
            Some(agent_id.to_string()),
            None,
            "BudgetAlert",
            format!("Budget alert for Agent #{agent_id}: {signal:?}"),
        ),
        AttentionBudgetAlert {
            agent_id,
            threshold,
            spent_ms,
            max_ms,
        } => (
            Some(agent_id.to_string()),
            None,
            "AttentionBudgetAlert",
            format!(
                "Attention budget alert: spent {spent_ms}ms / max {max_ms}ms (threshold {threshold})"
            ),
        ),
        LockAcquired {
            agent_id,
            path,
            exclusive,
        } => (
            Some(agent_id.to_string()),
            None,
            "LockAcquired",
            format!("Lock acquired on {path:?} (exclusive: {exclusive})"),
        ),
        LockReleased { agent_id, path } => (
            Some(agent_id.to_string()),
            None,
            "LockReleased",
            format!("Lock released on {path:?}"),
        ),
        ConflictDetected {
            path,
            agent_ids,
            conflict_id,
        } => (
            agent_ids.first().map(|id| id.to_string()),
            None,
            "ConflictDetected",
            format!("Conflict #{conflict_id} detected on {path:?}"),
        ),
        BuildStage { stage, .. } => (None, None, "BuildStage", format!("Build stage: {stage:?}")),
        MeshTopologyChanged {
            added_nodes,
            removed_nodes,
            changed_edges,
        } => (
            None,
            None,
            "MeshTopologyChanged",
            format!(
                "Mesh topology changed: added {}, removed {}, changed edges {}",
                added_nodes.len(),
                removed_nodes.len(),
                changed_edges
            ),
        ),
        WorkflowStarted { workflow_id, .. } => (
            None,
            None,
            "WorkflowStarted",
            format!("Workflow started: {workflow_id}"),
        ),
        WorkflowCompleted { workflow_id, .. } => (
            None,
            None,
            "WorkflowCompleted",
            format!("Workflow completed: {workflow_id}"),
        ),
        WorkflowFailed {
            workflow_id, error, ..
        } => (
            None,
            None,
            "WorkflowFailed",
            format!("Workflow {workflow_id} failed: {error}"),
        ),
        other => (None, None, "Other", format!("{other:?}")),
    };

    let detail_json = serde_json::to_string(kind).unwrap_or_else(|_| "{}".to_string());

    ActivityRow {
        agent_id,
        session_id,
        kind: kind_str.to_string(),
        summary,
        detail_json,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AgentEventKind;
    use crate::types::{AgentId, TaskId};

    #[test]
    fn task_completed_projects_summary() {
        let row = project(&AgentEventKind::TaskCompleted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            session_id: None,
            audit_report: None,
        });
        assert_eq!(row.kind, "TaskCompleted");
        assert!(row.summary.to_lowercase().contains("completed"));
    }

    #[test]
    fn cost_incurred_projects_with_real_fields() {
        let row = project(&AgentEventKind::CostIncurred {
            agent_id: AgentId(1),
            provider: "anthropic".into(),
            model: "claude-opus".into(),
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: 0.01,
            temporal_context: None,
        });
        assert_eq!(row.kind, "CostIncurred");
    }
}
