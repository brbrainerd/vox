//! Bridge to orchestrator-native [`PlanRefinementReport`] (gap + adequacy) for MCP `PlanTask` rows.

use super::params::{PlanDepth, PlanTask};
pub use vox_orchestrator::planning::PlanRefinementReport;
use vox_orchestrator::planning::{self, PlanAdequacyTask};

fn tasks_to_adequacy(tasks: &[PlanTask]) -> Vec<PlanAdequacyTask> {
    tasks
        .iter()
        .map(|t| PlanAdequacyTask {
            id: t.id,
            description: t.description.clone(),
            files: t.files.clone(),
            estimated_complexity: t.estimated_complexity,
            depends_on: t.depends_on.clone(),
            test_decision: None,
            preconditions: vec![],
        })
        .collect()
}

/// Map UI depth to a small integer nudge on minimum task targets.
pub fn plan_depth_bonus(depth: Option<PlanDepth>) -> i8 {
    match depth.unwrap_or_default() {
        PlanDepth::Minimal => -1,
        PlanDepth::Standard => 0,
        PlanDepth::Deep => 2,
    }
}

/// Tier-1 gap + plan adequacy (shared with orchestrator planning).
///
/// `prior_tasks`: plan snapshot **before** a refinement step; when set, compares against `tasks`
/// (after) for rewrite-compression and linkage-loss signals.
pub fn analyze_plan_gaps(
    goal: &str,
    scope_file_count: usize,
    router_complexity_hint: Option<u8>,
    plan_depth: Option<PlanDepth>,
    tasks: &[PlanTask],
    prior_tasks: Option<&[PlanTask]>,
) -> PlanRefinementReport {
    let at = tasks_to_adequacy(tasks);
    let prior_adeq = prior_tasks.map(tasks_to_adequacy);
    let prior_slice = prior_adeq.as_deref();
    planning::analyze_plan_refinement_report_with_prior(
        goal,
        scope_file_count,
        router_complexity_hint,
        plan_depth_bonus(plan_depth),
        &at,
        prior_slice,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::planning::PlanAdequacyTask;

    fn sample_task(id: usize, desc: &str) -> PlanTask {
        PlanTask {
            id,
            description: desc.to_string(),
            category: None,
            files: vec![],
            estimated_complexity: 3,
            depends_on: vec![],
        }
    }

    #[test]
    fn mcp_bridge_destructive_task() {
        let tasks = vec![sample_task(1, "rm -rf /unused directory")];
        let r = analyze_plan_gaps("cleanup", 0, None, None, &tasks, None);
        assert!(
            r.critical_count >= 1,
            "destructive operations must be flagged"
        );
    }

    #[test]
    fn bridge_matches_adequacy_task_shape() {
        let t = PlanAdequacyTask {
            id: 1,
            description: "x".into(),
            files: vec![],
            estimated_complexity: 1,
            depends_on: vec![],
            test_decision: None,
            preconditions: vec![],
        };
        let r = planning::analyze_plan_refinement_report("goal", 0, None, 0, &[t], false);
        assert!(r.aggregate_unresolved_risk >= 0.0);
    }
}
