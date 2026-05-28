use super::{Orchestrator, OrchestratorError};
use crate::planning::PlanningTaskMeta;
use crate::types::{FileAffinity, TaskEnqueueHints, TaskId, TaskPriority};

impl Orchestrator {
    /// Bridge planner workflow handoff decisions back into orchestrator execution.
    ///
    /// Current implementation schedules a workflow-tagged task and records handoff telemetry.
    pub async fn submit_workflow_handoff_goal(
        &self,
        goal: String,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        session_id: Option<String>,
        enqueue_hints: Option<TaskEnqueueHints>,
        tenant_id: Option<String>,
    ) -> Result<TaskId, OrchestratorError> {
        let plan_session_id = format!("wf-{}", uuid::Uuid::new_v4());
        let meta = PlanningTaskMeta {
            plan_session_id: plan_session_id.clone(),
            plan_node_id: "workflow_handoff".to_string(),
            plan_version: 1,
            execution_policy_json: Some(
                serde_json::json!({
                    "workflow_handoff": true,
                    "goal": goal,
                })
                .to_string(),
            ),
            campaign_id: enqueue_hints.as_ref().and_then(|h| h.campaign_id.clone()),
            benchmark_tier: enqueue_hints.as_ref().and_then(|h| h.benchmark_tier),
            execution_role: enqueue_hints.as_ref().and_then(|h| h.execution_role),
        };
        self.event_bus
            .emit(crate::events::AgentEventKind::WorkflowHandoffRequested {
                plan_session_id: plan_session_id.clone(),
                workflow_name: "auto".to_string(),
            });
        if crate::lineage::orchestration_lineage_persist_enabled() {
            if let Some(db) = self.db() {
                let repo = crate::lineage::repository_id();
                let mut payload = serde_json::json!({
                    "plan_session_id": plan_session_id,
                    "goal_preview": goal.chars().take(240).collect::<String>(),
                });
                if let Some(cid) = crate::lineage::orchestration_campaign_id() {
                    payload["campaign_id"] = serde_json::Value::String(cid);
                }
                let payload_str = payload.to_string();
                let _ = db
                    .append_orchestration_lineage_event(
                        &repo,
                        "workflow_handoff_started",
                        0_i64,
                        None,
                        session_id.as_deref(),
                        None,
                        Some(plan_session_id.as_str()),
                        Some("workflow_handoff"),
                        Some(payload_str.as_str()),
                    )
                    .await;
            }
        }
        let task_id = self
            .submit_task_with_agent_planned(
                format!("[workflow-handoff] {}", goal),
                file_manifest,
                priority,
                None,
                None,
                session_id,
                enqueue_hints,
                tenant_id,
                Some(meta),
            )
            .await?;
        self.event_bus
            .emit(crate::events::AgentEventKind::WorkflowHandoffCompleted {
                plan_session_id,
                task_id: task_id.0,
            });
        Ok(task_id)
    }

    /// Record a task turn into the durable workflow journal (research_metrics).
    pub async fn record_workflow_turn(&self, task_id: TaskId, turn: &crate::types::TaskTurn) {
        if let Some(db) = self.db() {
            let repo_id = crate::lineage::repository_id();
            let entry = serde_json::json!({
                "type": "task_turn",
                "task_id": task_id.0,
                "agent_id": turn.agent_id.0,
                "agent_name": turn.agent_name,
                "message": turn.message,
                "timestamp_ms": turn.timestamp_ms,
            });
            // Use "main" as the default workflow name for repo-scoped journals.
            let _ = db
                .record_workflow_journal_entry(&repo_id, "main", &entry)
                .await;
        }
    }

    /// Record a task phase change into the durable workflow journal.
    pub async fn record_workflow_phase_change(
        &self,
        task_id: TaskId,
        phase: crate::types::TaskPhase,
    ) {
        if let Some(db) = self.db() {
            let repo_id = crate::lineage::repository_id();
            let entry = serde_json::json!({
                "type": "task_phase_change",
                "task_id": task_id.0,
                "phase": phase.as_str(),
                "timestamp_ms": crate::types::now_unix_ms(),
            });
            let _ = db
                .record_workflow_journal_entry(&repo_id, "main", &entry)
                .await;
        }
    }

    /// Hydrate a task's transcript from the durable workflow journal (replaying history).
    pub async fn hydrate_task_from_journal(
        &self,
        task_id: TaskId,
    ) -> Result<(), OrchestratorError> {
        let db_arc = self.db().ok_or(OrchestratorError::DatabaseError(
            "No database connected".to_string(),
        ))?;
        let repo_id = crate::lineage::repository_id();
        let entries = db_arc
            .load_workflow_journal(&repo_id, "main")
            .await
            .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

        let turns: Vec<crate::types::TaskTurn> = entries
            .into_iter()
            .filter(|e| e["type"] == "task_turn" && e["task_id"] == task_id.0)
            .filter_map(|e| {
                Some(crate::types::TaskTurn {
                    agent_id: crate::types::AgentId(e["agent_id"].as_u64()?),
                    agent_name: e["agent_name"].as_str()?.to_string(),
                    message: e["message"].as_str()?.to_string(),
                    timestamp_ms: e["timestamp_ms"].as_u64()?,
                })
            })
            .collect();

        if !turns.is_empty() {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            for queue_lock in agents.values() {
                let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                if let Some(t) = queue.find_task_mut(task_id) {
                    t.transcript = turns.clone();
                } else if let Some(t) = queue.current_task_mut() {
                    if t.id == task_id {
                        t.transcript = turns.clone();
                    }
                }
            }
        }
        Ok(())
    }

    /// Hydrate ALL tasks in the orchestrator from the durable workflow journal.
    pub async fn hydrate_all_tasks_from_journal(&self) -> Result<(), OrchestratorError> {
        let db_arc = self.db().ok_or(OrchestratorError::DatabaseError(
            "No database connected".to_string(),
        ))?;
        let repo_id = crate::lineage::repository_id();
        let entries = db_arc
            .load_workflow_journal(&repo_id, "main")
            .await
            .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

        // Group turns and phases by task_id
        let mut task_turns: std::collections::HashMap<u64, Vec<crate::types::TaskTurn>> =
            std::collections::HashMap::new();
        let mut task_phases: std::collections::HashMap<u64, crate::types::TaskPhase> =
            std::collections::HashMap::new();

        for e in entries {
            let tid_opt = e["task_id"].as_u64();
            let type_str = e["type"].as_str().unwrap_or("");

            if let Some(tid) = tid_opt {
                if type_str == "task_turn" {
                    if let (Some(aid), Some(name), Some(msg), Some(ts)) = (
                        e["agent_id"].as_u64(),
                        e["agent_name"].as_str(),
                        e["message"].as_str(),
                        e["timestamp_ms"].as_u64(),
                    ) {
                        task_turns.entry(tid).or_default().push(crate::types::TaskTurn {
                            agent_id: crate::types::AgentId(aid),
                            agent_name: name.to_string(),
                            message: msg.to_string(),
                            timestamp_ms: ts,
                        });
                    }
                } else if type_str == "task_phase_change" {
                    if let Some(phase_str) = e["phase"].as_str() {
                        if let Ok(phase) = phase_str.parse::<crate::types::TaskPhase>() {
                            task_phases.insert(tid, phase);
                        }
                    }
                }
            }
        }

        if !task_turns.is_empty() || !task_phases.is_empty() {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            for queue_lock in agents.values() {
                let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                for task in queue.all_tasks_mut() {
                    let tid_u = task.id.0;
                    if let Some(turns) = task_turns.get(&tid_u) {
                        // Only hydrate if empty or shorter, to avoid downgrading memory state
                        if task.transcript.len() < turns.len() {
                            task.transcript = turns.clone();
                        }
                    }
                    if let Some(phase) = task_phases.get(&tid_u) {
                        task.current_phase = Some(*phase);
                    }
                }
            }
        }
        Ok(())
    }
}
