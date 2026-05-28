use serde::Deserialize;
use serde_json::Value;

use super::build_system_prompt;
use super::params::{
    PlanDepth, PlanListSessionsParams, PlanLoopMode, PlanParams, PlanReplanParams, PlanResult,
    PlanResumeParams, PlanStatusParams, PlanTask,
};
use super::plan_gap;
use super::plan_loop;
use crate::attention_policy::{
    evaluate_with_state, pending_backlog_for_session, plan_review_signals, trust_for_session,
};
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::chat_socrates_meta::{
    clarification_turn_for_session, mcp_questioning_session_key, socrates_surface_tags,
    socrates_tool_meta, spawn_questioning_trace_from_socrates, spawn_socrates_telemetry_with_meta,
};
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, mcp_infer_completion};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_orchestrator::planning::{ContentBlock, markdown_to_content_blocks};

const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_MCP_MODEL_LOCK: &str =
    "Retry; restart the MCP server if `mcp_chat_model_override` stays poisoned.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";
const REM_PLAN_JSON: &str = "Retry planning with a simpler goal or lower `max_tasks`; ensure the model returns valid JSON in a ```json block.";
const REM_PLAN_ADEQUACY_ENFORCE: &str = "Widen scope with concrete steps, paths, and verification; increase `plan_depth`; enable refinement (`loop_mode`) or raise caps; or set `VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE=false` on the MCP host.";
const REM_DEI_DAEMON: &str =
    "Start `vox-orchestrator-d` or verify IPC/socket configuration for this workspace.";

/// When true, `vox_plan` must not return success if the tier‑1 report is still thin.
pub(crate) fn plan_result_blocked_by_adequacy_enforce(
    cfg: &vox_orchestrator::OrchestratorConfig,
    report: &vox_orchestrator::planning::PlanRefinementReport,
) -> bool {
    cfg.plan_adequacy_enforce && report.adequacy.is_too_thin
}

fn plan_depth_rider(depth: PlanDepth) -> &'static str {
    match depth {
        PlanDepth::Minimal => {
            "Planning depth: MINIMAL — use fewer, broader tasks when possible, but still name explicit verification for risky or data-moving work."
        }
        PlanDepth::Standard => {
            "Planning depth: STANDARD — balanced decomposition with clear dependencies and explicit tests where appropriate."
        }
        PlanDepth::Deep => {
            "Planning depth: DEEP — produce MORE granular tasks: narrow scopes, explicit file paths where inferable, strong dependency chains, plus verification, migration, and rollback steps as applicable."
        }
    }
}

fn initial_plan_max_tokens(depth: PlanDepth) -> u64 {
    match depth {
        PlanDepth::Minimal => 3072,
        PlanDepth::Standard => 4096,
        PlanDepth::Deep => 8192,
    }
}

fn strip_plan_json_fence(block: &str) -> &str {
    let block = block.trim();
    if block.starts_with("```json") {
        block
            .strip_prefix("```json")
            .unwrap_or(block)
            .strip_suffix("```")
            .unwrap_or(block)
            .trim()
    } else if block.starts_with("```") {
        block
            .strip_prefix("```")
            .unwrap_or(block)
            .strip_suffix("```")
            .unwrap_or(block)
            .trim()
    } else {
        block
    }
}

fn parse_plan_payload(raw: &str) -> Result<PlanResponseSchema, serde_json::Error> {
    let cleaned = strip_plan_json_fence(raw);
    serde_json::from_str(cleaned)
}

fn effective_loop_mode_label(params: &PlanParams) -> String {
    let base = params.loop_mode.unwrap_or_default();
    let mut s = format!("{base:?}").to_ascii_lowercase();
    if matches!(base, PlanLoopMode::Off) && params.auto_expand_thin_plan != Some(false) {
        s.push_str("+auto_expand_thin");
    }
    s
}

fn should_emit_plan_interrupt(
    state: &ServerState,
    surface: &'static str,
    session_key: &str,
    expected_gain_bits: f64,
    expected_user_cost: f64,
    high_risk: bool,
) -> bool {
    if !state.orchestrator_config.attention_enabled {
        return true;
    }
    let bm = state.orchestrator.budget_manager_handle();
    let att_snap = vox_orchestrator::sync_lock::rw_read(&*bm).attention_snapshot();
    let backlog = pending_backlog_for_session(state, Some(session_key));
    let trust = trust_for_session(state, Some(session_key));
    let signals = plan_review_signals(
        expected_gain_bits,
        expected_user_cost,
        backlog,
        trust,
        high_risk,
        state.orchestrator_config.attention_interrupt_cost_ms,
    );
    match evaluate_with_state(state, &signals, &att_snap) {
        vox_orchestrator::InterruptionDecision::InterruptNow { .. }
        | vox_orchestrator::InterruptionDecision::RequireHumanBeforeContinue { .. } => true,
        vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint { reason }
        | vox_orchestrator::InterruptionDecision::BatchWithExistingPrompt { reason } => {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            state.record_attention_event(vox_orchestrator::AttentionEvent {
                agent_id: state
                    .orchestrator
                    .agent_for_session_id(session_key)
                    .unwrap_or(vox_orchestrator::AgentId(0)),
                task_id: None,
                event_type: vox_orchestrator::AttentionEventType::PolicyDeferred,
                tier: vox_orchestrator::ApprovalTier::Confirm,
                cost_ms: 0,
                outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                trust_score_at_time: trust,
                effective_complexity: (expected_user_cost * 10.0).clamp(0.0, 10.0),
                decision_entropy_bits: expected_gain_bits,
                timestamp_ms: ts,
                channel: Some(surface.to_string()),
                policy_reason: Some(reason),
            });
            false
        }
        vox_orchestrator::InterruptionDecision::ProceedAutonomously { reason } => {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            state.record_attention_event(vox_orchestrator::AttentionEvent {
                agent_id: state
                    .orchestrator
                    .agent_for_session_id(session_key)
                    .unwrap_or(vox_orchestrator::AgentId(0)),
                task_id: None,
                event_type: vox_orchestrator::AttentionEventType::PolicyProceedAuto,
                tier: vox_orchestrator::ApprovalTier::AutoApprove,
                cost_ms: 0,
                outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                trust_score_at_time: trust,
                effective_complexity: (expected_user_cost * 10.0).clamp(0.0, 10.0),
                decision_entropy_bits: expected_gain_bits,
                timestamp_ms: ts,
                channel: Some(surface.to_string()),
                policy_reason: Some(reason),
            });
            false
        }
    }
}

#[derive(Deserialize)]
struct PlanResponseSchema {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    tasks: Vec<PlanTask>,
}

/// Generate a structured plan for a goal. Optionally writes PLAN.md to the workspace root.
/// This backs the Cursor-style "Planning Mode" in the extension and in Vox agents.
pub async fn plan_goal(state: &ServerState, params: PlanParams) -> String {
    let max_tasks = params.max_tasks.unwrap_or(30);
    let plan_depth = params.plan_depth.unwrap_or_default();
    let scope_note = if params.scope_files.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nScope this plan to these files:\n{}",
            params.scope_files.join("\n")
        )
    };

    let user_prompt = format!(
        r#"You are an expert software architect and planner.

GOAL: {goal}{scope_note}

{depth_rider}

Generate a comprehensive, ordered task list to achieve this goal. You MUST output a valid JSON object matching this schema, embedded in a ```json codeblock.

{{
  "summary": "2-3 sentence executive summary of the approach",
  "tasks": [
    {{
      "id": 1,
      "description": "Short imperative description of what to implement.",
      "category": "CodeGen",
      "files": ["path/to/file.rs"],
      "estimated_complexity": 5,
      "depends_on": []
    }}
  ]
}}

Rules:
- Every task must be atomic and independently verifiable.
- "category" must be one of: "CodeGen", "Refactor", "Test", "Documentation", "Research", or "InfraConfig".
- "estimated_complexity" must be an integer from 1 (trivial edit) to 10 (full subsystem build).
- "depends_on" must be an array of prior task IDs that must complete first.
- If files are unknown, leave the array empty or use `["TBD"]`.
- Include test tasks explicitly.
- Maximum {max_tasks} tasks.
- Do NOT include filler tasks like 'Review and refactor'."#,
        goal = params.goal,
        max_tasks = max_tasks,
        scope_note = scope_note,
        depth_rider = plan_depth_rider(plan_depth),
    );

    let system_prompt = build_system_prompt(state, None).await;
    let resolution_template = McpChatModelResolution {
        complexity: match params.max_tasks {
            Some(n) if n > 10 => 9,
            _ => 7,
        },
        ..Default::default()
    };

    let (model, free_only) = match resolve_chat_llm_model(
        state,
        &user_prompt,
        resolution_template.clone(),
        params.session_id.as_deref(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("No model found for plan: {e}"),
                REM_MCP_MODEL_RESOLVE,
            )
            .to_json();
        }
    };

    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e.to_string(), REM_MCP_MODEL_LOCK)
                .to_json();
        }
    };
    let routing = McpInferRouting {
        user_prompt: &user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template: resolution_template.clone(),
        free_only,
        allow_cloud_ollama_fallback: true,
        user_id: params.session_id.as_deref(),
    };

    let plan_llm_started = std::time::Instant::now();
    let initial_cap = initial_plan_max_tokens(plan_depth);
    let (mut response_json, mut model_used, _tokens) = match mcp_infer_completion(
        state,
        model.clone(),
        "vox_plan",
        &system_prompt,
        &routing,
        initial_cap,
        0.3,
        None,
        None,
        true, // Enforce strict JSON mode for planning
        None,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("LLM error: {e}"),
                REM_LLM_COMPLETION,
            )
            .to_json();
        }
    };

    let parsed: PlanResponseSchema = match parse_plan_payload(&response_json) {
        Ok(p) => p,
        Err(e) => {
            let snippet: String = response_json.chars().take(1800).collect();
            let fix_prompt = format!(
                r#"Your previous planner output was not valid JSON (parse error: {err}).

Output ONLY a ```json fenced block containing a single object with keys "summary" and "tasks" (array of task objects with id, description, files, estimated_complexity, depends_on). No surrounding prose.

Invalid prior output (may be truncated):
{snippet}"#,
                err = e,
                snippet = snippet
            );
            let routing_fix = McpInferRouting {
                user_prompt: &fix_prompt,
                sticky_model_pref: pref.as_deref(),
                resolution_template: resolution_template.clone(),
                free_only,
                allow_cloud_ollama_fallback: true,
                user_id: params.session_id.as_deref(),
            };
            let retry_cap = initial_cap.max(8192);
            match mcp_infer_completion(
                state,
                model.clone(),
                "vox_plan_retry_json",
                &system_prompt,
                &routing_fix,
                retry_cap,
                0.15,
                None,
                None,
                true,
                None,
            )
            .await
            {
                Ok((rj2, m2, _)) => {
                    response_json = rj2;
                    model_used = m2;
                    match parse_plan_payload(&response_json) {
                        Ok(p) => p,
                        Err(e2) => {
                            let cleaned = strip_plan_json_fence(&response_json);
                            tracing::error!(error = %e2, raw = cleaned, "plan_goal: JSON decode failed after retry");
                            return ToolResult::<String>::err_with_remediation(
                                format!("Failed to parse task list JSON: {e2}"),
                                REM_PLAN_JSON,
                            )
                            .to_json();
                        }
                    }
                }
                Err(e_fix) => {
                    tracing::error!(error = %e, fix_error = %e_fix, raw = strip_plan_json_fence(&response_json), "plan_goal: JSON decode failed; retry LLM failed");
                    return ToolResult::<String>::err_with_remediation(
                        format!("Failed to parse task list JSON: {e}"),
                        REM_PLAN_JSON,
                    )
                    .to_json();
                }
            }
        }
    };

    let plan_session_key =
        mcp_questioning_session_key(state, "vox_plan", params.session_id.as_deref());
    state.record_questioning_attention_spend(
        &plan_session_key,
        plan_llm_started.elapsed().as_millis() as u64,
    );

    let summary = if parsed.summary.is_empty() {
        "No summary provided.".to_string()
    } else {
        parsed.summary
    };
    let mut tasks = parsed.tasks;
    let task_count_before_refine = tasks.len();
    let pre_refine_report = plan_gap::analyze_plan_gaps(
        &params.goal,
        params.scope_files.len(),
        None,
        params.plan_depth,
        &tasks,
        None,
    );

    let loop_sess = mcp_questioning_session_key(state, "vox_plan", params.session_id.as_deref());
    let complexity_for_refine = match params.max_tasks {
        Some(n) if n > 10 => 9,
        _ => 7,
    };
    let (refined_tasks, refined_summary, loop_state) = plan_loop::maybe_refine_plan(
        state,
        &params,
        tasks,
        summary,
        complexity_for_refine,
        &loop_sess,
    )
    .await;
    tasks = refined_tasks;
    let summary = refined_summary;

    if let Some(rep) = loop_state.last_gap_report.as_ref()
        && plan_result_blocked_by_adequacy_enforce(&state.orchestrator_config, rep)
    {
        let reasons = rep.adequacy.reason_codes.join(", ");
        return ToolResult::<String>::err_with_remediation(
            format!(
                "Plan adequacy: refined plan is still too thin for this goal (score {:.2}; codes: {}).",
                rep.adequacy.score, reasons
            ),
            REM_PLAN_ADEQUACY_ENFORCE,
        )
        .to_json();
    }

    if let Some(db) = state.db.as_ref() {
        if let Some(pid) = params.plan_telemetry_session_id.as_deref() {
            let strat = format!(
                "mcp_plan:{}:{}",
                effective_loop_mode_label(&params),
                format!("{:?}", params.plan_depth.unwrap_or_default()).to_ascii_lowercase()
            );
            let _ = db
                .create_plan_session(pid, params.session_id.as_deref(), &params.goal, &strat)
                .await;
            let post = loop_state.last_gap_report.as_ref();
            let adeq_improved = post.map(|g| {
                let score_up = g.adequacy.score > pre_refine_report.adequacy.score + 0.01;
                let thin_cleared =
                    pre_refine_report.adequacy.is_too_thin && !g.adequacy.is_too_thin;
                let risk_down = g.aggregate_unresolved_risk + 0.02
                    < pre_refine_report.aggregate_unresolved_risk;
                score_up || thin_cleared || risk_down
            });
            let meta = serde_json::json!({
                "refinement_rounds": loop_state.refinement_rounds,
                "loop_status": loop_state.loop_status,
                "stop_reason": loop_state.stop_reason,
                "telemetry": "vox_mcp_iterative_plan",
                "plan_depth": format!("{:?}", params.plan_depth.unwrap_or_default()).to_ascii_lowercase(),
                "initial_plan_max_output_tokens": initial_cap,
                "task_count_before_refine": task_count_before_refine,
                "task_count_after_refine": tasks.len(),
                "adequacy_improved_heuristic": adeq_improved,
                "adequacy_before": {
                    "score": pre_refine_report.adequacy.score,
                    "is_too_thin": pre_refine_report.adequacy.is_too_thin,
                    "reason_codes": pre_refine_report.adequacy.reason_codes,
                    "aggregate_unresolved_risk": pre_refine_report.aggregate_unresolved_risk,
                },
                "adequacy_after": post.map(|g| {
                    serde_json::json!({
                        "score": g.adequacy.score,
                        "is_too_thin": g.adequacy.is_too_thin,
                        "reason_codes": g.adequacy.reason_codes,
                        "detail_target_min_tasks": g.adequacy.detail_target_min_tasks,
                        "estimated_goal_complexity": g.adequacy.estimated_goal_complexity,
                    })
                }),
                "adequacy": post.map(|g| {
                    serde_json::json!({
                        "score": g.adequacy.score,
                        "is_too_thin": g.adequacy.is_too_thin,
                        "reason_codes": g.adequacy.reason_codes,
                        "detail_target_min_tasks": g.adequacy.detail_target_min_tasks,
                        "estimated_goal_complexity": g.adequacy.estimated_goal_complexity,
                    })
                }),
                "aggregate_unresolved_risk": post.map(|g| g.aggregate_unresolved_risk),
            });
            let _ = db
                .update_plan_session_iterative_fields(
                    pid,
                    params.question_link_session_id.as_deref(),
                    i64::from(loop_state.refinement_rounds),
                    loop_state.stop_reason.as_deref(),
                    Some(&meta.to_string()),
                )
                .await;

            // Persist plan nodes to DB for live tracking (T-019)
            let head = db.load_plan_head(pid).await.unwrap_or(None).unwrap_or(0);
            let ver = if head == 0 { 1 } else { head };
            if head == 0 {
                let _ = db
                    .append_plan_version(pid, 1, None, Some("initial"), None)
                    .await;
            }
            for t in &tasks {
                let policy = serde_json::json!({
                    "file_manifest": t.files,
                    "estimated_complexity": t.estimated_complexity
                });
                let _ = db
                    .upsert_plan_node(
                        pid,
                        ver,
                        &t.id.to_string(),
                        &t.description,
                        &serde_json::to_string(&t.depends_on).unwrap_or_default(),
                        &policy.to_string(),
                        "pending",
                        None,
                    )
                    .await;
            }
        }
    }

    let plan_total_tasks = tasks.len();
    let page_off = params.plan_page_offset.unwrap_or(0);
    let tasks_for_payload: Vec<PlanTask> = if let Some(lim) = params.plan_page_limit {
        tasks.iter().skip(page_off).take(lim).cloned().collect()
    } else {
        tasks.clone()
    };

    // Manual markdown generation for the on-disk/visual summary
    let mut base_plan_md = format!("## Plan\n\n**Overall Summary**: {summary}\n\n### Tasks\n\n");
    if tasks_for_payload.is_empty() {
        base_plan_md.push_str("*(No tasks generated)*\n");
    } else {
        for t in &tasks_for_payload {
            let deps = if t.depends_on.is_empty() {
                String::new()
            } else {
                let dep_strs: Vec<String> = t.depends_on.iter().map(|d| d.to_string()).collect();
                format!(" [depends: {}]", dep_strs.join(", "))
            };
            base_plan_md.push_str(&format!(
                "{}. **{}** — [files: {}] [complexity: {}/10]{}\n\n",
                t.id,
                t.description,
                t.files.join(", "),
                t.estimated_complexity,
                deps
            ));
        }
    }

    // Optionally write PLAN.md (always full refined task list when paginating the tool payload).
    let written_to_disk = if params.write_to_disk {
        let plan_path = state
            .workspace_root
            .as_deref()
            .unwrap_or(std::path::Path::new("."))
            .join("PLAN.md");
        let header = format!(
            "# Vox Plan\n\n**Goal**: {}\n**Generated**: {}\n**Model**: {}\n\n",
            params.goal,
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            model_used,
        );
        let body_md = if params.plan_page_limit.is_some() {
            let mut md = format!("## Plan\n\n**Overall Summary**: {summary}\n\n### Tasks\n\n");
            if tasks.is_empty() {
                md.push_str("*(No tasks generated)*\n");
            } else {
                for t in &tasks {
                    let deps = if t.depends_on.is_empty() {
                        String::new()
                    } else {
                        let dep_strs: Vec<String> =
                            t.depends_on.iter().map(|d| d.to_string()).collect();
                        format!(" [depends: {}]", dep_strs.join(", "))
                    };
                    md.push_str(&format!(
                        "{}. **{}** — [files: {}] [complexity: {}/10]{}\n\n",
                        t.id,
                        t.description,
                        t.files.join(", "),
                        t.estimated_complexity,
                        deps
                    ));
                }
            }
            md
        } else {
            base_plan_md.clone()
        };
        let full = header + &body_md;
        std::fs::write(&plan_path, &full).is_ok()
    } else {
        false
    };

    // Build typed content_blocks from plan_md, then append clarifying questions.
    let mut content_blocks = markdown_to_content_blocks(&base_plan_md);

    let gap_report_json = loop_state
        .last_gap_report
        .as_ref()
        .and_then(|g| serde_json::to_value(g).ok());
    let last_risk = loop_state
        .last_gap_report
        .as_ref()
        .map(|g| g.aggregate_unresolved_risk);
    let clarifying = if params.questioning_hints_enabled == Some(true) {
        loop_state
            .last_gap_report
            .as_ref()
            .map(|g| g.suggested_clarifying_questions.clone())
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Append clarifying questions as structured Question blocks (after prose+task blocks).
    for q in &clarifying {
        content_blocks.push(ContentBlock::Question { text: q.clone() });
    }

    let (adeq_score, too_thin, adeq_codes) = loop_state
        .last_gap_report
        .as_ref()
        .map(|g| {
            (
                Some(g.adequacy.score),
                g.adequacy.is_too_thin,
                g.adequacy.reason_codes.clone(),
            )
        })
        .unwrap_or((None, false, Vec::new()));

    let result = PlanResult {
        goal: params.goal.clone(),
        tasks: tasks_for_payload,
        summary,
        plan_md: base_plan_md,
        written_to_disk,
        plan_total_tasks,
        plan_page_offset: page_off,
        loop_mode_effective: effective_loop_mode_label(&params),
        refinement_rounds: loop_state.refinement_rounds,
        loop_stop_reason: loop_state.stop_reason,
        last_aggregate_gap_risk: last_risk,
        gap_report: gap_report_json,
        clarifying_questions: clarifying,
        plan_adequacy_score: adeq_score,
        plan_too_thin: too_thin,
        adequacy_reason_codes: adeq_codes,
        plan_depth_effective: format!("{:?}", params.plan_depth.unwrap_or_default())
            .to_ascii_lowercase(),
        content_blocks,
    };

    let grounding = if params.scope_files.is_empty() {
        0.56_f64
    } else {
        0.74_f64
    };
    let pol = state.orchestrator_config.effective_socrates_policy();
    let session_key = plan_session_key;
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(&pol, grounding, false, turn, spent_att, max_att, None);
    spawn_socrates_telemetry_with_meta(
        state,
        "vox_plan",
        soc.clone(),
        Some(model_used.clone()),
        Some(socrates_surface_tags(
            "planning",
            &["planning", "decomposition"],
        )),
    );
    let plan_high_risk = loop_state.last_gap_report.as_ref().is_some_and(|g| {
        g.critical_count > 0 || g.adequacy.is_too_thin || g.aggregate_unresolved_risk > 0.35
    });
    if should_emit_plan_interrupt(state, "vox_plan", &session_key, 0.16, 0.28, plan_high_risk) {
        spawn_questioning_trace_from_socrates(
            state,
            "vox_plan",
            soc.clone(),
            Some(session_key.clone()),
            Some(params.goal.clone()),
        );
    }
    let mut v = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("socrates".to_string(), soc);
    }
    ToolResult::ok(v).to_json()
}

/// Replan an existing DeI plan session.
pub async fn plan_replan(state: &ServerState, params: PlanReplanParams) -> String {
    let mut v = if state.orchestrator_config.planning_llm_synthesis_enabled {
        if let Some(db) = state.db.as_ref() {
            if let Ok(sessions) = db.list_plan_sessions(50, None).await {
                if let Some(sess) = sessions
                    .into_iter()
                    .find(|s| s.plan_session_id.as_str() == params.session_id)
                {
                    let goal = sess.goal_text;
                    let prompt = format!(
                        "REPLAN GOAL: {}\n\nDELTA HINT: {}\n\nOutput a new full plan in JSON.",
                        goal, params.delta_hint
                    );
                    let resolution = McpChatModelResolution {
                        complexity: 8,
                        ..Default::default()
                    };

                    let (model, free_only) = match resolve_chat_llm_model(
                        state,
                        &prompt,
                        resolution.clone(),
                        Some(&params.session_id),
                    )
                    .await
                    {
                        Ok(pair) => pair,
                        Err(e) => {
                            return ToolResult::<serde_json::Value>::err_with_remediation(
                                e,
                                REM_DEI_DAEMON,
                            )
                            .to_json();
                        }
                    };

                    let routing = McpInferRouting {
                        user_prompt: &prompt,
                        sticky_model_pref: None,
                        resolution_template: resolution,
                        free_only,
                        allow_cloud_ollama_fallback: true,
                        user_id: Some(&params.session_id),
                    };

                    let system_prompt = crate::chat_tools::build_system_prompt(state, None).await;
                    match mcp_infer_completion(
                        state,
                        model,
                        "vox_plan_replan_fallback",
                        &system_prompt,
                        &routing,
                        4096,
                        0.2,
                        None,
                        None,
                        true,
                        None,
                    )
                    .await
                    {
                        Ok((rj, _, _)) => {
                            serde_json::from_str::<serde_json::Value>(strip_plan_json_fence(&rj))
                                .unwrap_or_else(
                                    |_| serde_json::json!({ "summary": "error", "tasks": [] }),
                                )
                        }
                        Err(e) => {
                            return ToolResult::<serde_json::Value>::err_with_remediation(
                                e.to_string(),
                                REM_DEI_DAEMON,
                            )
                            .to_json();
                        }
                    }
                } else {
                    return ToolResult::<serde_json::Value>::err_with_remediation(
                        "Session not found in DB".to_string(),
                        REM_DEI_DAEMON,
                    )
                    .to_json();
                }
            } else {
                return ToolResult::<serde_json::Value>::err_with_remediation(
                    "Failed to list plan sessions".to_string(),
                    REM_DEI_DAEMON,
                )
                .to_json();
            }
        } else {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                "Codex DB not attached".to_string(),
                REM_DEI_DAEMON,
            )
            .to_json();
        }
    } else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Planning LLM synthesis disabled".to_string(),
            REM_DEI_DAEMON,
        )
        .to_json();
    };

    // DB Persistence for the new plan version (T-023)
    if let Some(db) = state.db.as_ref() {
        if let Ok(Some(old_ver)) = db.load_plan_head(&params.session_id).await {
            let new_ver = old_ver + 1;
            let _ = db
                .append_plan_version(
                    &params.session_id,
                    new_ver,
                    Some(old_ver),
                    Some("replan"),
                    Some(&params.delta_hint),
                )
                .await;

            if let Some(tasks) = v.get("tasks").and_then(|t| t.as_array()) {
                for t_val in tasks {
                    if let Ok(node) = serde_json::from_value::<vox_orchestrator::planning::PlanNode>(
                        t_val.clone(),
                    ) {
                        let _ = db
                            .upsert_plan_node(
                                &params.session_id,
                                new_ver,
                                &node.node_id,
                                &node.description,
                                &serde_json::to_string(&node.depends_on).unwrap_or_default(),
                                &serde_json::to_string(&node.execution_policy).unwrap_or_default(),
                                match node.status {
                                    vox_orchestrator::planning::PlanStatus::Pending => "pending",
                                    vox_orchestrator::planning::PlanStatus::Queued => "queued",
                                    vox_orchestrator::planning::PlanStatus::InProgress => {
                                        "in_progress"
                                    }
                                    vox_orchestrator::planning::PlanStatus::Completed => {
                                        "completed"
                                    }
                                    vox_orchestrator::planning::PlanStatus::Failed => "failed",
                                    vox_orchestrator::planning::PlanStatus::Cancelled => {
                                        "cancelled"
                                    }
                                    vox_orchestrator::planning::PlanStatus::Superseded => {
                                        "superseded"
                                    }
                                },
                                node.workflow_invocation.as_deref(),
                            )
                            .await;
                    }
                }
            }
        }
    }

    let pol = state.orchestrator_config.effective_socrates_policy();
    let session_key =
        mcp_questioning_session_key(state, "vox_replan", Some(params.session_id.as_str()));
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(&pol, 0.62, false, turn, spent_att, max_att, None);
    spawn_socrates_telemetry_with_meta(
        state,
        "vox_replan",
        soc.clone(),
        None,
        Some(socrates_surface_tags("planning", &["planning", "replan"])),
    );
    if should_emit_plan_interrupt(state, "vox_replan", &session_key, 0.14, 0.24, false) {
        spawn_questioning_trace_from_socrates(
            state,
            "vox_replan",
            soc.clone(),
            Some(session_key.clone()),
            Some(params.delta_hint.clone()),
        );
    }
    if let Some(obj) = v.as_object_mut() {
        obj.insert("socrates".to_string(), soc);
    }
    ToolResult::ok(v).to_json()
}

/// Read structured plan session status.
pub async fn plan_status(state: &ServerState, params: PlanStatusParams) -> String {
    let mut v = serde_json::json!({ "session_id": params.session_id, "status": "active", "source": "codex_db" });

    // DB Enrichment: Load live node rows if DB is available.
    if let Some(db) = state.db.as_ref() {
        if let Ok(Some(ver)) = db.load_plan_head(&params.session_id).await {
            if let Ok(nodes) = db
                .load_plan_nodes_with_status(&params.session_id, ver)
                .await
            {
                let completed = nodes.iter().filter(|n| n.status == "completed").count();
                let failed = nodes.iter().filter(|n| n.status == "failed").count();
                let mut node_details = Vec::new();
                for n in nodes {
                    let attempts = db
                        .list_plan_node_attempts(&params.session_id, &n.node_id)
                        .await
                        .unwrap_or_default();
                    node_details.push(serde_json::json!({
                        "node_id": n.node_id,
                        "status": n.status,
                        "description": n.description.chars().take(200).collect::<String>(),
                        "workflow": n.workflow_invocation,
                        "attempts_count": attempts.len(),
                        "last_outcome": attempts.last().map(|a| a.outcome.clone()),
                        "last_error": attempts.last().and_then(|a| a.error_text.clone()),
                    }));
                }
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("nodes".to_string(), serde_json::json!(node_details));
                    obj.insert(
                        "progress".to_string(),
                        serde_json::json!({
                            "total": node_details.len(),
                            "completed": completed,
                            "failed": failed,
                        }),
                    );
                    obj.insert("plan_version".to_string(), serde_json::json!(ver));
                }
            }
        }
    }

    let pol = state.orchestrator_config.effective_socrates_policy();
    let session_key =
        mcp_questioning_session_key(state, "vox_plan_status", Some(params.session_id.as_str()));
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(&pol, 0.58, false, turn, spent_att, max_att, None);
    spawn_socrates_telemetry_with_meta(
        state,
        "vox_plan_status",
        soc.clone(),
        None,
        Some(socrates_surface_tags(
            "planning_status",
            &["planning", "status"],
        )),
    );
    if should_emit_plan_interrupt(state, "vox_plan_status", &session_key, 0.10, 0.18, false) {
        spawn_questioning_trace_from_socrates(
            state,
            "vox_plan_status",
            soc.clone(),
            Some(session_key.clone()),
            None,
        );
    }
    if let Some(obj) = v.as_object_mut() {
        obj.insert("socrates".to_string(), soc);
    }
    ToolResult::ok(v).to_json()
}

/// List all planning sessions stored in the Codex DB.
pub async fn plan_list_sessions(state: &ServerState, params: PlanListSessionsParams) -> String {
    if let Some(db) = state.db.as_ref() {
        match db
            .list_plan_sessions(params.limit.unwrap_or(50), params.status_filter.as_deref())
            .await
        {
            Ok(sessions) => ToolResult::ok(serde_json::json!(sessions)).to_json(),
            Err(e) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        }
    } else {
        ToolResult::<serde_json::Value>::err("Codex DB not attached".to_string()).to_json()
    }
}

/// Resume a planning session from the Codex DB, optionally re-writing PLAN.md.
pub async fn plan_resume(state: &ServerState, params: PlanResumeParams) -> String {
    if let Some(db) = state.db.as_ref() {
        if let Ok(Some(ver)) = db.load_plan_head(&params.session_id).await {
            match db
                .load_plan_nodes_with_status(&params.session_id, ver)
                .await
            {
                Ok(nodes) => {
                    let tasks: Vec<PlanTask> = nodes
                        .iter()
                        .map(|n| PlanTask {
                            id: n.node_id.parse().unwrap_or(0),
                            description: n.description.clone(),
                            category: None,
                            files: serde_json::from_str::<Value>(&n.execution_policy_json)
                                .map(|v| {
                                    v.get("file_manifest")
                                        .and_then(|f| f.as_array())
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                                .collect()
                                        })
                                        .unwrap_or_default()
                                })
                                .unwrap_or_default(),
                            estimated_complexity: serde_json::from_str::<Value>(
                                &n.execution_policy_json,
                            )
                            .ok()
                            .and_then(|v| v.get("estimated_complexity").and_then(|c| c.as_u64()))
                            .unwrap_or(5) as u8,
                            depends_on: serde_json::from_str(&n.dependencies_json)
                                .unwrap_or_default(),
                        })
                        .collect();

                    if params.write_to_disk {
                        // Re-synthesize PLAN.md if requested (best effort)
                        let mut md = format!("# Vox Plan Resume: {}\n\n", params.session_id);
                        for t in &tasks {
                            md.push_str(&format!(
                                "{}. **{}** — [files: {}]\n\n",
                                t.id,
                                t.description,
                                t.files.join(", ")
                            ));
                        }
                        if let Some(root) = state.workspace_root.as_deref() {
                            let _ = std::fs::write(root.join("PLAN.md"), md);
                        }
                    }

                    ToolResult::ok(serde_json::json!({
                        "session_id": params.session_id,
                        "version": ver,
                        "tasks": tasks,
                    }))
                    .to_json()
                }
                Err(e) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
            }
        } else {
            ToolResult::<serde_json::Value>::err("Plan session not found in DB".to_string())
                .to_json()
        }
    } else {
        ToolResult::<serde_json::Value>::err("Codex DB not attached".to_string()).to_json()
    }
}

#[cfg(test)]
mod adequacy_enforce_tests {
    use super::plan_gap;
    use super::plan_result_blocked_by_adequacy_enforce;
    use crate::chat_tools::params::{PlanDepth, PlanTask};
    use vox_orchestrator::OrchestratorConfig;

    fn thin_plan_tasks() -> Vec<PlanTask> {
        vec![PlanTask {
            id: 1,
            description: "do the work".into(),
            category: None,
            files: vec![],
            estimated_complexity: 8,
            depends_on: vec![],
        }]
    }

    #[test]
    fn enforce_predicate_matches_config_and_thinness() {
        let goal = "migrate authentication across crates/vox-auth, crates/vox-mcp, and update docs; add regression tests";
        let tasks = thin_plan_tasks();
        let report = plan_gap::analyze_plan_gaps(goal, 0, None, None, &tasks, None);
        assert!(
            report.adequacy.is_too_thin,
            "fixture should be thin: {:?}",
            report.adequacy
        );

        let mut cfg_on = OrchestratorConfig::for_testing();
        cfg_on.plan_adequacy_enforce = true;
        let mut cfg_off = OrchestratorConfig::for_testing();
        cfg_off.plan_adequacy_enforce = false;

        assert!(plan_result_blocked_by_adequacy_enforce(&cfg_on, &report));
        assert!(!plan_result_blocked_by_adequacy_enforce(&cfg_off, &report));
    }

    #[test]
    fn enforce_off_when_plan_not_thin() {
        let goal = "migrate authentication across crates/vox-auth, crates/vox-mcp, and update docs; add regression tests";
        let tasks: Vec<PlanTask> = (1..=6usize)
            .map(|id| PlanTask {
                id,
                description: format!(
                    "Step {id}: concrete change in crates/vox-auth or vox-mcp; run cargo test to verify"
                ),
                category: None,
                files: if id % 2 == 0 {
                    vec!["crates/vox-auth/src/lib.rs".into()]
                } else {
                    vec!["crates/vox-mcp/src/lib.rs".into()]
                },
                estimated_complexity: 5,
                depends_on: if id > 1 { vec![id - 1] } else { vec![] },
            })
            .collect();
        let report =
            plan_gap::analyze_plan_gaps(goal, 2, None, Some(PlanDepth::Deep), &tasks, None);
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.plan_adequacy_enforce = true;
        assert!(
            !plan_result_blocked_by_adequacy_enforce(&cfg, &report),
            "adequate plan must not trip enforce: {:?}",
            report.adequacy
        );
    }
}
