//! Integration-style wiring: MCP dispatch records tools via [`Orchestrator::record_agentos_mcp_tool`];
//! policy evaluation reads the latest `mutation_kind` through [`Orchestrator::evaluate_orchestrator_policy_for_agent`].

use vox_orchestrator::{Orchestrator, OrchestratorConfig};

#[test]
fn agentos_mcp_policy_wiring_risk_shift_for_shell_vs_read() {
    let orch = Orchestrator::new(OrchestratorConfig::default());
    orch.record_agentos_mcp_tool(Some(42), "vox_git_status");
    let r_read = orch
        .evaluate_orchestrator_policy_for_agent(Some(42))
        .risk_score;
    orch.record_agentos_mcp_tool(Some(42), "vox_run_shell");
    let r_ext = orch
        .evaluate_orchestrator_policy_for_agent(Some(42))
        .risk_score;
    assert!(
        r_ext > r_read,
        "external/shell mutation should raise risk vs read-only git status (read={r_read} ext={r_ext})"
    );
}
