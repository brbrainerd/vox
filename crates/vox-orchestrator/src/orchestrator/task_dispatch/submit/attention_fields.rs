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
    // T5.5: use the rolling window (last `DEFAULT_TRUST_WINDOW_MS`), not the lifetime
    // `successful_outcomes`/`total_outcomes` accumulators, so an agent's approve-rate and
    // repeated-approve count age out old behavior instead of weighing it equally forever.
    let now_ms = crate::types::now_unix_ms();
    let approve_rate = trust
        .windowed_approve_rate(now_ms, crate::attention::DEFAULT_TRUST_WINDOW_MS)
        .unwrap_or(0.5);
    let repeated = trust
        .windowed_repeated_approve_count(now_ms, crate::attention::DEFAULT_TRUST_WINDOW_MS)
        .min(50);
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
    // Task E/2.4: RiskPosture may *escalate* the trust-classified tier (e.g. Low
    // risk forces Review) but must never demote below what trust warranted. Take
    // the stricter of (classified, risk-derived). The risk here is the FULL
    // precedence chain (explicit hint > category policy > source policy), not
    // just the explicit hint -- otherwise a task-type category/source risk
    // policy would have zero effect unless a caller also set an explicit hint.
    // Truly-nothing-applies (no explicit, no category, no source) leaves the
    // tier unchanged, matching the pre-existing behavior for unconfigured tasks.
    let (_, category_risk) =
        crate::mode::effective_category_policy(&config.task_policy, task.task_category);
    let source = task
        .trigger_source
        .unwrap_or(crate::mode::TriggerSource::Interactive);
    let (_, source_risk) = crate::mode::effective_source_policy(&config.task_policy, source);
    let effective_risk = task.risk_posture.or(category_risk).or(source_risk);
    let effective_tier = match effective_risk {
        Some(risk) => {
            let risk_tier = risk.resolve().approval.to_approval_tier();
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

    // Task 2.4: a task with NO explicit risk_posture, but a configured
    // category-policy risk override, must still get its approval tier
    // escalated -- reproduces the same "policy has zero effect without an
    // explicit hint" bug fixed in runtime.rs for cost/model routing.
    #[test]
    fn category_risk_policy_escalates_tier_without_explicit_risk_posture() {
        use crate::config::{OrchestratorConfig, TaskPolicyEntry, TaskPolicyOverrides};
        use std::collections::HashMap;

        let mut category = HashMap::new();
        category.insert(
            format!("{:?}", crate::types::TaskCategory::CodeGen),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("low".to_string()),
            },
        );
        let cfg = OrchestratorConfig {
            task_policy: TaskPolicyOverrides {
                category,
                source: HashMap::new(),
            },
            ..OrchestratorConfig::for_testing()
        };
        let orch = Orchestrator::new(cfg);

        let mut t = AgentTask::new(TaskId(7), "generate code", TaskPriority::Normal, vec![]);
        t.task_category = crate::types::TaskCategory::CodeGen;
        assert_eq!(t.risk_posture, None, "no explicit risk hint set");

        populate_task_attention_fields(&orch, &mut t, AgentId(0), &[]);

        // Low risk resolves to ApprovalLean::Review. A brand-new agent with no
        // trust history classifies well below Review, so if the category
        // policy had zero effect the tier would stay at the trust-classified
        // default -- this assertion fails on the pre-fix code.
        assert_eq!(
            t.approval_tier,
            Some(ApprovalTier::Review),
            "category risk policy (Low) must escalate the tier even with no explicit risk_posture hint"
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
