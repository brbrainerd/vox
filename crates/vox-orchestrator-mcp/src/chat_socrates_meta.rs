//! Socrates grounding snippets and telemetry for chat / inline / ghost tools.
//!
//! Rows written via `spawn_socrates_telemetry_with_meta` → `vox_db::VoxDb::record_socrates_surface_event`
//! (or `spawn_socrates_telemetry_with_llm` → `vox_db::VoxDb::record_unified_llm_turn`) are **operator /
//! research diagnostics** (aggregated risk/confidence/contradiction — see `vox_db::socrates_telemetry` rustdoc), not end-user
//! usage analytics. Questioning expansions use separate `question_*` tables.

use serde::Deserialize;
use serde_json::Value;
use vox_actor_runtime::supervisor::spawn_supervised_infallible;
use vox_orchestrator_types::socrates_policy::{
    ClarificationStopReason, ConfidencePolicy, QuestionCandidate, QuestionKind, QuestioningPolicy,
    RiskDecision,
};

use crate::attention_policy::{channel_label, decision_label, evaluate_with_state};
use crate::server_state::ServerState;

/// Optional LLM turn metrics recorded together with Socrates surface telemetry (single DB transaction).
#[derive(Debug, Clone)]
pub(crate) struct LlmSurfaceTelemetry {
    pub session_id: String,
    pub user_id: Option<String>,
    #[allow(dead_code)]
    pub tenant_id: Option<String>,
    pub prompt: String,
    pub response: String,
    pub model_id: String,
    pub provider: String,
    pub task_category: String,
    pub strength_tag: String,
    pub latency_ms: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub trace_id: Option<String>,
    pub success: bool,
    pub cost_usd: Option<f64>,
    pub quality_score: Option<f64>,
}

/// JSON shape of the `socrates` field returned to MCP clients (must match [`socrates_tool_meta`]).
#[derive(Debug, Deserialize)]
pub(crate) struct SocratesJsonMeta {
    pub(crate) risk_decision: RiskDecision,
    pub(crate) confidence_estimate: f64,
    pub(crate) contradiction_ratio: f64,
    #[serde(default)]
    pub(crate) _fact_check: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) questioning: Option<QuestioningJsonMeta>,
    #[serde(default)]
    pub(crate) _search_refinement: Option<SearchRefinementJsonMeta>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuestioningJsonMeta {
    pub(crate) question_needed: bool,
    #[serde(default)]
    pub(crate) question_kind: Option<QuestionKind>,
    #[serde(default)]
    pub(crate) prompt: Option<String>,
    #[serde(default)]
    pub(crate) expected_information_gain_bits: f64,
    #[serde(default)]
    pub(crate) expected_user_cost: f64,
    #[serde(default)]
    pub(crate) utility_bits_per_cost: f64,
    #[serde(default)]
    pub(crate) stop_reason: Option<ClarificationStopReason>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchRefinementJsonMeta {
    #[serde(default)]
    pub(crate) _recommended_action: Option<String>,
    #[serde(default)]
    pub(crate) _reason: Option<String>,
    #[serde(default)]
    pub(crate) _verification_performed: bool,
    #[serde(default)]
    pub(crate) _verification_reason: Option<String>,
}

#[must_use]
pub(crate) fn socrates_system_rider(policy: &ConfidencePolicy) -> String {
    let p = policy;
    format!(
        "\n## Socrates (grounding & diagnostic questioning)\n\
         - Below {:.0}% calibrated confidence: do not speculate; state what evidence is missing.\n\
         - {:.0}–{:.0}%: answer with explicit uncertainty or ask ONE focused clarifying question.\n\
         - Above {:.0}%: answer normally; tie claims to files or tools you used.\n\
         When you do ask, ask the single most diagnostic question:\n\
         1. Enumerate the 2–4 candidate solutions/interpretations consistent with the request so far.\n\
         2. Ask the one question whose answer best SPLITS those candidates (maximizes information gain over solutions, not over phrasings).\n\
         3. Separate *specification* uncertainty (what YOU want — a question can resolve this) from *model* uncertainty (what I'm unsure of — a question usually cannot). Only ask about the former.\n\
         4. Prefer a bounded multiple-choice over open-ended when the candidate set is known; never ask what context already implies.\n",
        p.abstain_threshold * 100.0,
        p.abstain_threshold * 100.0,
        p.ask_for_help_threshold * 100.0,
        p.ask_for_help_threshold * 100.0,
    )
}

#[allow(dead_code)]
pub(crate) fn spawn_socrates_telemetry(
    state: &ServerState,
    surface: &'static str,
    socrates_value: Value,
    model_used: Option<String>,
) {
    spawn_socrates_telemetry_with_llm(state, surface, socrates_value, model_used, None, None);
}

#[must_use]
pub(crate) fn socrates_surface_tags(task_class: &str, domain_tags: &[&str]) -> Value {
    let tags = domain_tags
        .iter()
        .copied()
        .filter(|s| !s.is_empty())
        .map(serde_json::Value::from)
        .collect::<Vec<_>>();
    serde_json::json!({
        "task_class": task_class,
        "domain_tags": tags,
    })
}

/// Persist Socrates surface aggregates to `research_metrics` / socrates telemetry tables (best-effort async).
pub(crate) fn spawn_socrates_telemetry_with_meta(
    state: &ServerState,
    surface: &'static str,
    socrates_value: Value,
    model_used: Option<String>,
    retrieval_meta: Option<Value>,
) {
    spawn_socrates_telemetry_with_llm(
        state,
        surface,
        socrates_value,
        model_used,
        retrieval_meta,
        None,
    );
}

/// Like `spawn_socrates_telemetry_with_meta`, but optionally records [`LlmSurfaceTelemetry`] in the same transaction as Socrates.
pub(crate) fn spawn_socrates_telemetry_with_llm(
    state: &ServerState,
    surface: &'static str,
    socrates_value: Value,
    model_used: Option<String>,
    retrieval_meta: Option<Value>,
    llm_turn: Option<LlmSurfaceTelemetry>,
) {
    let Some(db) = state.db.clone() else {
        return;
    };
    let repository_id = state.repository.repository_id.clone();
    spawn_supervised_infallible("socrates_telemetry", async move {
        let meta = match serde_json::from_value::<SocratesJsonMeta>(socrates_value.clone()) {
            Ok(m) => m,
            Err(e) => {
                let payload = serde_json::to_string(&socrates_value)
                    .unwrap_or_else(|_| "<non-serializable>".into());
                let snippet: String = payload.chars().take(400).collect();
                tracing::warn!(
                    surface,
                    error = %e,
                    payload_snippet = %snippet,
                    "socrates telemetry: JSON shape mismatch (must match socrates_tool_meta)"
                );
                return;
            }
        };

        if let Some(l) = llm_turn {
            let outcome = vox_db::store::types::ModelOutcome {
                session_id: &l.session_id,
                user_id: l.user_id.as_deref(),
                prompt: &l.prompt,
                response: &l.response,
                model_id: &l.model_id,
                provider: &l.provider,
                task_category: &l.task_category,
                strength_tag: &l.strength_tag,
                latency_ms: Some(l.latency_ms),
                input_tokens: l.input_tokens,
                output_tokens: l.output_tokens,
                cache_read_tokens: l.cache_read_tokens,
                trace_id: l.trace_id.as_deref(),
                tenant_id: None,
                context_utilization_pct: None,
                success: l.success,
                cost_usd: l.cost_usd,
                quality_score: l.quality_score,
                ttft_ms: None,
                tpot_ms: None,
            };
            match db
                .record_unified_llm_turn(
                    outcome,
                    Some((
                        repository_id.clone(),
                        surface.to_string(),
                        meta.risk_decision,
                        meta.confidence_estimate,
                        meta.contradiction_ratio,
                        model_used,
                        retrieval_meta,
                    )),
                )
                .await
            {
                Ok(ids) => {
                    tracing::info!(
                        target: "vox_socrates_telemetry",
                        llm_row_id = ids.llm_interaction_rowid,
                        socrates_row_id = ?ids.socrates_research_metric_rowid,
                        surface,
                        repository_id = %repository_id,
                        decision = ?meta.risk_decision,
                        "persisted unified llm + socrates_surface"
                    );
                }
                Err(e) => tracing::warn!(
                    error = %e,
                    surface,
                    "unified llm + socrates telemetry insert failed"
                ),
            }
            return;
        }

        match db
            .record_socrates_surface_event(
                &repository_id,
                surface,
                meta.risk_decision,
                meta.confidence_estimate,
                meta.contradiction_ratio,
                model_used.as_deref(),
                retrieval_meta,
            )
            .await
        {
            Ok(id) => {
                tracing::info!(
                    target: "vox_socrates_telemetry",
                    row_id = id,
                    surface,
                    repository_id = %repository_id,
                    decision = ?meta.risk_decision,
                    "persisted socrates_surface"
                );
            }
            Err(e) => tracing::warn!(
                error = %e,
                surface,
                "socrates telemetry insert failed"
            ),
        }
    });
}

pub(crate) fn spawn_questioning_trace_from_socrates(
    state: &ServerState,
    surface: &'static str,
    socrates_value: Value,
    session_id: Option<String>,
    prompt_hint: Option<String>,
) {
    let Some(db) = state.db.clone() else {
        return;
    };
    let spend_state = state.clone();
    let repository_id = state.repository.repository_id.clone();
    spawn_supervised_infallible("questioning_trace", async move {
        let meta = match serde_json::from_value::<SocratesJsonMeta>(socrates_value.clone()) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    surface,
                    error = %e,
                    "questioning trace skipped: invalid socrates payload"
                );
                return;
            }
        };
        let Some(q) = meta.questioning else {
            return;
        };

        let session_key = session_id.unwrap_or_else(|| format!("mcp:{repository_id}:{surface}"));
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let open = match db
            .find_open_question_session_for_repo(&session_key, &repository_id)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(surface, error = %e, "questioning trace: open_session lookup failed");
                None
            }
        };

        if q.question_needed {
            if db
                .has_pending_clarification_for_mcp_session(&session_key, &repository_id)
                .await
                .unwrap_or(false)
            {
                let _ = db
                    .record_questioning_metric(
                        &session_key,
                        Some(q.expected_information_gain_bits),
                        &serde_json::json!({
                            "surface": surface,
                            "question_needed": true,
                            "deferred": true,
                            "reason": "pending_unanswered_clarification",
                        })
                        .to_string(),
                    )
                    .await;
                return;
            }

            let question_session_id = if let Some(ref row) = open {
                row.id
            } else {
                match db
                    .create_question_session(vox_db::QuestionSessionCreateParams {
                        session_id: &session_key,
                        repository_id: &repository_id,
                        task_id: None,
                        policy_version: "v1",
                        started_at_ms: now_ms,
                    })
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::warn!(surface, error = %e, "questioning trace: create_session failed");
                        return;
                    }
                }
            };

            let turn_index = match db.next_question_event_turn_index(question_session_id).await {
                Ok(t) => i64::from(t),
                Err(e) => {
                    tracing::warn!(surface, error = %e, "questioning trace: turn_index failed");
                    return;
                }
            };

            let turn_index_u32 = turn_index.max(0) as u32;
            let questioning_defaults = QuestioningPolicy::default();
            let bm = spend_state.orchestrator.budget_manager_handle();
            let att_snap = vox_orchestrator::sync_lock::rw_read(&*bm).attention_snapshot();
            let pending_backlog = db
                .count_pending_clarifications_for_mcp_session(&session_key, &repository_id)
                .await
                .unwrap_or(0);
            let actor_agent_id = spend_state
                .orchestrator
                .agent_for_session_id(&session_key)
                .or_else(|| {
                    open.as_ref()
                        .and_then(|row| row.task_id.as_ref().map(|t| t.as_str()))
                        .and_then(|tid| tid.parse::<u64>().ok())
                        .and_then(|tid| {
                            spend_state
                                .orchestrator
                                .agent_assigned_to_task(vox_orchestrator::TaskId(tid))
                        })
                })
                .unwrap_or(vox_orchestrator::AgentId(0));
            let trust = vox_orchestrator::sync_lock::rw_read(&*bm)
                .trust_snapshot()
                .get(&actor_agent_id)
                .map(|t| t.trust_score)
                .unwrap_or(0.3);

            let signals = vox_orchestrator::InterruptionSignals {
                channel: interruption_channel_for_surface(surface),
                expected_information_gain_bits: q.expected_information_gain_bits,
                expected_user_cost: q.expected_user_cost,
                confidence_estimate: meta.confidence_estimate,
                contradiction_ratio: meta.contradiction_ratio,
                pending_clarification_backlog: pending_backlog,
                clarification_turn_index: turn_index_u32,
                max_clarification_turns: questioning_defaults.max_clarification_turns,
                irreversible_or_high_risk: matches!(meta.risk_decision, RiskDecision::Abstain)
                    || meta.contradiction_ratio > 0.35,
                base_interrupt_cost_ms: spend_state.orchestrator_config.attention_interrupt_cost_ms,
                trust_score: trust,
                open_question_session: open.is_some(),
                spec_uncertainty: 0.0,
                model_uncertainty: 0.0,
            };

            let decision = evaluate_with_state(&spend_state, &signals, &att_snap);

            match &decision {
                vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint { reason }
                | vox_orchestrator::InterruptionDecision::BatchWithExistingPrompt { reason } => {
                    let ts_evt = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let evt = vox_orchestrator::AttentionEvent {
                        agent_id: actor_agent_id,
                        task_id: None,
                        event_type: vox_orchestrator::AttentionEventType::PolicyDeferred,
                        tier: vox_orchestrator::ApprovalTier::Confirm,
                        cost_ms: 0,
                        outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                        trust_score_at_time: trust,
                        effective_complexity: (q.expected_user_cost * 10.0).clamp(0.0, 10.0),
                        decision_entropy_bits: q.expected_information_gain_bits,
                        timestamp_ms: ts_evt,
                        channel: Some(surface.to_string()),
                        policy_reason: Some(reason.clone()),
                    };
                    spend_state.record_attention_event(evt);
                    let withheld = Some(withheld_question(
                        q.prompt.as_deref().unwrap_or("Clarify intent"),
                        reason,
                        q.expected_information_gain_bits,
                    ));
                    let _ = db
                        .record_questioning_metric(
                            &session_key,
                            Some(q.expected_information_gain_bits),
                            &questioning_policy_metric_payload(
                                surface,
                                signals.channel,
                                true,
                                "deferred",
                                reason,
                                &decision,
                                ts_evt,
                                withheld,
                            )
                            .to_string(),
                        )
                        .await;
                    return;
                }
                vox_orchestrator::InterruptionDecision::ProceedAutonomously { reason } => {
                    let ts_evt = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let evt = vox_orchestrator::AttentionEvent {
                        agent_id: actor_agent_id,
                        task_id: None,
                        event_type: vox_orchestrator::AttentionEventType::PolicyProceedAuto,
                        tier: vox_orchestrator::ApprovalTier::AutoApprove,
                        cost_ms: 0,
                        outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                        trust_score_at_time: trust,
                        effective_complexity: (q.expected_user_cost * 10.0).clamp(0.0, 10.0),
                        decision_entropy_bits: q.expected_information_gain_bits,
                        timestamp_ms: ts_evt,
                        channel: Some(surface.to_string()),
                        policy_reason: Some(reason.clone()),
                    };
                    spend_state.record_attention_event(evt);
                    let withheld = Some(withheld_question(
                        q.prompt.as_deref().unwrap_or("Clarify intent"),
                        reason,
                        q.expected_information_gain_bits,
                    ));
                    let _ = db
                        .record_questioning_metric(
                            &session_key,
                            Some(q.expected_information_gain_bits),
                            &questioning_policy_metric_payload(
                                surface,
                                signals.channel,
                                false,
                                "proceed_auto",
                                reason,
                                &decision,
                                ts_evt,
                                withheld,
                            )
                            .to_string(),
                        )
                        .await;
                    return;
                }
                _ => {}
            }

            let scaled_cost = decision.scaled_cost_ms().max(1);

            let kind = q.question_kind.unwrap_or(QuestionKind::OpenEnded);
            let kind_str = match kind {
                QuestionKind::MultipleChoice => "multiple_choice",
                QuestionKind::OpenEnded => "open_ended",
                QuestionKind::Entry => "entry",
            };
            let prompt = q
                .prompt
                .clone()
                .or(prompt_hint)
                .unwrap_or_else(|| "Clarify intent".to_string());
            let question_id = format!("{surface}:{question_session_id}:{now_ms}");
            let utility = if q.utility_bits_per_cost > 0.0 {
                q.utility_bits_per_cost
            } else {
                q.expected_information_gain_bits / q.expected_user_cost.max(1e-6)
            };
            let event_row_id = match db
                .insert_question_event(vox_db::QuestionEventParams {
                    question_session_id,
                    question_id: &question_id,
                    turn_index,
                    actor: "assistant",
                    question_kind: kind_str,
                    prompt: &prompt,
                    expected_information_gain_bits: q.expected_information_gain_bits,
                    expected_user_cost: q.expected_user_cost,
                    utility_bits_per_cost: utility,
                    answer_text: None,
                    answer_type: None,
                    answered_at_ms: None,
                    created_at_ms: now_ms,
                })
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(surface, error = %e, "questioning trace: insert_question_event failed");
                    return;
                }
            };

            if kind == QuestionKind::MultipleChoice {
                let p = 1.0_f64 / 3.0_f64;
                let opts: [(&str, &str, bool); 3] = [
                    ("a", "First listed intent / primary interpretation", false),
                    ("b", "Alternative scope or constraint setup", false),
                    ("c", "Other — describe in chat", true),
                ];
                for (oid, label, is_other) in opts {
                    let _ = db
                        .upsert_question_option(vox_db::QuestionOptionParams {
                            question_event_id: event_row_id,
                            option_id: oid,
                            label,
                            prior_probability: Some(p),
                            posterior_probability: None,
                            is_other,
                        })
                        .await;
                }
            }

            let ts_evt = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let attention_evt = vox_orchestrator::AttentionEvent {
                agent_id: actor_agent_id,
                task_id: None,
                event_type: vox_orchestrator::AttentionEventType::A2AInterrupt,
                tier: vox_orchestrator::ApprovalTier::Confirm,
                cost_ms: scaled_cost,
                outcome: vox_orchestrator::ApprovalOutcome::Approved,
                trust_score_at_time: trust,
                effective_complexity: (q.expected_user_cost * 10.0).clamp(0.0, 10.0),
                decision_entropy_bits: q.expected_information_gain_bits,
                timestamp_ms: ts_evt,
                channel: Some(surface.to_string()),
                policy_reason: Some(format!("{decision:?}")),
            };
            spend_state.record_clarification_interrupt(&session_key, scaled_cost, attention_evt);

            let msg_uuid = format!("clarification-{surface}-{question_session_id}-{now_ms}");
            let _ = db
                .send_a2a_clarification_message(vox_db::A2aClarificationMessageParams {
                    message_uuid: &msg_uuid,
                    sender_agent: "mcp_chat",
                    receiver_agent: "orchestrator_clarifier",
                    msg_type: "clarification_request",
                    repository_id: &repository_id,
                    thread_id: Some(&session_key),
                    priority: 5,
                    clarification_intent: "disambiguate_user_intent",
                    hypothesis_set_id: &question_id,
                    question_kind: Some(kind_str),
                    expected_information_gain_bits: Some(q.expected_information_gain_bits),
                    expected_user_cost: Some(q.expected_user_cost),
                    requested_evidence_dimensions_json: None,
                    urgency: Some("normal"),
                    stop_policy_json: Some(r#"{"max_turns":3,"min_gain_bits":0.08}"#),
                })
                .await;

            let _ = db
                .record_questioning_metric(
                    &session_key,
                    Some(q.expected_information_gain_bits),
                    &serde_json::json!({
                        "surface": surface,
                        "question_needed": true,
                        "question_kind": kind_str,
                        "question_id": question_id,
                        "turn_index": turn_index,
                        "expected_information_gain_bits": q.expected_information_gain_bits,
                        "expected_user_cost": q.expected_user_cost,
                        "utility_bits_per_cost": utility,
                    })
                    .to_string(),
                )
                .await;
            return;
        }

        if let Some(stop_reason) = q.stop_reason {
            let stop_reason_str = match stop_reason {
                ClarificationStopReason::ConfidenceSufficient => "confidence_sufficient",
                ClarificationStopReason::RiskGateBlocked => "risk_gate_blocked",
                ClarificationStopReason::MaxClarificationTurns => "max_clarification_turns",
                ClarificationStopReason::MarginalGainTooLow => "marginal_gain_too_low",
                ClarificationStopReason::UserCostTooHigh => "user_cost_too_high",
                ClarificationStopReason::AttentionBudgetExceeded => "attention_budget_exceeded",
            };

            let question_session_id = if let Some(row) = open {
                row.id
            } else {
                match db
                    .create_question_session(vox_db::QuestionSessionCreateParams {
                        session_id: &session_key,
                        repository_id: &repository_id,
                        task_id: None,
                        policy_version: "v1",
                        started_at_ms: now_ms,
                    })
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::warn!(surface, error = %e, "questioning trace: create_session failed");
                        return;
                    }
                }
            };

            let turn_for_stop = db
                .next_question_event_turn_index(question_session_id)
                .await
                .map(i64::from)
                .unwrap_or(0_i64);

            let _ = db
                .insert_question_stop_event(vox_db::QuestionStopEventParams {
                    question_session_id,
                    stop_reason: stop_reason_str,
                    confidence_at_stop: Some(meta.confidence_estimate),
                    marginal_gain_bits: Some(q.expected_information_gain_bits),
                    expected_user_cost: Some(q.expected_user_cost),
                    turn_index: Some(turn_for_stop),
                    created_at_ms: now_ms,
                })
                .await;
            let _ = db
                .record_questioning_metric(
                    &session_key,
                    Some(q.expected_information_gain_bits),
                    &serde_json::json!({
                        "surface": surface,
                        "question_needed": false,
                        "stop_reason": stop_reason_str,
                        "confidence_estimate": meta.confidence_estimate,
                        "expected_information_gain_bits": q.expected_information_gain_bits,
                        "expected_user_cost": q.expected_user_cost,
                    })
                    .to_string(),
                )
                .await;
            let _ = db
                .close_question_session(question_session_id, "question_skipped", now_ms)
                .await;
        }
    });
}

#[must_use]
fn interruption_channel_for_surface(surface: &str) -> vox_orchestrator::InterruptionChannel {
    match surface {
        "vox_plan" | "vox_replan" | "vox_plan_status" => {
            vox_orchestrator::InterruptionChannel::PlanReview
        }
        "vox_inline_edit" | "vox_ghost_text" => vox_orchestrator::InterruptionChannel::InlineAssist,
        _ => vox_orchestrator::InterruptionChannel::ChatClarification,
    }
}

#[must_use]
fn questioning_policy_metric_payload(
    surface: &str,
    channel: vox_orchestrator::InterruptionChannel,
    question_needed: bool,
    policy_outcome: &str,
    reason: &str,
    decision: &vox_orchestrator::InterruptionDecision,
    timestamp_ms: u64,
    withheld: Option<WithheldQuestion>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "surface": surface,
        "channel": channel_label(channel),
        "question_needed": question_needed,
        "policy_outcome": policy_outcome,
        "reason": reason,
        "decision": decision_label(decision),
        "decision_legacy_debug": format!("{decision:?}"),
        "timestamp_ms": timestamp_ms,
    });
    if let Some(w) = withheld {
        if let Ok(w_val) = serde_json::to_value(w) {
            payload["withheld_question"] = w_val;
        }
    }
    payload
}

#[must_use]
pub(crate) fn mcp_questioning_session_key(
    state: &ServerState,
    surface: &'static str,
    explicit_session_id: Option<&str>,
) -> String {
    explicit_session_id
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| format!("mcp:{}:{surface}", state.repository.repository_id))
}

pub(crate) async fn clarification_turn_for_session(state: &ServerState, session_key: &str) -> u32 {
    let Some(db) = state.db.as_ref() else {
        return 0;
    };
    let repo = state.repository.repository_id.as_str();
    let res: Result<u32, anyhow::Error> = db
        .count_assistant_questions_in_open_session(session_key, repo)
        .await
        .map_err(|e| anyhow::anyhow!(e));
    match res {
        Ok(n) => n,
        Err(e) => {
            tracing::debug!(error = %e, "clarification_turn_for_session failed");
            0
        }
    }
}

#[must_use]
pub(crate) fn socrates_tool_meta(
    policy: &ConfidencePolicy,
    grounding_score: f64,
    contradiction_hint: bool,
    clarification_turn_index: u32,
    spent_clarification_attention_ms: u64,
    max_clarification_attention_ms: u64,
    retrieval: Option<&crate::memory::RetrievalEvidenceEnvelope>,
) -> Value {
    let p = policy;
    let retrieval_contradiction = retrieval
        .map(|r| r.contradiction_count > 0)
        .unwrap_or(false);
    let cr = if contradiction_hint || retrieval_contradiction {
        p.abstain_threshold
    } else {
        0.0_f64
    };
    let cov = retrieval.map(|r| r.citation_coverage).unwrap_or(1.0);
    let decision = p.evaluate_risk_decision(grounding_score, cr, cov);
    let questioning_policy = QuestioningPolicy::default();
    let candidates = retrieval_question_candidates(retrieval);
    let selection = p.select_clarification_question(
        grounding_score,
        cr,
        cov,
        clarification_turn_index,
        &candidates,
        questioning_policy,
        spent_clarification_attention_ms,
        max_clarification_attention_ms,
    );
    let refinement = retrieval.map(|r| {
        serde_json::json!({
            "recommended_action": r.recommended_next_action,
            "reason": if r.contradiction_count > 0 {
                Some("contradictions_detected")
            } else if r.source_diversity <= 1 && (r.memory_hit_count + r.knowledge_hit_count + r.chunk_hit_count + r.repo_hit_count) > 0 {
                Some("single_corpus_evidence")
            } else if r.evidence_quality < 0.55 {
                Some("weak_evidence_quality")
            } else if r.used_lexical_fallback {
                Some("lexical_fallback_only")
            } else {
                None::<&str>
            },
            "verification_performed": r.verification_performed,
            "verification_reason": r.verification_reason,
        })
    });
    serde_json::json!({
        "risk_decision": decision,
        "confidence_estimate": grounding_score,
        "contradiction_ratio": cr,
        "citation_coverage": cov,
        "questioning": {
            "question_needed": selection.question_needed,
            "question_kind": selection.question_kind,
            "prompt": selection.prompt,
            "expected_information_gain_bits": selection.expected_information_gain_bits,
            "expected_user_cost": selection.expected_user_cost,
            "utility_bits_per_cost": selection.utility_bits_per_cost,
            "stop_reason": selection.stop_reason,
        },
        "search_refinement": refinement,
    })
}

fn retrieval_question_candidates(
    retrieval: Option<&crate::memory::RetrievalEvidenceEnvelope>,
) -> Vec<QuestionCandidate> {
    let mut candidates = Vec::new();
    if let Some(r) = retrieval {
        if r.contradiction_count > 0 {
            candidates.push(QuestionCandidate {
                prompt:
                    "Which source should be treated as authoritative when the evidence conflicts?"
                        .to_string(),
                question_kind: QuestionKind::MultipleChoice,
                expected_information_gain_bits: 0.28,
                expected_user_cost: 0.24,
            });
        }
        if r.used_lexical_fallback || r.retrieval_tier == "lexical_fallback" {
            candidates.push(QuestionCandidate {
                prompt: "Provide the exact file, symbol, or phrase you want me to ground against."
                    .to_string(),
                question_kind: QuestionKind::Entry,
                expected_information_gain_bits: 0.26,
                expected_user_cost: 0.22,
            });
        }
        if r.source_diversity <= 1 && r.repo_hit_count > 0 {
            candidates.push(QuestionCandidate {
                prompt: "Should I prioritize repository code structure, documentation, or persisted research sources?".to_string(),
                question_kind: QuestionKind::MultipleChoice,
                expected_information_gain_bits: 0.23,
                expected_user_cost: 0.24,
            });
        } else if r.source_diversity <= 1 {
            candidates.push(QuestionCandidate {
                prompt: "What source of truth should I prioritize for this answer?".to_string(),
                question_kind: QuestionKind::OpenEnded,
                expected_information_gain_bits: 0.20,
                expected_user_cost: 0.30,
            });
        }
        if r.chunk_hit_count == 0 && r.knowledge_hit_count == 0 && r.memory_hit_count > 0 {
            candidates.push(QuestionCandidate {
                prompt: "Can you name the contract, document, or external source I should corroborate against?".to_string(),
                question_kind: QuestionKind::Entry,
                expected_information_gain_bits: 0.22,
                expected_user_cost: 0.26,
            });
        }
    }
    if candidates.is_empty() {
        candidates.push(QuestionCandidate {
            prompt: "Which option best matches your intent?".to_string(),
            question_kind: QuestionKind::MultipleChoice,
            expected_information_gain_bits: 0.20,
            expected_user_cost: 0.25,
        });
        candidates.push(QuestionCandidate {
            prompt: "Please share the key constraint in one sentence.".to_string(),
            question_kind: QuestionKind::OpenEnded,
            expected_information_gain_bits: 0.16,
            expected_user_cost: 0.45,
        });
        candidates.push(QuestionCandidate {
            prompt: "Provide the exact target value or path.".to_string(),
            question_kind: QuestionKind::Entry,
            expected_information_gain_bits: 0.14,
            expected_user_cost: 0.30,
        });
    }
    candidates
}

/// A question the policy chose NOT to surface, exposed to the client so the user can opt in.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub(crate) struct WithheldQuestion {
    pub(crate) prompt: String,
    pub(crate) reason: String,
    pub(crate) expected_information_gain_bits: f64,
}

#[must_use]
pub(crate) fn withheld_question(prompt: &str, reason: &str, eig_bits: f64) -> WithheldQuestion {
    WithheldQuestion {
        prompt: prompt.to_string(),
        reason: reason.to_string(),
        expected_information_gain_bits: eig_bits,
    }
}

#[cfg(test)]
mod withheld_tests {
    use super::*;
    #[test]
    fn withheld_serializes_for_client() {
        let w = withheld_question(
            "Which environment — staging or prod?",
            "backlog_and_low_diagnostic_value",
            0.07,
        );
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["prompt"], "Which environment — staging or prod?");
        assert_eq!(v["reason"], "backlog_and_low_diagnostic_value");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn questioning_metric_payload_persists_normalized_decision() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("memory db");
        let decision = vox_orchestrator::InterruptionDecision::ProceedAutonomously {
            reason: "attention_budget_exhausted_marginal_gain_insufficient".to_string(),
        };
        let payload = questioning_policy_metric_payload(
            "vox_chat_message",
            vox_orchestrator::InterruptionChannel::ChatClarification,
            false,
            "proceed_auto",
            "attention_budget_exhausted_marginal_gain_insufficient",
            &decision,
            123_456,
            None,
        );
        let session = "mcp:test:vox_chat_message";
        db.record_questioning_metric(session, Some(0.07), &payload.to_string())
            .await
            .expect("write metric");
        let rows = db
            .list_research_metrics_by_type("questioning_event", "mcp:test:", 10)
            .await
            .expect("list metrics");
        let meta = rows
            .iter()
            .find_map(
                |(sid, _, meta)| {
                    if sid == session { meta.clone() } else { None }
                },
            )
            .expect("stored metadata row");
        let parsed: serde_json::Value = serde_json::from_str(&meta).expect("json metadata");
        assert_eq!(parsed["surface"], "vox_chat_message");
        assert_eq!(parsed["decision"], "ProceedAutonomously");
        assert_eq!(parsed["channel"], "chat_clarification");
        assert_eq!(parsed["policy_outcome"], "proceed_auto");
    }

    #[test]
    fn rider_instructs_solution_space_reasoning_and_spec_model_split() {
        let r = socrates_system_rider(&ConfidencePolicy::default());
        assert!(
            r.contains("candidate"),
            "must mention enumerating candidate solutions"
        );
        assert!(
            r.to_lowercase().contains("information gain") || r.contains("splits"),
            "must mention picking the most-diagnostic question"
        );
        assert!(
            r.to_lowercase().contains("you want") || r.to_lowercase().contains("specification"),
            "must distinguish user-spec uncertainty"
        );
        // existing behaviour preserved:
        assert!(r.contains("calibrated confidence"));
    }

    #[test]
    fn questioning_metric_payload_includes_withheld_question() {
        let decision = vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint {
            reason: "low_utility".to_string(),
        };
        let withheld = Some(withheld_question("Staging or prod?", "low_utility", 0.05));
        let payload = questioning_policy_metric_payload(
            "vox_chat_message",
            vox_orchestrator::InterruptionChannel::ChatClarification,
            true,
            "deferred",
            "low_utility",
            &decision,
            123_456,
            withheld,
        );
        assert_eq!(payload["policy_outcome"], "deferred");
        assert_eq!(payload["withheld_question"]["prompt"], "Staging or prod?");
        assert_eq!(
            payload["withheld_question"]["expected_information_gain_bits"],
            0.05
        );
    }
}
