//! Non-bypassable preflight gate for MCP tools when enabled via [`OrchestratorConfig`](crate::config::OrchestratorConfig).

use serde_json::Value;

use super::risk_scoring::tool_risk_score;

/// Hard deny threshold for [`preflight_mcp_tool`].
pub const GUARDRAIL_CRITICAL_SCORE: u8 = 95;

/// Structured deny outcome for telemetry and MCP error surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardrailDenyDetail {
    pub tool: String,
    pub reason: String,
    pub risk_score: u8,
}

/// Evaluate preflight; returns structured detail when the tool must not run.
pub fn evaluate_mcp_tool_preflight(tool: &str, args: &Value) -> Result<(), GuardrailDenyDetail> {
    let score = tool_risk_score(tool, args);
    if score >= GUARDRAIL_CRITICAL_SCORE {
        return Err(GuardrailDenyDetail {
            tool: tool.to_string(),
            reason: format!(
                "AGENTOS_GUARDRAIL: blocked high-risk tool call (score={score}): {tool}"
            ),
            risk_score: score,
        });
    }
    Ok(())
}

/// Returns `Err(reason)` when the call must not proceed.
pub fn preflight_mcp_tool(tool: &str, args: &Value) -> Result<(), String> {
    evaluate_mcp_tool_preflight(tool, args).map_err(|d| d.reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn blocks_obvious_destructive_shell() {
        let r = preflight_mcp_tool(
            "vox_run_shell",
            &json!({ "command": "rm -rf /tmp/x", "user_approval": true }),
        );
        assert!(r.is_err());
    }

    #[test]
    fn allows_low_risk_read() {
        let r = preflight_mcp_tool("vox_git_status", &json!({}));
        assert!(r.is_ok());
    }

    #[test]
    fn evaluate_includes_risk_score_on_deny() {
        let err = evaluate_mcp_tool_preflight(
            "vox_run_shell",
            &json!({ "command": "rm -rf /tmp/x", "user_approval": true }),
        )
        .expect_err("expected deny");
        assert!(err.risk_score >= GUARDRAIL_CRITICAL_SCORE);
        assert_eq!(err.tool, "vox_run_shell");
    }
}
