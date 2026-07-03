use super::*;
use crate::params::{CompleteTaskParams, DrainAgentParams, FailTaskParams, ToolResult};
use vox_orchestrator::TaskId;

pub(super) const REM_TASK_ORCH_OP: &str = "Verify task lifecycle state, file locks, and orchestrator health before complete/fail/cancel/reorder/drain.";

/// Mark a task as completed, releasing its file locks (async).
pub async fn complete_task(state: &ServerState, params: CompleteTaskParams) -> String {
    let task_id = TaskId(params.task_id);
    let assigned = state.orchestrator.agent_assigned_to_task(task_id);
    let attestation = vox_orchestrator::CompletionAttestation {
        completion_summary: params.completion_summary,
        checks_passed: params.checks_passed,
        evidence_citations: params.evidence_citations,
        artifact_paths: params.artifact_paths.into_iter().map(Into::into).collect(),
        declared_non_placeholder: params.declared_non_placeholder,
        force_risky: params.force_risky,
        force_risky_reason: params.force_risky_reason,
        observation_summary: None,
        // Attribution fields (completing_model/provider/tokens/…) are populated by
        // the orchestrator from the task's SelectedModelRecord at completion time.
        ..Default::default()
    };
    let res = state
        .orchestrator
        .complete_task_with_attestation(task_id, Some(attestation))
        .await;

    match res {
        Ok(()) => {
            // Gamification: update the agent-scoped companion (matches event_router / HUD).
            if let (Some(db), Some(aid)) = (&state.db, assigned) {
                let uid = vox_gamify::db::canonical_user_id();
                let id = format!("agent-{}", aid.0);
                let mut companion = match vox_gamify::db::list_companions(db, &uid).await {
                    Ok(comps) => comps
                        .into_iter()
                        .find(|c: &vox_gamify::companion::Companion| c.id == id),
                    Err(_) => None,
                }
                .unwrap_or_else(|| {
                    vox_gamify::companion::Companion::new(
                        &id,
                        &uid,
                        format!("Agent {}", aid.0),
                        "vox",
                    )
                });

                companion.interact(vox_gamify::companion::Interaction::TaskCompleted);
                let _ = vox_gamify::db::upsert_companion(db, &companion).await;
            }
            let trace_ctx = vox_telemetry::current_trace_ctx();
            let mut summary = vox_telemetry::TaskRootSummaryEvent {
                task_id: params.task_id,
                trace_id: trace_ctx.trace_id.to_string(),
                repository_id: None,
                outcome: "completed".into(),
                wall_time_ms: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost_usd: 0.0,
                child_call_count: 0,
                max_span_depth: trace_ctx.span_depth,
                subagent_fanout: 0,
            };
            vox_telemetry::fill_task_root_summary(&mut summary);
            vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::TaskRootSummary(summary));
            ToolResult::ok("task completed".to_string()).to_json()
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_TASK_ORCH_OP).to_json()
        }
    }
}

/// Mark a task as failed with a reason (async).
pub async fn fail_task(state: &ServerState, params: FailTaskParams) -> String {
    let task_id = TaskId(params.task_id);
    let assigned = state.orchestrator.agent_assigned_to_task(task_id);
    let res = state
        .orchestrator
        .fail_task(task_id, params.reason)
        .await
        .map_err(|e| e.to_string());

    match res {
        Ok(()) => {
            if let (Some(db), Some(aid)) = (&state.db, assigned) {
                let uid = vox_gamify::db::canonical_user_id();
                let id = format!("agent-{}", aid.0);
                let mut companion = match vox_gamify::db::list_companions(db, &uid).await {
                    Ok(comps) => comps
                        .into_iter()
                        .find(|c: &vox_gamify::companion::Companion| c.id == id),
                    Err(_) => None,
                }
                .unwrap_or_else(|| {
                    vox_gamify::companion::Companion::new(
                        &id,
                        &uid,
                        format!("Agent {}", aid.0),
                        "vox",
                    )
                });

                companion.interact(vox_gamify::companion::Interaction::TaskFailed);
                let _ = vox_gamify::db::upsert_companion(db, &companion).await;
            }
            let trace_ctx = vox_telemetry::current_trace_ctx();
            let mut summary = vox_telemetry::TaskRootSummaryEvent {
                task_id: params.task_id,
                trace_id: trace_ctx.trace_id.to_string(),
                repository_id: None,
                outcome: "failed".into(),
                wall_time_ms: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost_usd: 0.0,
                child_call_count: 0,
                max_span_depth: trace_ctx.span_depth,
                subagent_fanout: 0,
            };
            vox_telemetry::fill_task_root_summary(&mut summary);
            vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::TaskRootSummary(summary));
            ToolResult::ok("task marked as failed".to_string()).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_TASK_ORCH_OP).to_json(),
    }
}

/// Cancel a task by ID.
pub async fn cancel_task(state: &ServerState, params: crate::params::CancelTaskParams) -> String {
    match state
        .orchestrator
        .cancel_task(TaskId(params.task_id))
        .map_err(|e| e.to_string())
    {
        Ok(()) => ToolResult::ok("Task cancelled successfully".to_string()).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_TASK_ORCH_OP).to_json(),
    }
}

/// Change the priority of a queued task.
pub async fn reorder_task(state: &ServerState, params: crate::params::ReorderTaskParams) -> String {
    let priority = match params.priority.as_str() {
        "urgent" => vox_orchestrator::TaskPriority::Urgent,
        "background" => vox_orchestrator::TaskPriority::Background,
        _ => vox_orchestrator::TaskPriority::Normal,
    };

    match state
        .orchestrator
        .reorder_task(TaskId(params.task_id), priority)
        .map_err(|e| e.to_string())
    {
        Ok(()) => ToolResult::ok("Task reordered successfully".to_string()).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_TASK_ORCH_OP).to_json(),
    }
}

/// Flag a task as suspect by the user, triggering a resolution loop.
pub async fn doubt_task(state: &ServerState, params: crate::params::DoubtTaskParams) -> String {
    let task_id = TaskId(params.task_id);
    let assigned = state.orchestrator.agent_assigned_to_task(task_id);
    let reason_for_oplog = params.reason.clone();
    let res = state
        .orchestrator
        .doubt_task(task_id, params.reason)
        .map_err(|e| e.to_string());

    match res {
        Ok(()) => {
            // T1.1: durable TaskDoubted alongside the event-bus emit + FeedbackStore
            // registration inside `Orchestrator::doubt_task` (which is sync, so the
            // oplog write happens here at the async MCP call site instead).
            state
                .orchestrator
                .record_operation(
                    assigned.unwrap_or(vox_orchestrator::AgentId(0)),
                    vox_orchestrator::oplog::OperationKind::TaskDoubted {
                        task_id: params.task_id,
                        reason: reason_for_oplog,
                    },
                    format!("Task {} doubted", params.task_id),
                    None,
                    None,
                    None,
                    None,
                )
                .await;
            // Gamification: suspecting is a habit-building interaction.
            if let (Some(db), Some(aid)) = (&state.db, assigned) {
                let uid = vox_gamify::db::canonical_user_id();
                let id = format!("agent-{}", aid.0);
                let mut companion = match vox_gamify::db::list_companions(db, &uid).await {
                    Ok(comps) => comps
                        .into_iter()
                        .find(|c: &vox_gamify::companion::Companion| c.id == id),
                    Err(_) => None,
                }
                .unwrap_or_else(|| {
                    vox_gamify::companion::Companion::new(
                        &id,
                        &uid,
                        format!("Agent {}", aid.0),
                        "vox",
                    )
                });

                companion.interact(vox_gamify::companion::Interaction::TaskDoubted);
                let _ = vox_gamify::db::upsert_companion(db, &companion).await;
            }
            let trace_ctx = vox_telemetry::current_trace_ctx();
            let mut summary = vox_telemetry::TaskRootSummaryEvent {
                task_id: params.task_id,
                trace_id: trace_ctx.trace_id.to_string(),
                repository_id: None,
                outcome: "doubted".into(),
                wall_time_ms: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost_usd: 0.0,
                child_call_count: 0,
                max_span_depth: trace_ctx.span_depth,
                subagent_fanout: 0,
            };
            vox_telemetry::fill_task_root_summary(&mut summary);
            vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::TaskRootSummary(summary));
            ToolResult::ok("task flagged as suspect; resolution agent escalated".to_string())
                .to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_TASK_ORCH_OP).to_json(),
    }
}

/// Remove all queued tasks from an agent without retiring it.
pub async fn drain_agent(state: &ServerState, params: DrainAgentParams) -> String {
    match state
        .orchestrator
        .drain_agent(vox_orchestrator::AgentId(params.agent_id))
    {
        Ok(tasks) => ToolResult::ok(format!(
            "Drained {} tasks from agent {}",
            tasks.len(),
            params.agent_id
        ))
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_TASK_ORCH_OP).to_json()
        }
    }
}
