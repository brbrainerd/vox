//! Execution time telemetry types for `agent_exec_history`.
//!
//! Moved from `vox_db::exec_time_telemetry` so consumers can construct records
//! without pulling in the heavy `vox-db` crate. The `TimedExecution` driver
//! (which embeds an `Option<vox_db::VoxDb>` and an `Instant`) stays in `vox-db`.

/// Write-path record for one execution observation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecTimeRecord<'a> {
    /// Stable tool fingerprint, e.g. `"mcp:vox_build_crate"`, `"process:unity_editor"`.
    /// Convention: prefix with `"mcp:"` for MCP tools, `"process:"` for OS-launched processes.
    pub tool_key: &'a str,
    pub repository_id: &'a str,
    pub duration_ms: u64,
    /// Budget the agent planned for (None if first-ever run or no forecast was available).
    pub timeout_budget_ms: Option<u64>,
    pub compute_tokens_used: Option<u64>,
    pub vendor_cost_usd_micros: Option<i64>,
    pub attention_cost_ms: Option<u64>,
    pub outcome: ExecOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecOutcome {
    Success,
    Timeout,
    Error,
}

impl ExecOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            ExecOutcome::Success => "success",
            ExecOutcome::Timeout => "timeout",
            ExecOutcome::Error => "error",
        }
    }
}

/// Historical latency profile returned by `vox_db::VoxDb::query_tool_latency`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolLatencyProfile {
    pub tool_key: String,
    /// Total successful observations in the query window.
    pub sample_count: i64,
    pub avg_ms: f64,
    /// Approximate 90th-percentile (via ORDER BY + OFFSET trick; no extension required).
    pub p90_ms: f64,
    pub max_ms: i64,
    /// Fraction of all observations (including non-success) that ended in timeout.
    pub timeout_rate: f64,
    /// Suggested agent wait: `(p90_ms * safety_multiplier).ceil() as u64`.
    pub recommended_budget_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_outcome_serialization() {
        assert_eq!(ExecOutcome::Success.as_str(), "success");
        assert_eq!(ExecOutcome::Timeout.as_str(), "timeout");
        assert_eq!(ExecOutcome::Error.as_str(), "error");
    }
}
