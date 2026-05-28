//! Persisted plan DAG dispatch: enqueue every node whose dependencies are satisfied.
//!
//! Local orchestrator queue remains authoritative; `plan_nodes.status` tracks scheduling progress.

use crate::Orchestrator;
use crate::orchestrator::OrchestratorError;
use crate::planning::executor_bridge;
use crate::planning::{ExecutionPolicy, PlanNode, PlanStatus};
use crate::types::TaskId;

use vox_db::PlanNodeRow;

fn row_to_plan_node(row: &PlanNodeRow) -> Result<PlanNode, OrchestratorError> {
    let depends_on: Vec<String> = serde_json::from_str(&row.dependencies_json).map_err(|e| {
        OrchestratorError::DatabaseError(format!("plan node {} deps: {e}", row.node_id))
    })?;
    let execution_policy: ExecutionPolicy =
        serde_json::from_str(&row.execution_policy_json).unwrap_or_default();
    let status = match row.status.as_str() {
        "completed" => PlanStatus::Completed,
        "queued" => PlanStatus::Queued,
        "in_progress" => PlanStatus::InProgress,
        "failed" => PlanStatus::Failed,
        "cancelled" => PlanStatus::Cancelled,
        "superseded" => PlanStatus::Superseded,
        _ => PlanStatus::Pending,
    };
    Ok(PlanNode {
        node_id: row.node_id.clone(),
        description: row.description.clone(),
        depends_on,
        status,
        execution_policy,
        workflow_invocation: row.workflow_invocation.clone(),
    })
}

/// Enqueue orchestrator work for every `pending` plan node whose dependencies are already
/// `completed`. Marks each successfully enqueued node as `queued` in `plan_nodes`.
pub async fn enqueue_runnable_plan_nodes(
    orch: &Orchestrator,
    plan_session_id: &str,
    plan_version: u32,
    session_id: Option<String>,
    tenant_id: Option<String>,
) -> Result<Vec<TaskId>, OrchestratorError> {
    let Some(db) = orch.db() else {
        return Ok(vec![]);
    };
    let rows = db
        .load_plan_nodes_with_status(plan_session_id, plan_version as i64)
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

    let mut completed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for r in &rows {
        if r.status == "completed" {
            completed.insert(r.node_id.clone());
        }
    }

    let mut out: Vec<TaskId> = Vec::new();
    for r in &rows {
        if r.status != "pending" {
            continue;
        }
        let deps: Vec<String> = serde_json::from_str(&r.dependencies_json).unwrap_or_default();
        if !deps.iter().all(|d| completed.contains(d)) {
            continue;
        }
        let node = row_to_plan_node(r)?;
        let tid = executor_bridge::enqueue_plan_node(
            orch,
            &node,
            plan_session_id,
            plan_version,
            session_id.clone(),
            tenant_id.clone(),
        )
        .await?;
        db.set_plan_node_status(plan_session_id, plan_version as i64, &r.node_id, "queued")
            .await
            .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;
        out.push(tid);
    }
    Ok(out)
}
