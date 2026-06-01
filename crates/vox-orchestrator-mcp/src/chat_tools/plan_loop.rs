//! Bounded iterative refinement for `vox_plan` (draft → gap/adequacy → expansion-first LLM refine).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::Deserialize;

use super::build_system_prompt;
use super::params::{PlanDepth, PlanLoopMode, PlanParams, PlanTask};
use super::plan_gap;
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, mcp_infer_completion};
use crate::server_state::ServerState;

use vox_orchestrator::planning::PlanRefinementReport;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanRefinementState {
    pub refinement_rounds: u32,
    pub loop_status: String,
    pub stop_reason: Option<String>,
    pub last_gap_report: Option<PlanRefinementReport>,
}

#[derive(Deserialize)]
struct PlanResponseSchema {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    tasks: Vec<PlanTask>,
}

fn hash_fingerprint(tasks: &[PlanTask]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for t in tasks {
        t.id.hash(&mut hasher);
        t.description.hash(&mut hasher);
        t.estimated_complexity.hash(&mut hasher);
        for d in &t.depends_on {
            d.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn strip_json_fence(block: &str) -> &str {
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

fn expansion_refinement_prompt(
    goal: &str,
    scope_note: &str,
    max_tasks: usize,
    prior_summary: &str,
    prior_tasks_json: &str,
    gap_json: &str,
) -> String {
    format!(
        r#"You are EXPANDING a software engineering task plan (do not throw away prior work).

GOAL: {goal}{scope_note}

Previous summary:
{prior_summary}

Previous tasks JSON (preserve each task id and intent unless a gap code requires a fix):
{prior_tasks_json}

Gap / adequacy report from automated review (address these concretely — add work, do not paraphrase away detail):
{gap_json}

Output a VALID JSON object in a ```json fenced block:
{{ "summary": "...", "tasks": [ {{ "id", "description", "files", "estimated_complexity", "depends_on" }} ] }}

Rules:
- Keep existing tasks with the same ids when they are still valid. You may tighten wording only to fix a listed gap.
- ADD new tasks for uncovered phases using NEW ids (max id + 1, …) up to {max_tasks} tasks total — split mega-steps, add verification, docs, migration, rollback.
- Fix vague or dangerous steps; repair dependency ordering.
- "depends_on" must list prerequisite task ids that appear earlier in the plan.
- Do NOT replace the whole plan with a shorter rewording unless gaps explicitly demand it.
- Do NOT include filler-only tasks (e.g. generic "review code" with no acceptance criteria)."#,
    )
}

fn refine_max_output_cap(params: &PlanParams, token_budget_remaining: u32) -> u32 {
    let reserve_tokens = 4096_u32;
    let ceiling = match params.plan_depth.unwrap_or_default() {
        PlanDepth::Minimal => 6144,
        PlanDepth::Standard => 8192,
        PlanDepth::Deep => 12288,
    };
    token_budget_remaining
        .clamp(512, ceiling)
        .max(reserve_tokens)
}

fn auto_expand_thin(params: &PlanParams) -> bool {
    params.auto_expand_thin_plan != Some(false)
}

fn effective_refine_cap(mode: PlanLoopMode, params: &PlanParams) -> u32 {
    let cfg = params.max_refine_rounds.unwrap_or(2).min(8);
    match mode {
        PlanLoopMode::Off if auto_expand_thin(params) => {
            if cfg == 0 {
                0
            } else {
                cfg.min(4)
            }
        }
        PlanLoopMode::Off => 0,
        PlanLoopMode::Auto | PlanLoopMode::Force => cfg,
    }
}

fn needs_refinement(
    mode: PlanLoopMode,
    gap_report: &PlanRefinementReport,
    risk_threshold: f32,
) -> bool {
    match mode {
        PlanLoopMode::Force => true,
        PlanLoopMode::Auto => {
            gap_report.aggregate_unresolved_risk > risk_threshold || gap_report.adequacy.is_too_thin
        }
        PlanLoopMode::Off => {
            gap_report.aggregate_unresolved_risk > risk_threshold || gap_report.adequacy.is_too_thin
        }
    }
}

/// Run bounded refinement when `loop_mode` is Auto/Force, or when `loop_mode` is Off and
/// `auto_expand_thin_plan` is not `false` and the tier‑1 report says the plan is thin/risky.
pub async fn maybe_refine_plan(
    state: &ServerState,
    params: &PlanParams,
    mut tasks: Vec<PlanTask>,
    mut summary: String,
    initial_model_complexity: u8,
    session_label: &str,
) -> (Vec<PlanTask>, String, PlanRefinementState) {
    let mode = params.loop_mode.unwrap_or(PlanLoopMode::Off);
    let risk_threshold = params
        .gap_risk_threshold
        .unwrap_or(0.28_f32)
        .clamp(0.05, 0.95);
    let token_budget = params.refine_budget_tokens.unwrap_or(18_000_u32);
    let max_rounds = effective_refine_cap(mode, params);
    let scope_file_count = params.scope_files.len();

    let starting_report = plan_gap::analyze_plan_gaps(
        &params.goal,
        scope_file_count,
        None,
        params.plan_depth,
        &tasks,
        None,
    );

    if max_rounds == 0 {
        return (
            tasks,
            summary,
            PlanRefinementState {
                refinement_rounds: 0,
                loop_status: "off".to_string(),
                stop_reason: None,
                last_gap_report: Some(starting_report),
            },
        );
    }

    let mut rounds: u32 = 0;
    let mut tokens_spent: u32 = 0;
    let mut prev_fingerprint: Option<u64> = None;
    let mut oscillation_strikes: u8 = 0;
    let mut last_gap: Option<PlanRefinementReport>;

    loop {
        let gap_report = plan_gap::analyze_plan_gaps(
            &params.goal,
            scope_file_count,
            None,
            params.plan_depth,
            &tasks,
            None,
        );
        last_gap = Some(gap_report.clone());

        let tasks_before_refine_pass = tasks.clone();

        let should_refine =
            rounds < max_rounds && needs_refinement(mode, &gap_report, risk_threshold);

        if !should_refine {
            let stop = if rounds == 0 {
                None
            } else if rounds >= max_rounds {
                Some("max_refine_rounds".to_string())
            } else if gap_report.aggregate_unresolved_risk <= risk_threshold
                && !gap_report.adequacy.is_too_thin
            {
                Some("ready_below_threshold".to_string())
            } else {
                Some("ready".to_string())
            };
            return (
                tasks,
                summary,
                PlanRefinementState {
                    refinement_rounds: rounds,
                    loop_status: "ready".to_string(),
                    stop_reason: stop,
                    last_gap_report: last_gap,
                },
            );
        }

        if token_budget > 0 && tokens_spent >= token_budget {
            return (
                tasks,
                summary,
                PlanRefinementState {
                    refinement_rounds: rounds,
                    loop_status: "stopped_budget".to_string(),
                    stop_reason: Some("token_budget".to_string()),
                    last_gap_report: last_gap,
                },
            );
        }

        let scope_note = if params.scope_files.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nScope this plan to these files:\n{}",
                params.scope_files.join("\n")
            )
        };
        let max_tasks = params.max_tasks.unwrap_or(30);
        let prior_tasks_json = serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".into());
        let gap_json = serde_json::to_string(&gap_report).unwrap_or_else(|_| "{}".into());
        let user_prompt = expansion_refinement_prompt(
            &params.goal,
            &scope_note,
            max_tasks,
            &summary,
            &prior_tasks_json,
            &gap_json,
        );

        let resolution_template = McpChatModelResolution {
            complexity: initial_model_complexity.max(8),
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
            Err(_) => {
                return (
                    tasks,
                    summary,
                    PlanRefinementState {
                        refinement_rounds: rounds,
                        loop_status: "stopped_model_error".to_string(),
                        stop_reason: Some("model_resolve_failed".to_string()),
                        last_gap_report: last_gap,
                    },
                );
            }
        };

        let pref = match crate::sync_poison::poison_rw_read(
            state.mcp_chat_model_override.read(),
            "mcp_chat_model_override",
        ) {
            Ok(g) => g.clone(),
            Err(_) => {
                return (
                    tasks,
                    summary,
                    PlanRefinementState {
                        refinement_rounds: rounds,
                        loop_status: "stopped_model_error".to_string(),
                        stop_reason: Some("model_lock".to_string()),
                        last_gap_report: last_gap,
                    },
                );
            }
        };

        let routing = McpInferRouting {
            user_prompt: &user_prompt,
            sticky_model_pref: pref.as_deref(),
            resolution_template: resolution_template.clone(),
            free_only,
            allow_cloud_ollama_fallback: true,
            selection_rationale: None,
            user_id: params.session_id.as_deref(),
        };

        let system_prompt = build_system_prompt(state, None).await;
        let max_out = refine_max_output_cap(params, token_budget.saturating_sub(tokens_spent));

        let mut used_total: u64;
        let mut response_json = match mcp_infer_completion(
            state,
            model.clone(),
            "vox_plan_refine",
            &system_prompt,
            &routing,
            u64::from(max_out),
            0.25,
            None,
            None,
            true,
            None,
        )
        .await
        {
            Ok((rj, _mu, ut)) => {
                used_total = ut;
                rj
            }
            Err(_) => {
                return (
                    tasks,
                    summary,
                    PlanRefinementState {
                        refinement_rounds: rounds,
                        loop_status: "stopped_llm_error".to_string(),
                        stop_reason: Some("completion_failed".to_string()),
                        last_gap_report: last_gap,
                    },
                );
            }
        };

        let parsed: PlanResponseSchema = match serde_json::from_str(strip_json_fence(
            &response_json,
        )) {
            Ok(p) => p,
            Err(e) => {
                let snippet: String = response_json.chars().take(1600).collect();
                let fix_prompt = format!(
                    r#"Your previous refinement output was not valid JSON (parse error: {err}).

Output ONLY a ```json fenced block: a single object with "summary" and "tasks" (same schema as before). No prose.

Broken output (may be truncated):
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
                    selection_rationale: None,
                    user_id: params.session_id.as_deref(),
                };
                let retry_cap = u64::from(max_out).max(8192);
                match mcp_infer_completion(
                    state,
                    model.clone(),
                    "vox_plan_refine_retry_json",
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
                    Ok((rj2, _m2, ut2)) => {
                        used_total = used_total.saturating_add(ut2);
                        response_json = rj2;
                        match serde_json::from_str(strip_json_fence(&response_json)) {
                            Ok(p) => p,
                            Err(_) => {
                                return (
                                    tasks,
                                    summary,
                                    PlanRefinementState {
                                        refinement_rounds: rounds,
                                        loop_status: "stopped_parse_error".to_string(),
                                        stop_reason: Some("invalid_refine_json".to_string()),
                                        last_gap_report: last_gap,
                                    },
                                );
                            }
                        }
                    }
                    Err(_) => {
                        return (
                            tasks,
                            summary,
                            PlanRefinementState {
                                refinement_rounds: rounds,
                                loop_status: "stopped_parse_error".to_string(),
                                stop_reason: Some("invalid_refine_json".to_string()),
                                last_gap_report: last_gap,
                            },
                        );
                    }
                }
            }
        };

        tokens_spent = tokens_spent.saturating_add(u32::try_from(used_total).unwrap_or(u32::MAX));

        tracing::debug!(
            target = "vox_mcp::plan_loop",
            label = %session_label,
            round = rounds,
            tokens = used_total,
            "iterative plan refinement pass"
        );

        if !parsed.tasks.is_empty() {
            tasks = parsed.tasks;
        }
        if !parsed.summary.is_empty() {
            summary = parsed.summary;
        }

        last_gap = Some(plan_gap::analyze_plan_gaps(
            &params.goal,
            scope_file_count,
            None,
            params.plan_depth,
            &tasks,
            Some(tasks_before_refine_pass.as_slice()),
        ));

        let fp = hash_fingerprint(&tasks);
        if let Some(prev) = prev_fingerprint {
            if prev == fp {
                oscillation_strikes = oscillation_strikes.saturating_add(1);
            } else {
                oscillation_strikes = 0;
            }
            if oscillation_strikes >= 1 {
                return (
                    tasks,
                    summary,
                    PlanRefinementState {
                        refinement_rounds: rounds.saturating_add(1),
                        loop_status: "stopped_plateau".to_string(),
                        stop_reason: Some("oscillation".to_string()),
                        last_gap_report: last_gap,
                    },
                );
            }
        }
        prev_fingerprint = Some(fp);

        rounds = rounds.saturating_add(1);
    }
}
