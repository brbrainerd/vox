//! Task-local trace context. Propagated across async boundaries via
//! `TRACE_CTX::scope(ctx, future)`.
//!
//! Phase C wires this into A2A envelopes and MCP dispatch.

use uuid::Uuid;

/// Distributed trace context propagated through async task boundaries.
///
/// Fields are `Option` so partial contexts are valid during the bootstrap phase.
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Stable identifier for the originating top-level task.
    pub task_id: Option<u64>,
    /// `task_id` of the direct parent agent, if this is a sub-agent call.
    pub parent_task_id: Option<u64>,
    /// Globally unique trace identifier shared across the entire call tree.
    pub trace_id: Uuid,
    /// Number of agent-to-agent hops from the root; root = 0.
    pub span_depth: u16,
    /// String identifier of the calling agent, if known.
    pub caller_agent_id: Option<String>,
}

impl TraceContext {
    /// Create a root context (no parent).
    pub fn root(task_id: u64) -> Self {
        Self {
            task_id: Some(task_id),
            parent_task_id: None,
            trace_id: Uuid::new_v4(),
            span_depth: 0,
            caller_agent_id: None,
        }
    }

    /// Derive a child context for a sub-agent dispatch.
    pub fn child(&self, child_task_id: u64, child_agent_id: impl Into<String>) -> Self {
        Self {
            task_id: Some(child_task_id),
            parent_task_id: self.task_id,
            trace_id: self.trace_id,
            span_depth: self.span_depth.saturating_add(1),
            caller_agent_id: Some(child_agent_id.into()),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self {
            task_id: None,
            parent_task_id: None,
            trace_id: Uuid::new_v4(),
            span_depth: 0,
            caller_agent_id: None,
        }
    }
}

tokio::task_local! {
    /// Task-local trace context. Use `TRACE_CTX.scope(ctx, fut).await` to attach.
    pub static TRACE_CTX: TraceContext;
}

/// Retrieve the current trace context or return a default empty context.
pub fn current_trace_ctx() -> TraceContext {
    TRACE_CTX.try_with(|ctx| ctx.clone()).unwrap_or_default()
}

/// Retrieve the current trace context, or `None` if no context is active.
pub fn current_trace_context() -> Option<TraceContext> {
    TRACE_CTX.try_with(|ctx| ctx.clone()).ok()
}
