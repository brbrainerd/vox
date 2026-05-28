use crate::Orchestrator;
use crate::planning::synthesizer::synthesize_plan_nodes;
use crate::planning::{PlanNode, PlanningTaskMeta};

pub fn trigger_matches(reason: &str, policy_json: Option<&str>) -> bool {
    let Some(policy_json) = policy_json else {
        return false;
    };
    let Ok(policy) = serde_json::from_str::<crate::planning::ExecutionPolicy>(policy_json) else {
        return false;
    };
    let r = reason.to_ascii_lowercase();
    policy.replan_triggers.iter().any(|t| {
        let s = match t {
            crate::planning::ReplanTrigger::CompilerErrorUnresolved => "compile",
            crate::planning::ReplanTrigger::TestFailureNewRegression => "test",
            crate::planning::ReplanTrigger::ScopeDenied => "scope",
            crate::planning::ReplanTrigger::LockConflictPersistent => "lock",
            crate::planning::ReplanTrigger::MissingCapability => "capability",
            crate::planning::ReplanTrigger::ExternalDependencyUnreachable => "dependency",
            crate::planning::ReplanTrigger::Custom(v) => v,
        };
        if s.trim().is_empty() {
            return false;
        }
        r.contains(s)
            || (s == "test" && (r.contains("repro") || r.contains("flaky")))
            || (s == "compile" && r.contains("unresolved"))
            || (s == "scope" && r.contains("context"))
    })
}

pub async fn synthesize_recovery_nodes(reason: &str, failed_desc: &str) -> Vec<PlanNode> {
    let reason_l = reason.to_ascii_lowercase();
    let reproduce_first = reason_l.contains("test")
        || reason_l.contains("repro")
        || reason_l.contains("regression")
        || reason_l.contains("flaky")
        || reason_l.contains("oracle");
    let goal = if reproduce_first {
        format!(
            "create or refresh a minimal failing reproducer for `{}`, then diagnose and fix task `{}`",
            reason, failed_desc
        )
    } else {
        format!(
            "diagnose failure `{}` and fix task `{}`",
            reason, failed_desc
        )
    };
    synthesize_plan_nodes(&goal)
}

pub async fn enqueue_recovery_first_node(
    orch: &Orchestrator,
    meta: &PlanningTaskMeta,
    reason: &str,
    failed_desc: &str,
    session_id: Option<String>,
    tenant_id: Option<String>,
) -> Result<(), crate::orchestrator::OrchestratorError> {
    let mut nodes = synthesize_recovery_nodes(reason, failed_desc).await;
    if let Some(first) = nodes.first_mut() {
        if meta.campaign_id.is_some()
            || meta.benchmark_tier.is_some()
            || meta.execution_role.is_some()
        {
            first.execution_policy.enqueue_hints = Some(crate::types::TaskEnqueueHints {
                task_category: None,
                complexity: None,
                model_preference: None,
                model_override: None,
                campaign_id: meta.campaign_id.clone(),
                benchmark_tier: meta.benchmark_tier,
                execution_role: meta.execution_role,
                thread_id: None,
                harness_spec_json: None,
                tool_hints: vec![],
                research_hints: vec![],
                is_detached: Some(false),
                requires_approval: None,
                required_labels: None,
                socrates_context: None,
                attachment_manifest: None,
                budget: None,
                trace_id: None,
                active_skill: None,
                tenant_id: tenant_id.clone(),
            });
        }
        let next_version = meta.plan_version.saturating_add(1);
        let _ = crate::planning::executor_bridge::enqueue_plan_node(
            orch,
            first,
            &meta.plan_session_id,
            next_version,
            session_id,
            tenant_id,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_matches_ignores_empty_custom_trigger() {
        let policy = crate::planning::ExecutionPolicy {
            replan_triggers: vec![crate::planning::ReplanTrigger::Custom(String::new())],
            ..Default::default()
        };
        let json = serde_json::to_string(&policy).expect("policy json");
        assert!(!trigger_matches("anything", Some(json.as_str())));
    }
}
