# Telemetry Phase A — `vox-telemetry` Facade Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a new `vox-telemetry` L1 facade crate that hosts all canonical metric-type constants and the `TelemetryRecorder` trait, move them out of `vox-db::research_metrics_contract`, and wire two dormant sinks (ResearchMetricsSink, SpoolSink) at CLI startup — with zero semantic change to existing telemetry emission.

**Architecture:** A pure-types + trait crate at L1 defines `TelemetryEvent`, all `METRIC_TYPE_*` constants, `TelemetryWriteOptions`, `validate_research_metric_row`, the `TelemetryRecorder` trait, a `OnceLock`-backed global handle, and a `record_event!` no-op macro. `vox-db::research_metrics_contract` becomes a re-export shim. `vox-db` and `vox-cli` each grow one sink implementation file. The CLI registers a `CompositeRecorder([ResearchMetricsSink, SpoolSink])` at startup; since no call sites call `record_event!` yet, both sinks sit inert.

**Tech Stack:** Rust 2024 edition, `serde`/`serde_json`, `thiserror`, `tokio` (task_local + spawn), `tracing`, `uuid`, `std::sync::OnceLock`

**Spec:** `docs/src/architecture/telemetry-unification-design-2026.md` (Phase A)

**Phases B–D** (model_call_event, span propagation, default-on config) are separate plans.

---

## File map

| Action | Path | Responsibility |
|---|---|---|
| **Create** | `crates/vox-telemetry/Cargo.toml` | L1 crate manifest |
| **Create** | `crates/vox-telemetry/src/lib.rs` | Public API, `record_event!` macro |
| **Create** | `crates/vox-telemetry/src/types.rs` | All constants, `TelemetryWriteOptions`, `validate_research_metric_row`, `TelemetryEvent`, `TelemetryError` |
| **Create** | `crates/vox-telemetry/src/recorder.rs` | `TelemetryRecorder` trait, `OnceLock` global, `CompositeRecorder` |
| **Create** | `crates/vox-telemetry/src/no_op.rs` | `NoOpRecorder` |
| **Create** | `crates/vox-telemetry/src/span.rs` | `TraceContext`, `task_local!` |
| **Create** | `crates/vox-telemetry/src/config.rs` | `TelemetryConfig`, `from_env()` |
| **Modify** | `Cargo.toml` (workspace root) | Add `vox-telemetry` to `workspace.dependencies` |
| **Modify** | `docs/src/architecture/layers.toml` | Add `vox-telemetry = { layer = 1 }` |
| **Modify** | `docs/src/architecture/where-things-live.md` | Add L1 row for `vox-telemetry` |
| **Modify** | `crates/vox-db/Cargo.toml` | Add `vox-telemetry = { workspace = true }` |
| **Modify** | `crates/vox-db/src/research_metrics_contract.rs` | Replace body with `pub use vox_telemetry::types::*;` |
| **Modify** | `crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs` | Update import + `.map_err()` on validate call |
| **Create** | `crates/vox-db/src/telemetry_sink.rs` | `ResearchMetricsSink` implementing `TelemetryRecorder` |
| **Modify** | `crates/vox-db/src/lib.rs` | Add `pub mod telemetry_sink;` |
| **Modify** | `crates/vox-cli/Cargo.toml` | Add `vox-telemetry = { workspace = true }` |
| **Create** | `crates/vox-cli/src/telemetry_sink.rs` | `SpoolSink` implementing `TelemetryRecorder` |
| **Modify** | `crates/vox-cli/src/lib.rs` | Add `pub mod telemetry_sink;`, `init_telemetry_sinks()`, call it from `run_vox_cli_from_parsed` |
| **Modify** | `crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs` | Update path from `vox-db/src/research_metrics_contract.rs` → `vox-telemetry/src/types.rs` |

---

## Task 1 — Scaffold `vox-telemetry` crate manifest and register in workspace

**Files:**
- Create: `crates/vox-telemetry/Cargo.toml`
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]` section)
- Modify: `docs/src/architecture/layers.toml`

- [ ] **Step 1: Create the crate directory and Cargo.toml**

```toml
# crates/vox-telemetry/Cargo.toml
[package]
name = "vox-telemetry"
description = "L1 telemetry facade: canonical metric-type constants, TelemetryRecorder trait, and record_event! macro. Zero domain dependencies."
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
serde        = { workspace = true, features = ["derive"] }
serde_json   = { workspace = true }
thiserror    = { workspace = true }
tokio        = { workspace = true, features = ["rt", "macros"] }
tracing      = { workspace = true }
uuid         = { version = "1.0", features = ["v4", "serde"] }
workspace-hack = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create `crates/vox-telemetry/src/` directory with empty lib.rs placeholder**

```rust
// crates/vox-telemetry/src/lib.rs  (placeholder — filled in Task 5)
```

- [ ] **Step 3: Add `vox-telemetry` to `[workspace.dependencies]` in root `Cargo.toml`**

Find the `[workspace.dependencies]` block. Add this line immediately after `vox-build-meta`:

```toml
vox-telemetry             = { path = "crates/vox-telemetry" }
```

- [ ] **Step 4: Add `vox-telemetry` to `layers.toml` under L1**

In `docs/src/architecture/layers.toml`, find the `# ── L1 — primitives & utilities` block. Add after `vox-plugin-types`:

```toml
vox-telemetry = { layer = 1 }                            # L1 telemetry facade; zero domain deps
```

- [ ] **Step 5: Verify crate is discoverable**

```bash
cargo metadata --no-deps --format-version 1 | python -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; print([p['name'] for p in pkgs if p['name']=='vox-telemetry'])"
```

Expected: `['vox-telemetry']`

- [ ] **Step 6: Commit scaffold**

```bash
git add crates/vox-telemetry/ Cargo.toml docs/src/architecture/layers.toml
git commit -m "feat(vox-telemetry): scaffold L1 facade crate + register in workspace/layers"
```

---

## Task 2 — Create `types.rs`: move constants and validation from `vox-db`

**Files:**
- Create: `crates/vox-telemetry/src/types.rs`

- [ ] **Step 1: Write `types.rs` with all constants, types, and validation moved from `vox-db::research_metrics_contract`**

```rust
// crates/vox-telemetry/src/types.rs
//! Canonical telemetry constants, event types, and validation helpers.
//!
//! All `METRIC_TYPE_*` constants and [`validate_research_metric_row`] previously
//! lived in `vox-db::research_metrics_contract`. That module now re-exports
//! everything from here so downstream code continues to compile unchanged.
//!
//! - Human contract: `docs/src/reference/telemetry-metric-contract.md`
//! - Sensitivity classes (S0–S3): `docs/src/architecture/telemetry-retention-sensitivity-ssot.md`
//! - Taxonomy / owners: `docs/src/architecture/telemetry-taxonomy-contracts-ssot.md`
//! - Design: `docs/src/architecture/telemetry-unification-design-2026.md`

// ── TelemetryError ────────────────────────────────────────────────────────

/// Error returned by [`validate_research_metric_row`] and sink operations.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("telemetry validation: {0}")]
    Validation(String),
}

// ── Payload size and field length limits ────────────────────────────────────

/// Upper bound on `metadata_json` payload size.
pub const RESEARCH_METRICS_METADATA_JSON_MAX_BYTES: usize = 256 * 1024;
/// `session_id` length cap.
pub const RESEARCH_METRICS_SESSION_ID_MAX_CHARS: usize = 512;
/// `metric_type` length cap.
pub const RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS: usize = 128;

// ── Canonical `metric_type` values — existing (S0–S2) ────────────────────────

/// S0 — Coarse benchmark names, timings, ratios.
pub const METRIC_TYPE_BENCHMARK_EVENT: &str = "benchmark_event";
/// S0–S1 — Compiler / fixture ids.
pub const METRIC_TYPE_SYNTAX_K_EVENT: &str = "syntax_k_event";
/// S1 (payload may reach S2) — MCP surface calibration.
pub const METRIC_TYPE_SOCRATES_SURFACE: &str = "socrates_surface";
/// S2 — BM25/vector fusion stats.
pub const METRIC_TYPE_MEMORY_HYBRID_FUSION: &str = "memory_hybrid_fusion";
/// S1–S2 — Durable workflow journal JSON.
pub const METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY: &str = "workflow_journal_entry";
/// S1 — Mesh/registry audit; must not store bearer tokens.
pub const METRIC_TYPE_POPULI_CONTROL_EVENT: &str = "populi_control_event";
/// S1–S2 — Questioning KPIs.
pub const METRIC_TYPE_QUESTIONING_EVENT: &str = "questioning_event";
/// S1 — Tool execution tracking (duration, timeouts, compute/cost).
pub const METRIC_TYPE_AGENT_EXEC_TIME: &str = "agent_exec_time";
/// S1 — Routing policy / capability-gate events from runtime/orchestrator.
pub const METRIC_TYPE_MODEL_ROUTE_EVENT: &str = "model_route_event";

// ── Orchestrator policy decision metrics (D1–D10) — S1 ────────────────────

/// D6: Circuit breaker trip — doom-loop detected.
pub const METRIC_TYPE_CIRCUIT_BREAKER_TRIP: &str = "orch.circuit_breaker.trip";
/// D3: Socrates confidence fusion decision.
pub const METRIC_TYPE_SOCRATES_FUSION: &str = "orch.socrates.fusion";
/// D1: Model tier routing decision (Economy / Standard / Strong).
pub const METRIC_TYPE_MODEL_TIER_ROUTE: &str = "orch.routing.tier";
/// D2: Plan-mode vs. ReAct mode decision.
pub const METRIC_TYPE_PLAN_MODE_DECISION: &str = "orch.plan.mode_decision";
/// D9: HITL interrupt — human-in-the-loop escalation triggered.
pub const METRIC_TYPE_HITL_INTERRUPT: &str = "orch.hitl.interrupt";
/// D5: Risk matrix score event.
pub const METRIC_TYPE_RISK_SCORE: &str = "orch.risk.score";
/// D8: Privacy routing decision.
pub const METRIC_TYPE_PRIVACY_ROUTE_DECISION: &str = "orch.privacy.route_decision";
/// D7: Cache hit prediction for provider routing.
pub const METRIC_TYPE_CACHE_HIT_PREDICTION: &str = "orch.cache.hit_prediction";
/// D7: Budget gate decision (Proceed / Downgrade / Halt).
pub const METRIC_TYPE_BUDGET_DECISION: &str = "orch.budget.decision";
/// D10: Calibration loop run observation.
pub const METRIC_TYPE_CALIBRATION_RUN: &str = "orch.calibration.run";
/// D10: Calibration drift alert — predicted vs. observed confidence diverging.
pub const METRIC_TYPE_DRIFT_ALERT: &str = "orch.calibration.drift_alert";
/// D10: Contextual bandit arm update.
pub const METRIC_TYPE_BANDIT_UPDATE: &str = "orch.calibration.bandit_update";
/// D4: Sub-agent dispatched.
pub const METRIC_TYPE_SUBAGENT_DISPATCH: &str = "orch.subagent.dispatch";
/// D4: Sub-agent chain depth exceeded configured cap.
pub const METRIC_TYPE_CHAIN_DEPTH_ALERT: &str = "orch.subagent.chain_depth_alert";

// ── New metric types (Phase B–D) — defined here, used when emit sites are added ─

/// S1 — Per-LLM-call record: tokens, cache, latency, cost, error class. (Phase B)
pub const METRIC_TYPE_MODEL_CALL_EVENT: &str = "model_call_event";
/// S1 — Top-level task completion rollup: total tokens, cost, wall time, depth. (Phase C)
pub const METRIC_TYPE_TASK_ROOT_SUMMARY: &str = "task.root_summary";
/// S0 — Build summary mirrored from build_run after `vox ci build-timings`. (Phase D)
pub const METRIC_TYPE_BUILD_SUMMARY_EVENT: &str = "build.summary";
/// S1 — Generic subsystem error / retry event. (Phase D)
pub const METRIC_TYPE_ERROR_EVENT: &str = "telemetry.error";

// ── Session id prefixes ──────────────────────────────────────────────────────

pub const SESSION_PREFIX_BENCH: &str = "bench:";
pub const SESSION_PREFIX_SYNTAXK: &str = "syntaxk:";
pub const SESSION_PREFIX_MCP: &str = "mcp:";
pub const SESSION_PREFIX_WORKFLOW: &str = "workflow:";
pub const SESSION_PREFIX_MENS: &str = "mens:";
pub const SESSION_PREFIX_ROUTE: &str = "route:";

/// Hybrid retrieval telemetry uses a fixed session id (not `mcp:<repository_id>`).
pub const SESSION_ID_MEMORY_HYBRID_FUSION: &str = "socrates:retrieval";

// ── TelemetryWriteOptions ────────────────────────────────────────────────────

/// Repository-scoped session_id builder. Use with `METRIC_TYPE_*` constants.
#[derive(Debug, Clone, Copy)]
pub struct TelemetryWriteOptions<'a> {
    pub repository_id: &'a str,
}

impl<'a> TelemetryWriteOptions<'a> {
    #[inline]
    pub fn new(repository_id: &'a str) -> Self {
        Self { repository_id }
    }

    #[inline]
    pub fn session_bench(&self) -> String {
        format!("{SESSION_PREFIX_BENCH}{}", self.repository_id)
    }

    #[inline]
    pub fn session_syntaxk(&self) -> String {
        format!("{SESSION_PREFIX_SYNTAXK}{}", self.repository_id)
    }

    #[inline]
    pub fn session_mcp(&self) -> String {
        format!("{SESSION_PREFIX_MCP}{}", self.repository_id)
    }

    #[inline]
    pub fn session_workflow(&self) -> String {
        format!("{SESSION_PREFIX_WORKFLOW}{}", self.repository_id)
    }

    #[inline]
    pub fn session_mens(&self) -> String {
        format!("{SESSION_PREFIX_MENS}{}", self.repository_id)
    }

    #[inline]
    pub fn session_route(&self) -> String {
        format!("{SESSION_PREFIX_ROUTE}{}", self.repository_id)
    }
}

// ── Validation ───────────────────────────────────────────────────────────────

fn valid_metric_type_chars(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
}

/// Validate inputs before `INSERT INTO research_metrics`.
///
/// Called by [`vox_db::VoxDb::append_research_metric`] on every write.
pub fn validate_research_metric_row(
    session_id: &str,
    metric_type: &str,
    metadata_json: Option<&str>,
) -> Result<(), TelemetryError> {
    if session_id.is_empty() {
        return Err(TelemetryError::Validation(
            "research_metrics: session_id must be non-empty".into(),
        ));
    }
    if session_id.len() > RESEARCH_METRICS_SESSION_ID_MAX_CHARS {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: session_id exceeds {} characters",
            RESEARCH_METRICS_SESSION_ID_MAX_CHARS
        )));
    }
    if metric_type.is_empty() {
        return Err(TelemetryError::Validation(
            "research_metrics: metric_type must be non-empty".into(),
        ));
    }
    if metric_type.len() > RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: metric_type exceeds {} characters",
            RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS
        )));
    }
    if !valid_metric_type_chars(metric_type) {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: metric_type {metric_type:?} contains disallowed characters"
        )));
    }
    if let Some(m) = metadata_json {
        if m.len() > RESEARCH_METRICS_METADATA_JSON_MAX_BYTES {
            return Err(TelemetryError::Validation(format!(
                "research_metrics: metadata_json exceeds {} bytes",
                RESEARCH_METRICS_METADATA_JSON_MAX_BYTES
            )));
        }
        if metric_type == METRIC_TYPE_MODEL_ROUTE_EVENT
            && (!m.contains("\"trace_id\"") || !m.contains("\"route_policy_profile\""))
        {
            return Err(TelemetryError::Validation(
                "research_metrics: model_route_event metadata_json must include trace_id and route_policy_profile".into(),
            ));
        }
    }
    Ok(())
}

// ── TelemetryEvent ─────────────────────────────────────────────────────────

/// A single telemetry observation that may be routed to one or more sinks.
///
/// New variants are added in later phases; each variant maps to one or more
/// `METRIC_TYPE_*` constants. All variants are `non_exhaustive` so older sinks
/// can ignore unknown events safely via `_ => {}` arms.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TelemetryEvent {
    /// Wraps the existing research_metrics row. Passed through ResearchMetricsSink unchanged.
    ResearchMetric(ResearchMetricEvent),
    // Phase B adds: ModelCall(ModelCallEvent)
    // Phase C adds: TaskRootSummary(TaskRootSummaryEvent)
    // Phase D adds: BuildSummary(BuildSummaryEvent), Error(ErrorEvent)
}

/// Payload for a `TelemetryEvent::ResearchMetric`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResearchMetricEvent {
    pub session_id: String,
    pub metric_type: String,
    pub metric_value: Option<f64>,
    pub metadata_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_session_and_metric_type() {
        assert!(validate_research_metric_row("", "t", None).is_err());
        assert!(validate_research_metric_row("s", "", None).is_err());
    }

    #[test]
    fn accepts_colon_in_metric_type() {
        assert!(validate_research_metric_row("sess", "mcp:foo_bar", None).is_ok());
    }

    #[test]
    fn rejects_disallowed_metric_type_chars() {
        assert!(validate_research_metric_row("s", "bad type", None).is_err());
        assert!(validate_research_metric_row("s", "bad/type", None).is_err());
    }

    #[test]
    fn enforces_metadata_size_cap() {
        let big = "x".repeat(RESEARCH_METRICS_METADATA_JSON_MAX_BYTES + 1);
        assert!(validate_research_metric_row("s", "t", Some(&big)).is_err());
        let ok = "x".repeat(RESEARCH_METRICS_METADATA_JSON_MAX_BYTES);
        assert!(validate_research_metric_row("s", "t", Some(&ok)).is_ok());
    }

    #[test]
    fn telemetry_write_options_builds_expected_sessions() {
        let o = TelemetryWriteOptions::new("rid42");
        assert_eq!(o.session_bench(), "bench:rid42");
        assert_eq!(o.session_syntaxk(), "syntaxk:rid42");
        assert_eq!(o.session_mcp(), "mcp:rid42");
        assert_eq!(o.session_workflow(), "workflow:rid42");
        assert_eq!(o.session_mens(), "mens:rid42");
        assert_eq!(o.session_route(), "route:rid42");
    }

    #[test]
    fn new_phase_b_d_constants_pass_validation() {
        for mt in [
            METRIC_TYPE_MODEL_CALL_EVENT,
            METRIC_TYPE_TASK_ROOT_SUMMARY,
            METRIC_TYPE_BUILD_SUMMARY_EVENT,
            METRIC_TYPE_ERROR_EVENT,
        ] {
            assert!(
                validate_research_metric_row("sess", mt, None).is_ok(),
                "{mt} failed validation"
            );
        }
    }
}
```

- [ ] **Step 2: Verify `types.rs` compiles in isolation (lib.rs still placeholder)**

This will fail at link until TelemetryError is defined in lib.rs (Task 5). Skip compilation check until Task 5 is done.

---

## Task 3 — Create `recorder.rs`, `no_op.rs`, `span.rs`, `config.rs`

**Files:**
- Create: `crates/vox-telemetry/src/recorder.rs`
- Create: `crates/vox-telemetry/src/no_op.rs`
- Create: `crates/vox-telemetry/src/span.rs`
- Create: `crates/vox-telemetry/src/config.rs`

- [ ] **Step 1: Write `recorder.rs`**

```rust
// crates/vox-telemetry/src/recorder.rs
use std::sync::{Arc, OnceLock};

use crate::types::TelemetryEvent;

/// Trait implemented by every telemetry sink.
///
/// `record` is called synchronously on the caller's thread/task. Implementations
/// MUST return quickly (fire-and-forget internally via `tokio::spawn` or a channel).
pub trait TelemetryRecorder: Send + Sync + 'static {
    fn record(&self, event: &TelemetryEvent);
}

static GLOBAL_RECORDER: OnceLock<Arc<dyn TelemetryRecorder>> = OnceLock::new();

/// Register the process-wide recorder. Silently ignored if called more than once
/// (first writer wins). Call once at binary startup before any `record_event!`.
pub fn set_global_recorder(recorder: Arc<dyn TelemetryRecorder>) {
    let _ = GLOBAL_RECORDER.set(recorder);
}

/// Returns the global recorder, or `None` if not yet initialized.
///
/// Used by the `record_event!` macro; callers should prefer that macro.
pub fn global_recorder() -> Option<&'static Arc<dyn TelemetryRecorder>> {
    GLOBAL_RECORDER.get()
}

/// Fan-out recorder: delegates every `record` call to all inner recorders.
pub struct CompositeRecorder {
    inner: Vec<Arc<dyn TelemetryRecorder>>,
}

impl CompositeRecorder {
    pub fn new(inner: Vec<Arc<dyn TelemetryRecorder>>) -> Self {
        Self { inner }
    }
}

impl TelemetryRecorder for CompositeRecorder {
    fn record(&self, event: &TelemetryEvent) {
        for r in &self.inner {
            r.record(event);
        }
    }
}
```

- [ ] **Step 2: Write `no_op.rs`**

```rust
// crates/vox-telemetry/src/no_op.rs
use crate::{recorder::TelemetryRecorder, types::TelemetryEvent};

/// Default recorder used when no sink is registered. Discards all events silently.
pub struct NoOpRecorder;

impl TelemetryRecorder for NoOpRecorder {
    #[inline]
    fn record(&self, _event: &TelemetryEvent) {}
}
```

- [ ] **Step 3: Write `span.rs`**

```rust
// crates/vox-telemetry/src/span.rs
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
```

- [ ] **Step 4: Write `config.rs`**

```rust
// crates/vox-telemetry/src/config.rs
//! `TelemetryConfig`: read once at startup, governs which sinks and categories are active.
//!
//! Resolution order (highest wins):
//!   1. `/etc/vox/telemetry-policy.toml` — org-level hard-off (Phase D)
//!   2. `~/.config/vox/config.toml`        — user preference (Phase D)
//!   3. `VOX_TELEMETRY`                    — master on/off/debug (Phase D)
//!   4. Legacy per-category env vars        — compat shim
//!   5. Default                             — local collection on, remote upload off
//!
//! Phases B–D complete the config loading. In Phase A, only `from_env_legacy` is
//! implemented to keep the existing per-category env-var gates working.

/// Master telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Master switch. When false, `record_event!` is a guaranteed no-op.
    pub enabled: bool,
    /// Whether to attempt remote upload via the spool. Requires Clavis credentials (ADR 023).
    pub remote_upload: bool,
    /// Gather research_metrics (existing categories: benchmark, syntax_k, socrates, etc.).
    pub research_metrics: bool,
    /// Gather per-LLM-call model performance events (Phase B).
    pub model_calls: bool,
    /// Gather agent dispatch + task root summary (Phase C).
    pub agent_orchestration: bool,
    /// Gather build summary events (Phase D).
    pub build: bool,
    /// Gather subsystem error events (Phase D).
    pub errors: bool,
}

impl TelemetryConfig {
    /// Returns the default config: local collection on, all categories on, remote upload off.
    pub fn default_on() -> Self {
        Self {
            enabled: true,
            remote_upload: false,
            research_metrics: true,
            model_calls: true,
            agent_orchestration: true,
            build: true,
            errors: true,
        }
    }

    /// Returns an all-off config (used for tests and when master switch is disabled).
    pub fn all_off() -> Self {
        Self {
            enabled: false,
            remote_upload: false,
            research_metrics: false,
            model_calls: false,
            agent_orchestration: false,
            build: false,
            errors: false,
        }
    }

    /// Load from legacy per-category env vars. The master `VOX_TELEMETRY` switch and
    /// file-based config layers are added in Phase D.
    pub fn from_env_legacy() -> Self {
        let benchmark_on = env_flag("VOX_BENCHMARK_TELEMETRY");
        Self {
            enabled: true,
            remote_upload: false,
            // In Phase A, research_metrics category follows legacy benchmark gate.
            // Phase D replaces this with the master switch.
            research_metrics: benchmark_on.unwrap_or(false),
            model_calls: false, // activated Phase B
            agent_orchestration: false, // activated Phase C
            build: false, // activated Phase D
            errors: false, // activated Phase D
        }
    }
}

fn env_flag(key: &str) -> Option<bool> {
    match std::env::var(key).ok()?.trim() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self::from_env_legacy()
    }
}
```

---

## Task 4 — Write `lib.rs`: `TelemetryError`, public exports, and `record_event!`

**Files:**
- Modify: `crates/vox-telemetry/src/lib.rs`

- [ ] **Step 1: Write the complete `lib.rs`**

```rust
// crates/vox-telemetry/src/lib.rs
//! `vox-telemetry` — L1 telemetry facade.
//!
//! Zero domain dependencies. Every emitter in the workspace depends on this crate
//! for the `record_event!` macro and `METRIC_TYPE_*` constants. Sinks live in
//! higher-layer crates (`vox-db`, `vox-cli`) and register themselves at binary
//! startup via [`set_global_recorder`].
//!
//! # Quick start for producers
//!
//! ```rust,ignore
//! use vox_telemetry::{record_event, TelemetryEvent, ResearchMetricEvent, METRIC_TYPE_BENCHMARK_EVENT};
//!
//! record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent {
//!     session_id: "bench:myrepo".into(),
//!     metric_type: METRIC_TYPE_BENCHMARK_EVENT.into(),
//!     metric_value: Some(42.0),
//!     metadata_json: None,
//! }));
//! ```
//!
//! If no recorder is registered, `record_event!` is a no-op.

pub mod config;
pub mod no_op;
pub mod recorder;
pub mod span;
pub mod types;

// ── Public re-exports ─────────────────────────────────────────────────────

pub use config::TelemetryConfig;
pub use no_op::NoOpRecorder;
pub use recorder::{CompositeRecorder, TelemetryRecorder, global_recorder, set_global_recorder};
pub use span::{TRACE_CTX, TraceContext, current_trace_ctx};
pub use types::{
    // size limits
    RESEARCH_METRICS_METADATA_JSON_MAX_BYTES,
    RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS,
    RESEARCH_METRICS_SESSION_ID_MAX_CHARS,
    // existing metric types
    METRIC_TYPE_AGENT_EXEC_TIME,
    METRIC_TYPE_BANDIT_UPDATE,
    METRIC_TYPE_BENCHMARK_EVENT,
    METRIC_TYPE_BUDGET_DECISION,
    METRIC_TYPE_CACHE_HIT_PREDICTION,
    METRIC_TYPE_CALIBRATION_RUN,
    METRIC_TYPE_CHAIN_DEPTH_ALERT,
    METRIC_TYPE_CIRCUIT_BREAKER_TRIP,
    METRIC_TYPE_DRIFT_ALERT,
    METRIC_TYPE_HITL_INTERRUPT,
    METRIC_TYPE_MEMORY_HYBRID_FUSION,
    METRIC_TYPE_MODEL_ROUTE_EVENT,
    METRIC_TYPE_MODEL_TIER_ROUTE,
    METRIC_TYPE_PLAN_MODE_DECISION,
    METRIC_TYPE_POPULI_CONTROL_EVENT,
    METRIC_TYPE_PRIVACY_ROUTE_DECISION,
    METRIC_TYPE_QUESTIONING_EVENT,
    METRIC_TYPE_RISK_SCORE,
    METRIC_TYPE_SOCRATES_FUSION,
    METRIC_TYPE_SOCRATES_SURFACE,
    METRIC_TYPE_SUBAGENT_DISPATCH,
    METRIC_TYPE_SYNTAX_K_EVENT,
    METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY,
    // new metric types (Phase B–D emit sites)
    METRIC_TYPE_BUILD_SUMMARY_EVENT,
    METRIC_TYPE_ERROR_EVENT,
    METRIC_TYPE_MODEL_CALL_EVENT,
    METRIC_TYPE_TASK_ROOT_SUMMARY,
    // session prefixes
    SESSION_ID_MEMORY_HYBRID_FUSION,
    SESSION_PREFIX_BENCH,
    SESSION_PREFIX_MENS,
    SESSION_PREFIX_MCP,
    SESSION_PREFIX_ROUTE,
    SESSION_PREFIX_SYNTAXK,
    SESSION_PREFIX_WORKFLOW,
    // event types
    ResearchMetricEvent,
    TelemetryEvent,
    // write helpers
    TelemetryWriteOptions,
    validate_research_metric_row,
};

// ── record_event! macro ───────────────────────────────────────────────────

/// Emit a telemetry event through the global recorder.
///
/// No-op (zero cost) when no recorder has been registered via [`set_global_recorder`].
///
/// ```rust,ignore
/// record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent { … }));
/// ```
#[macro_export]
macro_rules! record_event {
    ($event:expr) => {
        if let Some(r) = $crate::global_recorder() {
            r.record($event);
        }
    };
}
```

- [ ] **Step 2: Run tests for the new crate**

```bash
cargo test -p vox-telemetry
```

Expected: all tests pass (the 5 tests in `types.rs`).

- [ ] **Step 3: Commit the vox-telemetry implementation**

```bash
git add crates/vox-telemetry/src/
git commit -m "feat(vox-telemetry): implement types, recorder, span, config, record_event! macro"
```

---

## Task 5 — Update `vox-db`: add dependency, turn `research_metrics_contract` into re-export

**Files:**
- Modify: `crates/vox-db/Cargo.toml`
- Modify: `crates/vox-db/src/research_metrics_contract.rs`
- Modify: `crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs`

- [ ] **Step 1: Add `vox-telemetry` to `vox-db/Cargo.toml`**

In `crates/vox-db/Cargo.toml`, under `[dependencies]`, add:

```toml
vox-telemetry = { workspace = true }
```

- [ ] **Step 2: Write failing test first — ensure existing `vox-db` metric contract tests still pass after re-export**

Run the existing tests to capture baseline:

```bash
cargo test -p vox-db research_metrics_contract
```

Expected: 4 tests pass (`rejects_empty_session_and_metric_type`, `accepts_colon_in_metric_type`, `rejects_disallowed_metric_type_chars`, `enforces_metadata_size_cap`, `telemetry_write_options_builds_expected_sessions`).

- [ ] **Step 3: Replace `research_metrics_contract.rs` body with a re-export shim**

Completely replace `crates/vox-db/src/research_metrics_contract.rs` with:

```rust
//! Re-exports from [`vox_telemetry::types`] — this module is preserved for
//! backwards compatibility. All constants, types, and helpers have moved to
//! the `vox-telemetry` L1 crate.
//!
//! - Design: `docs/src/architecture/telemetry-unification-design-2026.md`

pub use vox_telemetry::{
    RESEARCH_METRICS_METADATA_JSON_MAX_BYTES,
    RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS,
    RESEARCH_METRICS_SESSION_ID_MAX_CHARS,
    METRIC_TYPE_AGENT_EXEC_TIME,
    METRIC_TYPE_BANDIT_UPDATE,
    METRIC_TYPE_BENCHMARK_EVENT,
    METRIC_TYPE_BUDGET_DECISION,
    METRIC_TYPE_BUILD_SUMMARY_EVENT,
    METRIC_TYPE_CACHE_HIT_PREDICTION,
    METRIC_TYPE_CALIBRATION_RUN,
    METRIC_TYPE_CHAIN_DEPTH_ALERT,
    METRIC_TYPE_CIRCUIT_BREAKER_TRIP,
    METRIC_TYPE_DRIFT_ALERT,
    METRIC_TYPE_ERROR_EVENT,
    METRIC_TYPE_HITL_INTERRUPT,
    METRIC_TYPE_MEMORY_HYBRID_FUSION,
    METRIC_TYPE_MODEL_CALL_EVENT,
    METRIC_TYPE_MODEL_ROUTE_EVENT,
    METRIC_TYPE_MODEL_TIER_ROUTE,
    METRIC_TYPE_PLAN_MODE_DECISION,
    METRIC_TYPE_POPULI_CONTROL_EVENT,
    METRIC_TYPE_PRIVACY_ROUTE_DECISION,
    METRIC_TYPE_QUESTIONING_EVENT,
    METRIC_TYPE_RISK_SCORE,
    METRIC_TYPE_SOCRATES_FUSION,
    METRIC_TYPE_SOCRATES_SURFACE,
    METRIC_TYPE_SUBAGENT_DISPATCH,
    METRIC_TYPE_SYNTAX_K_EVENT,
    METRIC_TYPE_TASK_ROOT_SUMMARY,
    METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY,
    SESSION_ID_MEMORY_HYBRID_FUSION,
    SESSION_PREFIX_BENCH,
    SESSION_PREFIX_MENS,
    SESSION_PREFIX_MCP,
    SESSION_PREFIX_ROUTE,
    SESSION_PREFIX_SYNTAXK,
    SESSION_PREFIX_WORKFLOW,
    TelemetryError,
    TelemetryWriteOptions,
    validate_research_metric_row,
};
```

- [ ] **Step 4: Update `codex_metrics_packages.rs` import and error conversion**

In `crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs`, change line 3:

```rust
// Before:
use crate::research_metrics_contract::validate_research_metric_row;

// After:
use vox_telemetry::validate_research_metric_row;
```

And change line 21:

```rust
// Before:
validate_research_metric_row(session_id, metric_type, metadata_json)?;

// After:
validate_research_metric_row(session_id, metric_type, metadata_json)
    .map_err(|e| StoreError::Db(e.to_string()))?;
```

- [ ] **Step 5: Run baseline tests again to confirm re-exports work**

```bash
cargo test -p vox-db research_metrics_contract
```

Expected: same tests as before still pass (tests live in `vox-telemetry` now; re-export shim has none, which is correct).

- [ ] **Step 6: Run full `vox-db` test suite**

```bash
cargo test -p vox-db
```

Expected: all existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/
git commit -m "refactor(vox-db): research_metrics_contract → re-export from vox-telemetry"
```

---

## Task 6 — Add `ResearchMetricsSink` to `vox-db`

**Files:**
- Create: `crates/vox-db/src/telemetry_sink.rs`
- Modify: `crates/vox-db/src/lib.rs`

- [ ] **Step 1: Write `crates/vox-db/src/telemetry_sink.rs`**

```rust
// crates/vox-db/src/telemetry_sink.rs
//! [`ResearchMetricsSink`] — writes `TelemetryEvent::ResearchMetric` events to the
//! `research_metrics` table via [`crate::VoxDb::append_research_metric`].
//!
//! Other event variants are silently ignored; they are handled by higher-layer sinks
//! introduced in Phases B–D.

use std::sync::Arc;

use vox_telemetry::{ResearchMetricEvent, TelemetryEvent, TelemetryRecorder};

/// `TelemetryRecorder` sink backed by a live `VoxDb` connection.
///
/// `record` spawns a background tokio task for the async DB write so the caller
/// is never blocked. Write failures are logged at WARN and swallowed — telemetry
/// must never surface as a user-visible error.
pub struct ResearchMetricsSink {
    db: Arc<crate::VoxDb>,
}

impl ResearchMetricsSink {
    pub fn new(db: crate::VoxDb) -> Self {
        Self { db: Arc::new(db) }
    }
}

impl TelemetryRecorder for ResearchMetricsSink {
    fn record(&self, event: &TelemetryEvent) {
        let TelemetryEvent::ResearchMetric(e) = event else {
            return;
        };
        let db = Arc::clone(&self.db);
        let e: ResearchMetricEvent = e.clone();
        tokio::spawn(async move {
            if let Err(err) = db
                .append_research_metric(
                    &e.session_id,
                    &e.metric_type,
                    e.metric_value,
                    e.metadata_json.as_deref(),
                )
                .await
            {
                tracing::warn!(?err, "ResearchMetricsSink: append_research_metric failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_metrics_sink_is_recorder() {
        fn _assert_recorder<T: vox_telemetry::TelemetryRecorder>() {}
        _assert_recorder::<ResearchMetricsSink>();
    }
}
```

- [ ] **Step 2: Add `pub mod telemetry_sink;` to `vox-db/src/lib.rs`**

Find the block of `pub mod` declarations (around line 64) and add:

```rust
pub mod telemetry_sink;
```

- [ ] **Step 3: Run test**

```bash
cargo test -p vox-db telemetry_sink
```

Expected: `research_metrics_sink_is_recorder` PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/telemetry_sink.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): add ResearchMetricsSink implementing TelemetryRecorder"
```

---

## Task 7 — Add `SpoolSink` and `init_telemetry_sinks` to `vox-cli`

**Files:**
- Modify: `crates/vox-cli/Cargo.toml`
- Create: `crates/vox-cli/src/telemetry_sink.rs`
- Modify: `crates/vox-cli/src/lib.rs`

- [ ] **Step 1: Add `vox-telemetry` dep to `vox-cli/Cargo.toml`**

In `crates/vox-cli/Cargo.toml`, under `[dependencies]`, add:

```toml
vox-telemetry = { workspace = true }
```

- [ ] **Step 2: Write `telemetry_sink.rs` in vox-cli**

```rust
// crates/vox-cli/src/telemetry_sink.rs
//! [`SpoolSink`] — serializes `TelemetryEvent` to the local upload queue.
//!
//! Only S0–S1 events should reach this sink by default. The spool is drained
//! by `vox telemetry upload` when the user has configured upload credentials
//! (ADR 023).

use std::path::PathBuf;

use vox_telemetry::{TelemetryEvent, TelemetryRecorder};

/// `TelemetryRecorder` sink that writes events as JSON files to the local spool.
///
/// `record` spawns a blocking tokio task to avoid holding an async executor
/// during file I/O. Errors are logged at DEBUG (spool failure is non-critical).
pub struct SpoolSink {
    root: PathBuf,
}

impl SpoolSink {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl TelemetryRecorder for SpoolSink {
    fn record(&self, event: &TelemetryEvent) {
        let root = self.root.clone();
        let event = event.clone();
        tokio::spawn(async move {
            if let Err(err) = crate::telemetry_spool::enqueue(&root, &event) {
                tracing::debug!(?err, "SpoolSink: enqueue failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spool_sink_is_recorder() {
        fn _assert_recorder<T: vox_telemetry::TelemetryRecorder>() {}
        _assert_recorder::<SpoolSink>();
    }
}
```

- [ ] **Step 3: Add `pub mod telemetry_sink;` and `init_telemetry_sinks` to `vox-cli/src/lib.rs`**

Find the area around `pub use vox_cli_core::init_tracing_for_cli;` (line ~87). Add:

```rust
pub mod telemetry_sink;
```

Then add the `init_telemetry_sinks` function (place it before `run_vox_cli_from_parsed`, around line 555):

```rust
/// Register the process-wide telemetry sinks.
///
/// In Phase A `db` is always `None` — the ResearchMetricsSink is not wired yet.
/// Phase B passes `Some(db)` after the workspace DB is opened.
pub fn init_telemetry_sinks(db: Option<vox_db::VoxDb>) {
    use std::sync::Arc;
    use vox_telemetry::{CompositeRecorder, TelemetryRecorder};

    let mut sinks: Vec<Arc<dyn TelemetryRecorder>> = Vec::new();

    if let Some(db) = db {
        sinks.push(Arc::new(
            crate::telemetry_sink::ResearchMetricsSink::new(db),
        ));
    }

    // SpoolSink is always registered so S0–S1 events can be spooled for later upload.
    sinks.push(Arc::new(crate::telemetry_sink::SpoolSink::new(
        crate::telemetry_spool::spool_root(),
    )));

    vox_telemetry::set_global_recorder(Arc::new(CompositeRecorder::new(sinks)));
}
```

Wait — `ResearchMetricsSink` is defined in `vox-db`, not `vox-cli`. Fix the reference:

```rust
pub fn init_telemetry_sinks(db: Option<vox_db::VoxDb>) {
    use std::sync::Arc;
    use vox_telemetry::{CompositeRecorder, TelemetryRecorder};

    let mut sinks: Vec<Arc<dyn TelemetryRecorder>> = Vec::new();

    if let Some(db) = db {
        sinks.push(Arc::new(vox_db::telemetry_sink::ResearchMetricsSink::new(db)));
    }

    sinks.push(Arc::new(crate::telemetry_sink::SpoolSink::new(
        crate::telemetry_spool::spool_root(),
    )));

    vox_telemetry::set_global_recorder(Arc::new(CompositeRecorder::new(sinks)));
}
```

- [ ] **Step 4: Call `init_telemetry_sinks` from `run_vox_cli_from_parsed`**

In `crates/vox-cli/src/lib.rs`, find `run_vox_cli_from_parsed` (line ~563) and add the call:

```rust
pub async fn run_vox_cli_from_parsed(root: VoxCliRoot) -> anyhow::Result<()> {
    if root.global.verbose > 0 && std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            crate::config::set_process_env("RUST_LOG", "debug");
        }
    }
    init_tracing_for_cli();
    init_telemetry_sinks(None); // Phase B: pass Some(workspace_db) here
    apply_global_opts(&root.global);
    cli_dispatch::dispatch_cli(root.cmd, &root.global).await
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p vox-cli telemetry_sink
```

Expected: `spool_sink_is_recorder` PASS.

- [ ] **Step 6: Run full vox-cli test suite**

```bash
cargo test -p vox-cli
```

Expected: all existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/
git commit -m "feat(vox-cli): add SpoolSink + init_telemetry_sinks; wire at startup (Phase A — db=None)"
```

---

## Task 8 — Update `data_ssot_guards.rs` CI guard for the new constants path

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs`

- [ ] **Step 1: Write a failing test to confirm guard uses the new path**

Read the current guard to find the exact string to change:

```bash
grep -n "research_metrics_contract\|vox-db/src" crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs
```

Expected output: one or more lines showing the hardcoded `crates/vox-db/src/research_metrics_contract.rs` path.

- [ ] **Step 2: Update the path in `data_ssot_guards.rs`**

Find line 292 (approximately):

```rust
// Before:
let research_contract = root.join("crates/vox-db/src/research_metrics_contract.rs");

// After:
let research_contract = root.join("crates/vox-telemetry/src/types.rs");
```

- [ ] **Step 3: Run the data-ssot-guards check**

```bash
cargo run -p vox-cli -- ci data-ssot-guards 2>&1 | tail -20
```

Expected: exits 0, no "expected pub const METRIC_TYPE_*" errors. The guard should now find all constants in `vox-telemetry/src/types.rs`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs
git commit -m "fix(vox-cli/ci): update data_ssot_guards path to vox-telemetry/src/types.rs"
```

---

## Task 9 — Update `where-things-live.md` and final verification

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add `vox-telemetry` row to the L1 table in `where-things-live.md`**

Find the `### L1 — primitives & utilities` table. Add as the last row before the blank line:

```markdown
| [`vox-telemetry`](../../../../crates/vox-telemetry/) | L1 telemetry facade: `METRIC_TYPE_*` constants, `TelemetryRecorder` trait, `record_event!` macro. Zero domain dependencies. |
```

- [ ] **Step 2: Run `vox-arch-check`**

```bash
cargo run -p vox-arch-check -- .
```

Expected: exits 0. If it complains about `vox-telemetry` not being in `where_things_live`, the step above fixes it. If it complains about `description_present`, ensure the `Cargo.toml` description field is filled (it is).

- [ ] **Step 3: Run full workspace test sweep for affected crates**

```bash
cargo test -p vox-telemetry -p vox-db -p vox-cli
```

Expected: all tests pass.

- [ ] **Step 4: Run `vox ci` gates**

```bash
cargo run -p vox-cli -- ci run 2>&1 | grep -E "PASS|FAIL|error"
```

Expected: no new FAIL lines. The `data-ssot-guards` gate must be green.

- [ ] **Step 5: Final commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): add vox-telemetry to where-things-live.md L1 table"
```

---

## Verification checklist

- [ ] `cargo test -p vox-telemetry` — 5 types tests + 1 recorder test pass
- [ ] `cargo test -p vox-db` — all existing tests pass; `research_metrics_contract` re-exports compile
- [ ] `cargo test -p vox-cli` — all existing tests pass; `telemetry_sink` tests pass
- [ ] `cargo run -p vox-arch-check -- .` — exits 0; `vox-telemetry` at L1, no inversion
- [ ] `cargo run -p vox-cli -- ci data-ssot-guards` — exits 0; finds constants in new path
- [ ] No `record_event!` call sites exist yet — no existing emission behavior changed
- [ ] `vox-db::research_metrics_contract` still exports all constants that other crates import

---

## Phase B preview (separate plan)

Phase B wires the first actual emit call site: `model_call_event`. It:
1. Adds `ModelCallEvent` variant to `TelemetryEvent` in `vox-telemetry/src/types.rs`
2. Calls `record_event!` in `vox-orchestrator-mcp/src/llm_bridge/infer.rs` after cost computation
3. Changes `init_telemetry_sinks(None)` to `init_telemetry_sinks(Some(db))` in `run_vox_cli_from_parsed`
4. Adds a DB integration test asserting cache tokens survive round-trip to `research_metrics`
