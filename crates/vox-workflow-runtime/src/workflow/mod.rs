//! Interpreted workflow planning and execution (internal).

mod hir_context;
pub mod plan;
pub mod populi;
mod return_extract;
pub mod run;
pub mod tracker;
pub mod types;

pub use hir_context::{current_hir_module, set_current_hir_module};
pub use plan::{plan_workflow_activities, plan_workflow_replay_ir};
#[cfg(feature = "mens")]
pub use populi::execute_populi_step;
pub use return_extract::{ExtractError, extract_terminal_return};
pub use run::{WORKFLOW_JOURNAL_VERSION, interpret_workflow, interpret_workflow_durable};
pub use tracker::{DefaultTracker, InMemoryTracker, WorkflowTracker};
pub use types::{PlannedActivity, PopuliActivity, PopuliHttpOp, ReplayNode, WorkflowReplayIr};

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::HirModule;

    #[test]
    fn plan_workflow_activities_returns_error_when_hir_is_stubbed() {
        let hir = HirModule::default();
        let result = plan_workflow_activities(&hir, "any_workflow");
        assert!(
            result.is_err(),
            "planner should error when workflow HIR is unavailable"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found"),
            "error should mention workflow not found: {msg}"
        );
    }
}
