use crate::params::{AskClarificationParams, ResolveFeedbackParams, ToolResult};
use crate::server_state::ServerState;
use vox_orchestrator::feedback::{FeedbackKind, FeedbackResolution, Surface};
use vox_orchestrator::types::TaskId;

pub async fn ask_clarification(state: &ServerState, params: AskClarificationParams) -> String {
    let pending_backlog = state.feedback().open_needs_you().len() as u32;
    let agent_id = params
        .session_id
        .as_deref()
        .and_then(|sid| state.orchestrator.agent_for_session_id(sid))
        .unwrap_or(vox_orchestrator::AgentId(0));
    let bm = state.orchestrator.budget_manager_handle();
    let trust = vox_orchestrator::sync_lock::rw_read(&*bm)
        .trust_snapshot()
        .get(&agent_id)
        .map(|t| t.trust_score)
        .unwrap_or(0.3);

    let signals = vox_orchestrator::InterruptionSignals {
        channel: vox_orchestrator::InterruptionChannel::ChatClarification,
        expected_information_gain_bits: 0.8,
        expected_user_cost: 3.0,
        confidence_estimate: 0.8,
        contradiction_ratio: 0.0,
        pending_clarification_backlog: pending_backlog,
        clarification_turn_index: 0,
        max_clarification_turns: 5,
        irreversible_or_high_risk: false,
        base_interrupt_cost_ms: state.orchestrator_config.attention_interrupt_cost_ms,
        trust_score: trust,
        open_question_session: false,
        spec_uncertainty: 0.0,
        model_uncertainty: 0.0,
    };

    let bm_snap = state.orchestrator.budget_manager_handle();
    let att_snap = vox_orchestrator::sync_lock::rw_read(&*bm_snap).attention_snapshot();

    let decision = crate::attention_policy::evaluate_with_state(state, &signals, &att_snap);
    let surface = vox_orchestrator::feedback::surface_for(&decision);
    let cost = vox_orchestrator::feedback::scaled_cost_of(&decision);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let (event_type, outcome, policy_reason) = match &decision {
        vox_orchestrator::InterruptionDecision::InterruptNow { reason, .. }
        | vox_orchestrator::InterruptionDecision::RequireHumanBeforeContinue { reason, .. } => (
            vox_orchestrator::AttentionEventType::A2AInterrupt,
            vox_orchestrator::ApprovalOutcome::Approved,
            Some(reason.clone()),
        ),
        vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint { reason }
        | vox_orchestrator::InterruptionDecision::BatchWithExistingPrompt { reason }
        | vox_orchestrator::InterruptionDecision::ProceedAutonomously { reason } => (
            vox_orchestrator::AttentionEventType::PolicyDeferred,
            vox_orchestrator::ApprovalOutcome::AutoApproved,
            Some(reason.clone()),
        ),
    };

    let evt = vox_orchestrator::AttentionEvent {
        agent_id,
        task_id: None,
        event_type,
        tier: vox_orchestrator::ApprovalTier::Confirm,
        cost_ms: cost,
        outcome,
        trust_score_at_time: trust,
        effective_complexity: 30.0,
        decision_entropy_bits: 0.8,
        timestamp_ms: ts,
        channel: Some(format!("{:?}", surface).to_lowercase()),
        policy_reason,
    };
    state.record_attention_event(evt);

    let gates_task_ids = params.gates.iter().copied().map(TaskId).collect::<Vec<_>>();

    let id = state.feedback().register(
        FeedbackKind::Clarification,
        params.prompt,
        params.options,
        gates_task_ids.clone(),
        None,
        0.8,
        cost,
        surface,
        params.session_id,
        Some(agent_id),
        ts,
        None,
    );

    let surface_str = match surface {
        Surface::NeedsYou => "needs_you",
        Surface::Withheld => "withheld",
    };

    // T1.2: durable FeedbackRequested BEFORE the event-bus broadcast (Tier-A
    // durable-before-broadcast contract; see `vox_orchestrator::events::is_tier_a`).
    state
        .orchestrator
        .record_operation(
            agent_id,
            vox_orchestrator::oplog::OperationKind::FeedbackRequested {
                request_id: id.0.clone(),
                task_id: gates_task_ids.first().map(|t| t.0),
                kind: "clarification".into(),
            },
            format!("Clarification requested: {}", id.0),
            None,
            None,
            None,
            None,
        )
        .await;

    state
        .orchestrator
        .event_bus()
        .emit(vox_orchestrator::AgentEventKind::FeedbackRequested {
            feedback_id: id.0.clone(),
            kind: "clarification".into(),
            gates: gates_task_ids.iter().map(|t| t.0).collect(),
            surface: surface_str.to_string(),
        });

    ToolResult::ok(serde_json::json!({
        "feedback_id": id.0,
        "surface": surface_str
    }))
    .to_json()
}

pub async fn resolve_feedback(state: &ServerState, params: ResolveFeedbackParams) -> String {
    let fid = vox_orchestrator::feedback::FeedbackId(params.feedback_id.clone());
    let Some(req) = state.feedback().get(&fid) else {
        return ToolResult::<serde_json::Value>::err("feedback not found").to_json();
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let action = vox_orchestrator::feedback::FeedbackAction::from(params.action.clone());

    let resolution = FeedbackResolution {
        action: action.clone(),
        decided_at_ms: now_ms,
        decided_by: "gui".into(),
    };

    let Some(_resolved) = state.feedback().resolve(&fid, resolution) else {
        return ToolResult::<serde_json::Value>::err("already resolved or not found").to_json();
    };

    let agent_id = req.agent_id.unwrap_or(vox_orchestrator::AgentId(0));
    let bm = state.orchestrator.budget_manager_handle();
    let trust = vox_orchestrator::sync_lock::rw_read(&*bm)
        .trust_snapshot()
        .get(&agent_id)
        .map(|t| t.trust_score)
        .unwrap_or(0.3);

    let evt = vox_orchestrator::AttentionEvent {
        agent_id,
        task_id: None,
        event_type: vox_orchestrator::AttentionEventType::ClarificationAnswered,
        tier: vox_orchestrator::ApprovalTier::Confirm,
        cost_ms: req.scaled_cost_ms,
        outcome: vox_orchestrator::ApprovalOutcome::Approved,
        trust_score_at_time: trust,
        effective_complexity: 30.0,
        decision_entropy_bits: req.info_gain_bits,
        timestamp_ms: now_ms,
        channel: Some(format!("{:?}", req.kind).to_lowercase()),
        policy_reason: Some("resolved".to_string()),
    };
    state.record_attention_event(evt);

    if req.kind == FeedbackKind::Doubt {
        if let (vox_orchestrator::feedback::FeedbackAction::Overrule, Some(tid)) =
            (&action, req.doubted_task_id)
        {
            if let Err(e) = state
                .orchestrator
                .overrule_task(tid, Some("Overruled by user via Needs You".to_string()))
            {
                tracing::error!("Failed to overrule task {}: {:?}", tid.0, e);
            }
        }
    }

    // T1.2: durable FeedbackResolved BEFORE the event-bus broadcast. The
    // action string mirrors `FeedbackAction`'s `#[serde(tag = "action",
    // rename_all = "snake_case")]` wire form (e.g. "answer", "overrule").
    let action_str = serde_json::to_value(&action)
        .ok()
        .and_then(|v| v.get("action").and_then(|a| a.as_str()).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());
    state
        .orchestrator
        .record_operation(
            agent_id,
            vox_orchestrator::oplog::OperationKind::FeedbackResolved {
                request_id: fid.0.clone(),
                action: action_str,
            },
            format!("Feedback resolved: {}", fid.0),
            None,
            None,
            None,
            None,
        )
        .await;

    state
        .orchestrator
        .event_bus()
        .emit(vox_orchestrator::AgentEventKind::FeedbackResolved {
            feedback_id: fid.0.clone(),
        });

    state.feedback().promote_withheld(|item| item.surface);

    if req.kind == FeedbackKind::SkillProposal
        && matches!(
            action,
            vox_orchestrator::feedback::FeedbackAction::AcceptSkill
        )
    {
        let Some(candidate) = req.meta.as_ref() else {
            return ToolResult::<serde_json::Value>::err("skill proposal has no candidate payload")
                .to_json();
        };
        let Some(ws_root) = state.workspace_root.clone() else {
            return ToolResult::<serde_json::Value>::err("no workspace root; cannot install skill")
                .to_json();
        };
        return match author_and_install_skill(candidate, &ws_root) {
            Ok(names) => {
                ToolResult::ok(serde_json::json!({"resolved": true, "installed": names})).to_json()
            }
            Err(e) => ToolResult::<serde_json::Value>::err(&e).to_json(),
        };
    }

    ToolResult::ok(serde_json::json!({
        "resolved": true
    }))
    .to_json()
}

/// Author a `SKILL.md` from a serialized mined `Candidate` and install it
/// workspace-local under `<ws_root>/.vox/skills/<name>/`. Returns installed names.
pub(crate) fn author_and_install_skill(
    candidate: &serde_json::Value,
    ws_root: &std::path::Path,
) -> Result<Vec<String>, String> {
    use vox_skill_discovery::candidate::Candidate;
    let cand: Candidate = serde_json::from_value(candidate.clone())
        .map_err(|e| format!("bad candidate payload: {e}"))?;
    let df = cand
        .draft_frontmatter
        .ok_or_else(|| "candidate has no draft frontmatter".to_string())?;
    let md = vox_plugin_host::author_skill_md(&df.name, &df.description, &cand.members);

    // Author into a fresh, unique temp dir. `TempDir` is RAII: it self-cleans on
    // drop, including every early-return error path below. The dir holds only our
    // one SKILL.md, so the installer discovers exactly one skill.
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let skill_dir = tmp.path().join("skill");
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
    std::fs::write(skill_dir.join("SKILL.md"), md).map_err(|e| e.to_string())?;

    let src = tmp.path().to_string_lossy();
    let installed = vox_plugin_host::install_to_user_root(&src, ws_root, false, None)?;
    Ok(installed.into_iter().map(|i| i.name).collect())
}

pub async fn propose_skill(
    state: &ServerState,
    params: crate::params::ProposeSkillParams,
) -> String {
    match state.orchestrator.propose_skill(
        &params.name,
        &params.description,
        params.session_id,
        params.candidate,
    ) {
        Some(fid) => {
            // T1.2: durable FeedbackRequested BEFORE the bus broadcast. `propose_skill`
            // itself stays sync (dedup check only) and no longer emits on the event bus;
            // we record durably here (already async), then broadcast explicitly.
            state
                .orchestrator
                .record_operation(
                    vox_orchestrator::AgentId(0),
                    vox_orchestrator::oplog::OperationKind::FeedbackRequested {
                        request_id: fid.0.clone(),
                        task_id: None,
                        kind: "skill_proposal".into(),
                    },
                    format!("Skill proposal requested: {}", fid.0),
                    None,
                    None,
                    None,
                    None,
                )
                .await;
            state
                .orchestrator
                .emit_feedback_requested_skill_proposal(&fid);
            ToolResult::ok(serde_json::json!({ "feedback_id": fid.0 })).to_json()
        }
        None => ToolResult::ok(serde_json::json!({ "skipped": "duplicate proposal already open" }))
            .to_json(),
    }
}

pub async fn feedback_list(state: &ServerState, _params: serde_json::Value) -> String {
    let needs_you = state.feedback().open_needs_you();
    let withheld = state.feedback().withheld();

    ToolResult::ok(serde_json::json!({
        "needs_you": needs_you,
        "withheld": withheld
    }))
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{AskClarificationParams, ResolveFeedbackParams};
    use vox_orchestrator::feedback::{FeedbackKind, Surface};
    use vox_orchestrator::types::TaskId;

    #[tokio::test]
    async fn test_feedback_tools_lifecycle() {
        let state = ServerState::new_test().await;

        // 1. Test ask_clarification
        let ask_params = AskClarificationParams {
            prompt: "What is your database schema?".to_string(),
            options: vec!["postgres".to_string(), "mysql".to_string()],
            gates: vec![123],
            session_id: Some("session-1".to_string()),
        };

        let res_json = ask_clarification(&state, ask_params).await;
        let res_val: serde_json::Value = serde_json::from_str(&res_json).unwrap();
        assert!(res_val.get("success").unwrap().as_bool().unwrap());
        let data = res_val.get("data").unwrap();
        let fid = data
            .get("feedback_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        let surface = data.get("surface").unwrap().as_str().unwrap().to_string();

        // 2. Test feedback_list
        let list_json = feedback_list(&state, serde_json::json!({})).await;
        let list_val: serde_json::Value = serde_json::from_str(&list_json).unwrap();
        assert!(list_val.get("success").unwrap().as_bool().unwrap());
        let list_data = list_val.get("data").unwrap();

        if surface == "needs_you" {
            let needs_you_arr = list_data.get("needs_you").unwrap().as_array().unwrap();
            assert_eq!(needs_you_arr.len(), 1);
            assert_eq!(needs_you_arr[0].get("id").unwrap().as_str().unwrap(), fid);
        } else {
            let withheld_arr = list_data.get("withheld").unwrap().as_array().unwrap();
            assert_eq!(withheld_arr.len(), 1);
            assert_eq!(withheld_arr[0].get("id").unwrap().as_str().unwrap(), fid);
        }

        // 3. Test resolve_feedback
        let resolve_params = ResolveFeedbackParams {
            feedback_id: fid.clone(),
            action: crate::params::McpFeedbackAction::Answer {
                option: Some(0),
                text: None,
            },
        };
        let resolve_json = resolve_feedback(&state, resolve_params).await;
        let resolve_val: serde_json::Value = serde_json::from_str(&resolve_json).unwrap();
        assert!(resolve_val.get("success").unwrap().as_bool().unwrap());

        // 4. Test list after resolve
        let list_json2 = feedback_list(&state, serde_json::json!({})).await;
        let list_val2: serde_json::Value = serde_json::from_str(&list_json2).unwrap();
        let list_data2 = list_val2.get("data").unwrap();
        assert_eq!(
            list_data2
                .get("needs_you")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            list_data2
                .get("withheld")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn test_resolve_doubt_overrule() {
        let state = ServerState::new_test().await;

        // Manually register a Doubt
        let fid = state.feedback().register(
            FeedbackKind::Doubt,
            "Doubted result".to_string(),
            vec![],
            vec![],
            Some(TaskId(999)),
            0.0,
            0,
            Surface::NeedsYou,
            None,
            None,
            1,
            None,
        );

        let resolve_params = ResolveFeedbackParams {
            feedback_id: fid.0,
            action: crate::params::McpFeedbackAction::Overrule,
        };

        let res_json = resolve_feedback(&state, resolve_params).await;
        let res_val: serde_json::Value = serde_json::from_str(&res_json).unwrap();
        assert!(res_val.get("success").unwrap().as_bool().unwrap());
    }

    #[test]
    fn mcp_accept_skill_deserializes() {
        let v = serde_json::json!({"action": "accept_skill"});
        let a: crate::params::McpFeedbackAction = serde_json::from_value(v).unwrap();
        let core: vox_orchestrator::feedback::FeedbackAction = a.into();
        assert_eq!(
            core,
            vox_orchestrator::feedback::FeedbackAction::AcceptSkill
        );
    }

    #[test]
    fn author_and_install_writes_workspace_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = serde_json::json!({
            "kind": "RepeatedOperations",
            "members": ["read", "edit", "run"],
            "score": 6.0,
            "suggested_action": "Save recurring procedure as a skill",
            "draft_frontmatter": {
                "name": "read-edit-run",
                "description": "Recurring procedure: read → edit → run (seen 4× across 2 sessions)",
                "category": "workflow",
                "tags": ["auto-discovered", "operations"]
            }
        });
        let names = super::author_and_install_skill(&candidate, tmp.path()).unwrap();
        assert_eq!(names, vec!["read-edit-run".to_string()]);
        let f = tmp
            .path()
            .join(".vox")
            .join("skills")
            .join("read-edit-run")
            .join("SKILL.md");
        assert!(f.exists(), "expected {f:?} to exist");
    }
}
