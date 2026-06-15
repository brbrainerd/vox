//! Shared interruption-policy adapters for MCP surfaces.

use vox_orchestrator::{
    A2AMessageType, AttentionBudget, InterruptionChannel, InterruptionDecision,
    InterruptionSignals, TaskPriority, evaluate_interruption,
};

use crate::server_state::ServerState;

#[must_use]
fn apply_calibration(
    mut signals: InterruptionSignals,
    cfg: &vox_orchestrator::OrchestratorConfig,
) -> InterruptionSignals {
    let cal = &cfg.interruption_calibration;
    let gain_offset = match signals.channel {
        InterruptionChannel::PlanReview => cal.plan_review_gain_offset_bits,
        InterruptionChannel::TaskSubmit => cal.task_submit_gain_offset_bits,
        InterruptionChannel::A2AEscalation => cal.a2a_escalation_gain_offset_bits,
        InterruptionChannel::InlineAssist => cal.inline_assist_gain_offset_bits,
        _ => 0.0,
    };
    signals.expected_information_gain_bits =
        (signals.expected_information_gain_bits + gain_offset).clamp(0.0, 1.5);
    let backlog_penalty = 1.0
        + (signals.pending_clarification_backlog as f64 * cal.backlog_cost_penalty_per_item)
            .max(0.0);
    signals.expected_user_cost = (signals.expected_user_cost * backlog_penalty).clamp(0.05, 1.0);
    let adjusted_trust = (signals.trust_score - 0.5) * cal.trust_adjustment_scale + 0.5;
    signals.trust_score = adjusted_trust.clamp(0.0, 1.0);
    signals
}

#[must_use]
pub(crate) fn has_explicit_human_confirmation(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("[approval:confirm]")
        || t.contains("[approval:reviewed]")
        || t.contains("[human-approved]")
        || t.contains("human_review_approved")
}

#[must_use]
pub(crate) fn decision_label(decision: &InterruptionDecision) -> &'static str {
    match decision {
        InterruptionDecision::InterruptNow { .. } => "InterruptNow",
        InterruptionDecision::DeferUntilCheckpoint { .. } => "DeferUntilCheckpoint",
        InterruptionDecision::BatchWithExistingPrompt { .. } => "BatchWithExistingPrompt",
        InterruptionDecision::ProceedAutonomously { .. } => "ProceedAutonomously",
        InterruptionDecision::RequireHumanBeforeContinue { .. } => "RequireHumanBeforeContinue",
    }
}

#[must_use]
pub(crate) fn channel_label(channel: InterruptionChannel) -> &'static str {
    match channel {
        InterruptionChannel::ChatClarification => "chat_clarification",
        InterruptionChannel::PlanReview => "plan_review",
        InterruptionChannel::TaskSubmit => "task_submit",
        InterruptionChannel::A2AEscalation => "a2a_escalation",
        InterruptionChannel::InlineAssist => "inline_assist",
        InterruptionChannel::Other => "other",
    }
}

#[must_use]
pub(crate) fn trust_for_session(state: &ServerState, session_id: Option<&str>) -> f64 {
    let bm = state.orchestrator.budget_manager_handle();
    let trust_snapshot = vox_orchestrator::sync_lock::rw_read(&*bm).trust_snapshot();
    if let Some(sid) = session_id
        && let Some(agent_id) = state.orchestrator.agent_for_session_id(sid)
        && let Some(ts) = trust_snapshot.get(&agent_id)
    {
        return ts.trust_score;
    }
    trust_snapshot
        .values()
        .next()
        .map(|t| t.trust_score)
        .unwrap_or(0.3)
}

#[must_use]
pub(crate) fn pending_backlog_for_session(state: &ServerState, session_id: Option<&str>) -> u32 {
    let Some(db) = state.db.as_ref() else {
        return 0;
    };
    let Some(sid) = session_id else {
        return 0;
    };
    db.block_on(async {
        db.count_pending_clarifications_for_mcp_session(sid, &state.repository.repository_id)
            .await
            .unwrap_or(0)
    })
}

#[must_use]
pub(crate) fn task_submit_signals(
    description: &str,
    write_file_count: usize,
    priority: TaskPriority,
    session_backlog: u32,
    trust_score: f64,
    base_interrupt_cost_ms: u64,
) -> InterruptionSignals {
    let d = description.to_ascii_lowercase();
    let high_risk = d.contains("deploy")
        || d.contains("production")
        || d.contains("secret")
        || d.contains("delete")
        || d.contains("publish");
    let priority_gain = match priority {
        TaskPriority::Urgent => 0.22,
        TaskPriority::Normal => 0.16,
        TaskPriority::Background => 0.10,
        _ => 0.16,
    };
    let write_cost = (write_file_count as f64 / 4.0).clamp(0.0, 1.0);
    InterruptionSignals {
        channel: InterruptionChannel::TaskSubmit,
        expected_information_gain_bits: (priority_gain + (write_file_count as f64 * 0.01))
            .clamp(0.05, 0.35),
        expected_user_cost: (0.2 + write_cost + if high_risk { 0.2 } else { 0.0 }).clamp(0.05, 1.0),
        confidence_estimate: if high_risk { 0.40 } else { 0.68 },
        contradiction_ratio: if high_risk { 0.20 } else { 0.08 },
        pending_clarification_backlog: session_backlog,
        clarification_turn_index: 0,
        max_clarification_turns: 3,
        irreversible_or_high_risk: high_risk,
        base_interrupt_cost_ms,
        trust_score,
        open_question_session: session_backlog > 0,
    }
}

#[must_use]
pub(crate) fn a2a_escalation_signals(
    msg_type: &A2AMessageType,
    payload: &str,
    session_backlog: u32,
    trust_score: f64,
    base_interrupt_cost_ms: u64,
) -> InterruptionSignals {
    let payload_lc = payload.to_ascii_lowercase();
    let severe_gate = payload_lc.contains("security breach")
        || payload_lc.contains("credential leak")
        || payload_lc.contains("production deploy");
    let high_risk = matches!(
        msg_type,
        A2AMessageType::ErrorReport | A2AMessageType::ConflictDetected
    ) || payload_lc.contains("deploy")
        || payload_lc.contains("outage")
        || payload_lc.contains("security");
    let base_gain = match msg_type {
        A2AMessageType::ErrorReport => 0.24,
        A2AMessageType::ConflictDetected => 0.22,
        A2AMessageType::HelpRequest => 0.14,
        _ => 0.10,
    };
    let cost = (0.18 + ((payload.len() as f64 / 800.0).clamp(0.0, 0.6))).clamp(0.05, 1.0);
    InterruptionSignals {
        channel: InterruptionChannel::A2AEscalation,
        expected_information_gain_bits: base_gain,
        expected_user_cost: cost,
        confidence_estimate: if severe_gate {
            0.39
        } else if high_risk {
            0.42
        } else {
            0.62
        },
        contradiction_ratio: if severe_gate {
            0.42
        } else if high_risk {
            0.28
        } else {
            0.10
        },
        pending_clarification_backlog: session_backlog,
        clarification_turn_index: 0,
        max_clarification_turns: 3,
        irreversible_or_high_risk: high_risk,
        base_interrupt_cost_ms,
        trust_score,
        open_question_session: session_backlog > 0,
    }
}

#[must_use]
pub(crate) fn plan_review_signals(
    expected_gain_bits: f64,
    expected_user_cost: f64,
    session_backlog: u32,
    trust_score: f64,
    high_risk: bool,
    base_interrupt_cost_ms: u64,
) -> InterruptionSignals {
    InterruptionSignals {
        channel: InterruptionChannel::PlanReview,
        expected_information_gain_bits: expected_gain_bits.clamp(0.01, 1.0),
        expected_user_cost: expected_user_cost.clamp(0.05, 1.0),
        confidence_estimate: if high_risk { 0.45 } else { 0.70 },
        contradiction_ratio: if high_risk { 0.25 } else { 0.10 },
        pending_clarification_backlog: session_backlog,
        clarification_turn_index: 0,
        max_clarification_turns: 3,
        irreversible_or_high_risk: high_risk,
        base_interrupt_cost_ms,
        trust_score,
        open_question_session: session_backlog > 0,
    }
}

#[must_use]
pub(crate) fn evaluate_with_state(
    state: &ServerState,
    signals: &InterruptionSignals,
    attention_snapshot: &AttentionBudget,
) -> InterruptionDecision {
    let calibrated = apply_calibration(signals.clone(), &state.orchestrator_config);
    evaluate_interruption(
        &calibrated,
        attention_snapshot,
        state.orchestrator_config.attention_enabled,
        state.orchestrator_config.attention_alert_threshold,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_human_confirmation_tokens_are_detected() {
        assert!(has_explicit_human_confirmation(
            "ship it [approval:confirm]"
        ));
        assert!(has_explicit_human_confirmation("done [human-approved]"));
        assert!(!has_explicit_human_confirmation("please deploy"));
    }

    #[test]
    fn task_submit_signals_sets_task_channel() {
        let s = task_submit_signals("deploy", 3, TaskPriority::Urgent, 2, 0.4, 23_250);
        assert!(matches!(s.channel, InterruptionChannel::TaskSubmit));
        assert!(s.irreversible_or_high_risk);
        assert!(s.pending_clarification_backlog >= 2);
    }

    #[test]
    fn decision_label_is_contract_stable() {
        let d = InterruptionDecision::ProceedAutonomously {
            reason: "x".to_string(),
        };
        assert_eq!(decision_label(&d), "ProceedAutonomously");
    }

    #[test]
    fn severe_a2a_payload_forces_human_gate_inputs() {
        let s = a2a_escalation_signals(
            &A2AMessageType::ErrorReport,
            "security breach and credential leak",
            0,
            0.3,
            23_250,
        );
        assert!(s.irreversible_or_high_risk);
        assert!(s.confidence_estimate < 0.42 || s.contradiction_ratio > 0.38);
    }
}
