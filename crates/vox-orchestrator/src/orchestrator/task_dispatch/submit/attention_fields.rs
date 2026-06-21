//! Populate [`AgentTask::approval_tier`] and [`AgentTask::attention_weight`] at enqueue time.

use crate::attention::{
    ActionDescriptor, AgentTrustScore, ApprovalTier, classify_tier, compute_attention_cost_ms,
    decision_entropy_bits,
};
use crate::orchestrator::Orchestrator;
use crate::types::{AccessKind, AgentId, AgentTask, FileAffinity};

#[must_use]
pub(super) fn task_description_suggests_external(description: &str) -> bool {
    let d = description.to_ascii_lowercase();
    d.contains("deploy")
        || d.contains("production")
        || d.contains("publish ")
        || d.contains("http://")
        || d.contains("https://")
        || d.contains("curl ")
        || d.contains("terraform")
        || d.contains("kubectl")
        || d.contains("api key")
        || d.contains("secret ")
}

/// Set Phase-15 attention metadata from trust, manifest, and orchestrator attention config.
pub(super) fn populate_task_attention_fields(
    orch: &Orchestrator,
    task: &mut AgentTask,
    agent_id: AgentId,
    file_manifest: &[FileAffinity],
) {
    let config = crate::sync_lock::rw_read(&*orch.config);
    let bm = crate::sync_lock::rw_read(&*orch.budget_manager);
    let trust = bm
        .trust_snapshot()
        .get(&agent_id)
        .cloned()
        .unwrap_or_else(|| AgentTrustScore::new(agent_id));

    let write_count = file_manifest
        .iter()
        .filter(|f| f.access == AccessKind::Write)
        .count();
    let external = task_description_suggests_external(&task.description)
        || file_manifest.iter().any(|f| {
            f.path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("deploy")
        });
    let concurrent_tasks = {
        let agents = crate::sync_lock::rw_read(&*orch.agents);
        agents
            .values()
            .map(|q| {
                let qq = crate::sync_lock::rw_read(q);
                qq.len() + qq.in_progress_count()
            })
            .sum::<usize>()
    };
    let approve_rate = if trust.total_outcomes > 0 {
        trust.successful_outcomes as f64 / trust.total_outcomes as f64
    } else {
        0.5
    };
    let repeated = trust.successful_outcomes.min(50);
    let action = ActionDescriptor {
        estimated_complexity: task.estimated_complexity,
        tokens_output: 0,
        priority: task.priority,
        write_file_count: write_count,
        external,
        repeated_approve_count: repeated,
        concurrent_tasks: concurrent_tasks.max(1),
    };
    let entropy = decision_entropy_bits(approve_rate);
    let tier = classify_tier(&trust, &action, entropy, &config.tier_gate);
    // Task E: RiskPosture may *escalate* the trust-classified tier (e.g. Low risk
    // forces Review) but must never demote below what trust warranted. Take the
    // stricter of (classified, risk-derived). None == Moderate-equivalent, which
    // resolves to Confirm and therefore cannot demote a higher classified tier.
    let effective_tier = match task.risk_posture {
        Some(_) => {
            let risk_tier = task.resolved_risk().approval.to_approval_tier();
            tier.max_strictness(risk_tier)
        }
        None => tier,
    };
    task.approval_tier = Some(effective_tier);
    let base = config.attention_interrupt_cost_ms.max(1);
    let cost = compute_attention_cost_ms(
        &action,
        trust.trust_score,
        base,
        &config.attention_tlx_weights,
    );
    task.attention_weight = cost as f64 / base as f64;
}

#[must_use]
pub(super) fn submission_approval_block_reason(task: &AgentTask) -> Option<String> {
    if matches!(task.status, crate::types::TaskStatus::BlockedOnApproval) {
        return Some(format!(
            "task {} requires explicit approval before execution (plan.execution_mode = RequiresApproval)",
            task.id
        ));
    }
    match task.approval_tier {
        Some(ApprovalTier::Blocked) => Some(format!(
            "task {} was classified as Blocked by approval policy (attention_weight={:.2})",
            task.id, task.attention_weight
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::ApprovalTier;
    use crate::types::{AgentTask, TaskId, TaskPriority};

    #[test]
    fn blocked_tier_returns_reason() {
        let mut t = AgentTask::new(TaskId(42), "deploy", TaskPriority::Urgent, vec![]);
        t.approval_tier = Some(ApprovalTier::Blocked);
        t.attention_weight = 2.4;
        let reason = submission_approval_block_reason(&t);
        assert!(reason.is_some());
        assert!(reason.unwrap_or_default().contains("Blocked"));
    }

    #[test]
    fn blocked_on_approval_returns_reason() {
        let mut t = AgentTask::new(TaskId(43), "deploy", TaskPriority::Urgent, vec![]);
        t.status = crate::types::TaskStatus::BlockedOnApproval;
        let reason = submission_approval_block_reason(&t);
        assert!(reason.is_some());
        assert!(
            reason
                .unwrap_or_default()
                .contains("requires explicit approval")
        );
    }

    #[test]
    fn non_blocked_tier_is_allowed() {
        let mut t = AgentTask::new(TaskId(1), "normal", TaskPriority::Normal, vec![]);
        t.approval_tier = Some(ApprovalTier::Confirm);
        assert!(submission_approval_block_reason(&t).is_none());
    }

    // Task E: replicate the escalation decision in attention_fields without
    // constructing a whole Orchestrator. `effective = classified.max_strictness(risk)`.
    fn effective(
        classified: ApprovalTier,
        posture: Option<crate::mode::RiskPosture>,
    ) -> ApprovalTier {
        match posture {
            Some(p) => classified.max_strictness(p.resolve().approval.to_approval_tier()),
            None => classified,
        }
    }

    #[test]
    fn low_risk_escalates_confirm_to_review() {
        // Low risk resolves to ApprovalLean::Review.
        assert_eq!(
            effective(ApprovalTier::Confirm, Some(crate::mode::RiskPosture::Low)),
            ApprovalTier::Review
        );
    }

    #[test]
    fn high_risk_does_not_demote_trust_required_review() {
        // High risk resolves to AutoApprove, but must NOT pull a classified Review down.
        assert_eq!(
            effective(ApprovalTier::Review, Some(crate::mode::RiskPosture::High)),
            ApprovalTier::Review
        );
    }

    #[test]
    fn high_risk_does_not_demote_blocked() {
        assert_eq!(
            effective(ApprovalTier::Blocked, Some(crate::mode::RiskPosture::High)),
            ApprovalTier::Blocked
        );
    }

    #[test]
    fn none_posture_leaves_classified_tier_unchanged() {
        assert_eq!(
            effective(ApprovalTier::Confirm, None),
            ApprovalTier::Confirm
        );
        assert_eq!(
            effective(ApprovalTier::AutoApprove, None),
            ApprovalTier::AutoApprove
        );
    }
}
