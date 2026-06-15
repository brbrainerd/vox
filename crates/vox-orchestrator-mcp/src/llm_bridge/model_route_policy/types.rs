use vox_orchestrator::types::TaskCategory;

/// Heuristics for MCP chat model resolution from the orchestrator registry.
#[derive(Debug, Clone)]
pub struct McpChatModelResolution {
    /// When resolution fails, fall back to the cheapest free / cheapest model.
    pub allow_cheapest_fallback: bool,
    /// Task complexity hint (1–10) for registry routing.
    pub complexity: u8,
    /// Task category hint so MCP and orchestrator selection share the same intent.
    pub task_category: TaskCategory,
    /// Prefer a free model with large context (ghost text / latency-sensitive paths).
    pub free_tier_latency_critical: bool,
    /// Hint that the workload is fill-in-the-middle (affects free-tier preference).
    pub free_tier_fill_in_middle: bool,
    /// When set, never return a paid model (sticky override included); errors if no free model.
    pub enforce_free_tier_only: bool,
    /// `tokens_used / effective_max` for the MCP LLM budget agent when known (raises routing complexity).
    pub context_fill_ratio: Option<f32>,
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
        }
    }
}
