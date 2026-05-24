//! Structured telemetry for model routing decisions (logs + future Arca hooks).
//!
//! **Classification:** **S1** operator diagnostics (model id, provider, latency) — not end-user usage analytics.
//! Taxonomy: `docs/src/architecture/telemetry-taxonomy-contracts-ssot.md`, trust framing:
//! `docs/src/architecture/telemetry-trust-ssot.md`.

/// One model-routing event for observability and tuning.
#[derive(Debug, Clone)]
pub struct ModelRouteEvent {
    /// Logical source of the route (e.g. `mcp_chat`, `a2a_task`, `research`).
    pub route_source: &'static str,
    /// Task category used for scoring / selection (caller supplies `Debug` or display string, e.g. from orchestrator `TaskCategory`).
    pub task_category: String,
    /// Resolved model identifier sent to the provider.
    pub model_id: String,
    /// Provider organization slug (e.g. `openrouter`, `google`).
    ///
    /// For adapter inference, this is resolved from the registry entry for `model_id` when found,
    /// otherwise falls back to the originally selected model's provider (routing decision).
    pub provider: String,
    /// Whether selection was constrained to free-tier models.
    pub free_only: bool,
    /// Retry / fallback depth (0 = first route).
    pub fallback_generation: u32,
    /// End-to-end latency when known (milliseconds).
    pub latency_ms: Option<u64>,
    /// Whether the inference call completed successfully.
    pub success: bool,
    /// Short error classifier when `success` is false.
    pub error_kind: Option<String>,
}

/// Emit a structured tracing event consumable by log aggregators.
///
/// Target `vox_orchestrator::model_route` can be filtered in `RUST_LOG`.
pub fn emit_model_route(ev: &ModelRouteEvent) {
    tracing::info!(
        target: "vox_orchestrator::model_route",
        route_source = ev.route_source,
        task_category = %ev.task_category,
        model_id = %ev.model_id,
        provider = %ev.provider,
        free_only = ev.free_only,
        fallback_generation = ev.fallback_generation,
        latency_ms = ?ev.latency_ms,
        success = ev.success,
        error_kind = ev.error_kind.as_deref(),
        "model_route"
    );
}
