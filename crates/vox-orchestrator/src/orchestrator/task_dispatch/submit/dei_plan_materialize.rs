//! Shared plan synthesis + Codex persistence for DeI `ai.plan.*` stdio RPC (`vox-orchestrator-d`).
//!
//! Intentionally does **not** enqueue runnable tasks; [`crate::planning::schedule::enqueue_runnable_plan_nodes`]
//! is invoked from `ai.plan.execute` only.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::json;

use super::super::super::{Orchestrator, OrchestratorError};
use crate::planning::intake_router::evaluate_goal;
use crate::planning::{PlanNode, PlanningMode, PlanningStrategy, RouterEvaluation};
use crate::types::FileAffinity;

fn merge_file_affinities(into: &mut Vec<FileAffinity>, extra: &[FileAffinity]) {
    let mut have: HashSet<(std::path::PathBuf, crate::types::AccessKind)> =
        into.iter().map(|f| (f.path.clone(), f.access)).collect();
    for f in extra {
        let key = (f.path.clone(), f.access);
        if have.insert(key) {
            into.push(f.clone());
        }
    }
}

async fn synthesized_plan_nodes(
    orch: &Orchestrator,
    goal: &str,
    scope_files: &[String],
) -> Result<(Vec<PlanNode>, RouterEvaluation), OrchestratorError> {
    let cfg = crate::sync_lock::rw_read(&*orch.config).clone();
    let eval = evaluate_goal(&cfg, goal, Some(PlanningMode::ForcePlan));
    if eval.strategy == PlanningStrategy::WorkflowHandoff {
        return Err(OrchestratorError::ScopeDenied(
            "ai.plan: workflow-handoff goals are not supported on the stdio DeI surface; use MCP workflows."
                .into(),
        ));
    }

    let file_manifest: Vec<FileAffinity> = scope_files
        .iter()
        .map(|p| FileAffinity::read(PathBuf::from(p.trim())))
        .filter(|fa| !fa.path.as_os_str().is_empty())
        .collect();

    let socrates_ctx = orch
        .generate_goal_search_context(goal, &file_manifest)
        .await;
    let mut nodes = if cfg.planning_llm_synthesis_enabled {
        #[cfg(feature = "runtime")]
        {
            let maybe_llm_cfg = crate::sync_lock::rw_read(&*orch.models).get_llm_config(
                crate::types::TaskCategory::Planning,
                2,
                crate::config::CostPreference::Performance,
            );
            if let Some(mut llm_cfg) = maybe_llm_cfg {
                llm_cfg.temperature = Some(vox_config::gemini_tuning_temperature().unwrap_or(0.2));
                llm_cfg.top_p = vox_config::gemini_tuning_top_p();
                let depth_str = format!("{:?}", cfg.planning_depth);
                crate::planning::synthesizer::synthesize_plan_nodes_with_llm(
                    goal,
                    &depth_str,
                    |sys, user| {
                        let sys_msg = vox_actor_runtime::llm::LlmChatMessage {
                            role: "system".into(),
                            content: sys.into(),
                            ..Default::default()
                        };
                        let user_msg = vox_actor_runtime::llm::LlmChatMessage {
                            role: "user".into(),
                            content: user.into(),
                            ..Default::default()
                        };
                        let cfg_clone = llm_cfg.clone();
                        async move {
                            let opts =
                                vox_actor_runtime::ActivityOptions::new().with_timeout_secs(45);
                            match vox_actor_runtime::llm::llm_chat(
                                &opts,
                                vec![sys_msg, user_msg],
                                cfg_clone,
                            )
                            .await
                            {
                                vox_actor_runtime::ActivityResult::Ok(Ok(res)) => Ok(res.content),
                                vox_actor_runtime::ActivityResult::Ok(Err(e)) => Err(e),
                                _ => Err("activity_failed".to_string()),
                            }
                        }
                    },
                )
                .await
            } else {
                crate::planning::synthesizer::synthesize_plan_nodes(goal)
            }
        }
        #[cfg(not(feature = "runtime"))]
        {
            crate::planning::synthesizer::synthesize_plan_nodes(goal)
        }
    } else {
        crate::planning::synthesizer::synthesize_plan_nodes(goal)
    };

    let cfg_research = cfg.research_model_enabled;
    let socrates_ctx_clone = socrates_ctx.clone();
    for n in &mut nodes {
        let mut h = crate::types::TaskEnqueueHints::default();
        let mut soc = socrates_ctx_clone.clone();
        if cfg_research {
            soc.research_model_enabled = true;
        }
        h.socrates_context = Some(soc);
        n.execution_policy.enqueue_hints = Some(h);
        if !file_manifest.is_empty() {
            merge_file_affinities(&mut n.execution_policy.file_manifest, &file_manifest);
        }
    }
    crate::planning::quality_gate::validate_plan_nodes(&nodes)?;
    let adeq_tasks = crate::planning::plan_nodes_to_adequacy_tasks(&nodes);
    let adeq_report = crate::planning::analyze_plan_refinement_report(
        goal,
        file_manifest.len(),
        Some(eval.complexity),
        0,
        &adeq_tasks,
        socrates_ctx.fatigue_active,
    );
    if adeq_report.adequacy.is_too_thin && cfg.plan_adequacy_enforce {
        return Err(OrchestratorError::ScopeDenied(format!(
            "Plan adequacy gate: synthesized plan is too thin (score {:.2}, reasons {:?}).",
            adeq_report.adequacy.score, adeq_report.adequacy.reason_codes
        )));
    }

    Ok((nodes, eval))
}

fn steps_json(nodes: &[PlanNode]) -> Vec<serde_json::Value> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            json!({
                "id": i + 1,
                "description": n.description,
                "test_decision": "Deferred",
            })
        })
        .collect()
}

/// Re-synthesize nodes for an existing session/version (after `append_plan_version`).
pub async fn dei_plan_rematerialize_existing(
    orch: &Orchestrator,
    plan_session_id: &str,
    plan_version: i64,
    goal: String,
    scope_files: Vec<String>,
) -> Result<serde_json::Value, OrchestratorError> {
    let (nodes, eval) = synthesized_plan_nodes(orch, &goal, &scope_files).await?;
    let db_handle = orch.db();
    let Some(db) = db_handle.as_ref() else {
        return Err(OrchestratorError::DatabaseError(
            "Codex DB not attached".into(),
        ));
    };
    let strategy = format!("{:?}", eval.strategy);
    let _ = db
        .update_plan_session_goal_text(plan_session_id, &goal)
        .await;
    let _ = db
        .create_plan_session(plan_session_id, None, &goal, &strategy)
        .await;
    for n in &nodes {
        let deps_json = serde_json::to_string(&n.depends_on).unwrap_or_else(|_| "[]".to_string());
        let pol_json =
            serde_json::to_string(&n.execution_policy).unwrap_or_else(|_| "{}".to_string());
        let _ = db
            .upsert_plan_node(
                plan_session_id,
                plan_version,
                &n.node_id,
                &n.description,
                &deps_json,
                &pol_json,
                "pending",
                n.workflow_invocation.as_deref(),
            )
            .await;
    }
    Ok(json!({
        "session_id": plan_session_id,
        "goal": goal,
        "summary": format!("{} plan node(s) rematerialized", nodes.len()),
        "versions": [{ "version": plan_version, "steps": steps_json(&nodes) }],
    }))
}

/// Synthesize plan nodes, persist to Codex when a DB handle exists, return CLI-shaped JSON.
pub async fn dei_plan_new_json(
    orch: &Orchestrator,
    goal: String,
    origin_session_id: Option<String>,
    scope_files: Vec<String>,
) -> Result<serde_json::Value, OrchestratorError> {
    let (nodes, eval) = synthesized_plan_nodes(orch, &goal, &scope_files).await?;
    let plan_session_id = format!("plan-{}", uuid::Uuid::new_v4());
    let plan_version = 1_i64;
    if let Some(db) = orch.db().as_ref() {
        let strategy = format!("{:?}", eval.strategy);
        let _ = db
            .create_plan_session(
                &plan_session_id,
                origin_session_id.as_deref(),
                &goal,
                &strategy,
            )
            .await;
        let _ = db
            .append_plan_version(&plan_session_id, plan_version, None, None, None)
            .await;
        for n in &nodes {
            let deps_json =
                serde_json::to_string(&n.depends_on).unwrap_or_else(|_| "[]".to_string());
            let pol_json =
                serde_json::to_string(&n.execution_policy).unwrap_or_else(|_| "{}".to_string());
            let _ = db
                .upsert_plan_node(
                    &plan_session_id,
                    plan_version,
                    &n.node_id,
                    &n.description,
                    &deps_json,
                    &pol_json,
                    "pending",
                    n.workflow_invocation.as_deref(),
                )
                .await;
        }
    }

    Ok(json!({
        "session_id": plan_session_id,
        "goal": goal,
        "summary": format!("{} plan node(s) materialized", nodes.len()),
        "versions": [{ "version": plan_version, "steps": steps_json(&nodes) }],
    }))
}
