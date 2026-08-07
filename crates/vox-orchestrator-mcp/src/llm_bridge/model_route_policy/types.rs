use vox_orchestrator::types::TaskCategory;

/// Heuristics for MCP chat model resolution from the orchestrator registry.
#[derive(Debug, Clone)]
pub struct McpChatModelResolution {
    /// When resolution fails, fall back to the cheapest free / cheapest model.
    pub allow_cheapest_fallback: bool,
    /// Task complexity hint (1–10) for registry routing.
    pub complexity: u8,
    /// Task category hint so MCP and orchestrator selection share the same
    /// intent (feeds `SelectionIntent::for_task` and the capability-pin
    /// heuristic). Defaults to `CodeGen` for that legacy purpose and most call
    /// sites never override it — NOT a reliable signal of the caller's actual
    /// task type, so the per-category cost/model policy feature deliberately
    /// does not key off this field here (see `resolve_mcp_chat_model_sync_inner`).
    pub task_category: TaskCategory,
    /// Prefer a free model with large context (ghost text / latency-sensitive paths).
    pub free_tier_latency_critical: bool,
    /// Hint that the workload is fill-in-the-middle (affects free-tier preference).
    pub free_tier_fill_in_middle: bool,
    /// When set, never return a paid model (sticky override included); errors if no free model.
    pub enforce_free_tier_only: bool,
    /// `tokens_used / effective_max` for the MCP LLM budget agent when known (raises routing complexity).
    pub context_fill_ratio: Option<f32>,
    /// Task clutch profile ("how much gas"). When `Some`, drives `SelectionAxes`
    /// from `effective_axes(clutch, risk)` and (for `Free`) constrains to the free
    /// pool. `None` keeps the legacy binary Economy/Performance fallback.
    pub clutch: Option<vox_orchestrator::mode::ClutchProfile>,
    /// Task risk posture. When `Some` alongside `clutch`, a `Low` posture's
    /// `ModelLean::Intelligence` overrides a cheap clutch toward intelligence-weighted axes.
    pub risk: Option<vox_orchestrator::mode::RiskPosture>,
    /// Who/what triggered this resolution. Every MCP tool call site constructs
    /// this struct directly from a live chat/editor feature, so `Interactive`
    /// is correct by construction here — no inference needed (contrast with
    /// `AgentTask.trigger_source`, which is genuinely optional/hinted).
    pub trigger_source: vox_orchestrator::mode::TriggerSource,
}

impl Default for McpChatModelResolution {
    fn default() -> Self {
        Self {
            allow_cheapest_fallback: false,
            complexity: 5,
            task_category: TaskCategory::CodeGen,
            free_tier_latency_critical: false,
            free_tier_fill_in_middle: false,
            enforce_free_tier_only: false,
            context_fill_ratio: None,
            clutch: None,
            risk: None,
            trigger_source: vox_orchestrator::mode::TriggerSource::Interactive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_trigger_source_is_interactive() {
        let res = McpChatModelResolution::default();
        assert_eq!(
            res.trigger_source,
            vox_orchestrator::mode::TriggerSource::Interactive
        );
    }
}
