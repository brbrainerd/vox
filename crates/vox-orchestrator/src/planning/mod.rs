pub mod content_blocks;
pub mod executor_bridge;
pub mod intake_router;
pub mod orient;
/// Pure Plan/Act/Verify phase machine (Track D).
pub mod phase_loop;
pub mod plan_adequacy;
/// Plan-mode vs. ReAct mode decision trigger (D2).
pub mod plan_mode_trigger;
pub mod policy;
pub mod prompts;
pub mod quality_gate;
pub mod replan;
pub mod review;
pub mod schedule;
pub mod synthesizer;
pub mod test_decision;
pub mod types;

pub use content_blocks::{ContentBlock, markdown_to_content_blocks};
pub use orient::{OrientPhase, SocratesPlanJudge};
pub use plan_adequacy::{
    PlanAdequacySummary, PlanAdequacyTask, PlanRefinementReport, RubricScores, TaskGapFinding,
    analyze_plan_refinement_report, analyze_plan_refinement_report_with_prior,
    effective_goal_complexity, plan_nodes_to_adequacy_tasks,
};
pub use test_decision::{TestDecision, TestDecisionPolicy};
pub use types::{
    ExecutionPolicy, PlanNode, PlanSessionRecord, PlanStatus, PlanVersionRecord, PlanningMode,
    PlanningStrategy, PlanningTaskMeta, ReplanTrigger, RouterEvaluation,
};
